use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Multipart, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::audio::{
    audio_upload_path, decode_audio_segments, is_audio_content_type, is_audio_extension,
    write_audio_upload,
};
use crate::config::Settings;
use crate::embedder::ImageEmbedder;
use crate::image_io::load_media_bytes;
use crate::indexer::ImageIndexer;
use crate::models::{HealthResponse, IndexResponse, SearchResponse};
use crate::models::{SearchResult, SearchSceneResponse};
use crate::qdrant::QdrantImageStore;
use crate::search::ImageSearchService;
use crate::sources::build_image_sources;
use crate::video::{
    decode_video_scenes, is_video_content_type, is_video_extension, video_upload_path,
    write_video_upload,
};

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub store: QdrantImageStore,
    pub embedder: ImageEmbedder,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        let store = QdrantImageStore::new(
            settings.qdrant_url.clone(),
            settings.qdrant_collection.clone(),
            settings.vector_size,
        );
        let embedder = ImageEmbedder::new(settings.clip_model_name.clone(), settings.vector_size);
        Self {
            settings,
            store,
            embedder,
        }
    }
}

#[derive(Deserialize)]
pub struct SearchQuery {
    limit: Option<u32>,
}

pub async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let sources = build_image_sources(&state.settings);
    Json(HealthResponse {
        status: "ok".to_string(),
        collection: state.settings.qdrant_collection.clone(),
        source_dir: state
            .settings
            .source_image_dir
            .to_string_lossy()
            .to_string(),
        sources: sources.iter().map(|source| source.uri()).collect(),
    })
}

pub async fn index_images(State(state): State<Arc<AppState>>) -> Json<IndexResponse> {
    let indexer = ImageIndexer::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    Json(indexer.index_sources().await)
}

pub async fn search_upload(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
    mut multipart: Multipart,
) -> Result<Json<SearchResponse>, ApiError> {
    let mut uploaded = None;
    let mut upload_kind = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let content_type = field.content_type().unwrap_or_default().to_string();
        let filename = field.file_name().map(ToOwned::to_owned);
        let filename_extension = filename
            .as_deref()
            .and_then(|name| std::path::Path::new(name).extension())
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{}", extension.to_ascii_lowercase()));
        let is_image = content_type.starts_with("image/")
            || filename_extension
                .as_ref()
                .map(|extension| state.settings.image_extensions.contains(extension))
                .unwrap_or(false);
        let is_video = is_video_content_type(&content_type)
            || filename_extension
                .as_deref()
                .map(is_video_extension)
                .unwrap_or(false);
        let is_audio = is_audio_content_type(&content_type)
            || filename_extension
                .as_deref()
                .map(is_audio_extension)
                .unwrap_or(false);
        if !is_image && !is_video && !is_audio {
            return Err(ApiError::bad_request(
                "Upload must be an image, video, or audio file",
            ));
        }
        let raw = field
            .bytes()
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        uploaded = Some(raw);
        upload_kind = Some(UploadedFileKind {
            is_video,
            is_audio,
            filename,
        });
        break;
    }

    let raw = uploaded
        .ok_or_else(|| ApiError::bad_request("Upload must be an image, video, or audio file"))?;
    let upload_kind = upload_kind
        .ok_or_else(|| ApiError::bad_request("Upload must be an image, video, or audio file"))?;
    let max_bytes = state.settings.max_upload_mb as usize * 1024 * 1024;
    if raw.len() > max_bytes {
        return Err(ApiError::payload_too_large(format!(
            "Upload is larger than {} MB",
            state.settings.max_upload_mb
        )));
    }

    if upload_kind.is_video {
        return search_video_upload(state, query.limit, &raw, upload_kind.filename.as_deref())
            .await
            .map(Json);
    }

    if upload_kind.is_audio {
        return search_audio_upload(state, query.limit, &raw, upload_kind.filename.as_deref())
            .await
            .map(Json);
    }

    let media = load_media_bytes(&raw, &state.settings)
        .map_err(|_| ApiError::bad_request("Could not decode image"))?;
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    service
        .search_media(&media, query.limit)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

struct UploadedFileKind {
    is_video: bool,
    is_audio: bool,
    filename: Option<String>,
}

async fn search_audio_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let upload_path = audio_upload_path(&state.settings.upload_dir, filename);
    write_audio_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let segments = match decode_audio_segments(&upload_path, &state.settings) {
        Ok(segments) => segments,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process audio: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    let mut scene_responses = Vec::new();
    let mut flattened = Vec::new();

    for segment in &segments {
        let mut response = service
            .search_media(&segment.media, limit)
            .await
            .map_err(ApiError::internal)?;
        for result in &mut response.results {
            result.query_scene_index = Some(segment.scene_index);
        }
        flattened.extend(response.results.clone());
        scene_responses.push(SearchSceneResponse {
            scene_index: segment.scene_index,
            scene_kind: "audio_bit".to_string(),
            start_frame: (segment.start_seconds * 1000.0).round() as u64,
            end_frame: (segment.end_seconds * 1000.0).round() as u64,
            start_seconds: segment.start_seconds,
            end_seconds: segment.end_seconds,
            clip_url: None,
            speaker_id: segment.speaker_id.clone(),
            speaker_label: segment.speaker_label.clone(),
            query_phash: response.query_phash,
            count: response.count,
            results: response.results,
        });
    }

    let results = deduplicate_flat_results(flattened);
    Ok(SearchResponse {
        query_phash: scene_responses
            .first()
            .map(|scene| scene.query_phash.clone())
            .unwrap_or_default(),
        count: results.len(),
        results,
        query_media_kind: "audio".to_string(),
        scenes: scene_responses,
        query_audio_analysis: segments
            .first()
            .and_then(|segment| segment.media.audio_analysis.clone()),
    })
}

async fn search_video_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let upload_path = video_upload_path(&state.settings.upload_dir, filename);
    write_video_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let scenes = match decode_video_scenes(&upload_path, &state.settings) {
        Ok(scenes) => scenes,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process video: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    let mut scene_responses = Vec::new();
    let mut flattened = Vec::new();

    for scene in &scenes {
        let mut response = service
            .search_media(&scene.media, limit)
            .await
            .map_err(ApiError::internal)?;
        for result in &mut response.results {
            result.query_scene_index = Some(scene.scene_index);
        }
        flattened.extend(response.results.clone());
        scene_responses.push(SearchSceneResponse {
            scene_index: scene.scene_index,
            scene_kind: "scene".to_string(),
            start_frame: scene.start.frame_index,
            end_frame: scene.end.frame_index,
            start_seconds: scene.start.timestamp.seconds(),
            end_seconds: scene.end.timestamp.seconds(),
            clip_url: scene.clip_url.clone(),
            speaker_id: None,
            speaker_label: None,
            query_phash: response.query_phash,
            count: response.count,
            results: response.results,
        });
    }

    let results = deduplicate_flat_results(flattened);
    Ok(SearchResponse {
        query_phash: scene_responses
            .first()
            .map(|scene| scene.query_phash.clone())
            .unwrap_or_default(),
        count: results.len(),
        results,
        query_media_kind: "video".to_string(),
        scenes: scene_responses,
        query_audio_analysis: None,
    })
}

fn deduplicate_flat_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut by_image_id = BTreeMap::<String, SearchResult>::new();
    for result in results {
        by_image_id
            .entry(result.image.id.clone())
            .and_modify(|existing| {
                if result.vector_score > existing.vector_score {
                    *existing = result.clone();
                }
            })
            .or_insert(result);
    }
    let mut deduped = by_image_id.into_values().collect::<Vec<_>>();
    deduped.sort_by(|left, right| right.vector_score.total_cmp(&left.vector_score));
    deduped
}

pub struct ApiError {
    status: StatusCode,
    detail: String,
}

impl ApiError {
    fn bad_request(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            detail: detail.into(),
        }
    }

    fn payload_too_large(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            detail: detail.into(),
        }
    }

    fn internal(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            detail: detail.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "detail": self.detail }))).into_response()
    }
}
