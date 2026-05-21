use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::sync::Arc;

use axum::extract::{Multipart, Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use jobs_core::{
    JobArtifact, JobContext, JobError, JobEvent, JobId, JobProgress, JobSnapshot, JobSpec,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use text_analysis_transcription::{WhisperCppModel, WhisperCppModelStore};
use uuid::Uuid;

use crate::audio::{
    audio_transcription_model_store, audio_upload_path, decode_audio_segments,
    is_audio_content_type, is_audio_extension, parse_whisper_cpp_model, whisper_model_is_cached,
    write_audio_upload,
};
use crate::config::Settings;
use crate::embedder::ImageEmbedder;
use crate::image_io::load_media_bytes;
use crate::indexer::ImageIndexer;
use crate::jobs::JobManager;
use crate::models::{HealthResponse, IndexResponse, SearchResponse};
use crate::models::{SearchResult, SearchSceneResponse};
use crate::ocr::normalize_ocr_query;
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
    pub jobs: JobManager,
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
            jobs: JobManager::default(),
        }
    }
}

#[derive(Deserialize)]
pub struct SearchQuery {
    limit: Option<u32>,
    ocr_text: Option<String>,
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

pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<JobSnapshot>>, ApiError> {
    state.jobs.snapshots().map(Json).map_err(ApiError::from_job)
}

pub async fn get_job(
    State(state): State<Arc<AppState>>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let job_id = parse_job_id(job_id)?;
    state
        .jobs
        .snapshot(&job_id)
        .map_err(ApiError::from_job)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("Unknown job `{job_id}`")))
}

pub async fn get_job_events(
    State(state): State<Arc<AppState>>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<Vec<JobEvent>>, ApiError> {
    let job_id = parse_job_id(job_id)?;
    if state
        .jobs
        .snapshot(&job_id)
        .map_err(ApiError::from_job)?
        .is_none()
    {
        return Err(ApiError::not_found(format!("Unknown job `{job_id}`")));
    }
    state
        .jobs
        .events(&job_id)
        .map(Json)
        .map_err(ApiError::from_job)
}

pub async fn cancel_job(
    State(state): State<Arc<AppState>>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let job_id = parse_job_id(job_id)?;
    state
        .jobs
        .request_cancel(&job_id)
        .map_err(ApiError::from_job)?;
    state
        .jobs
        .snapshot(&job_id)
        .map_err(ApiError::from_job)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("Unknown job `{job_id}`")))
}

#[derive(Debug, Serialize)]
pub struct AudioTranscriptionModelsResponse {
    pub enabled: bool,
    pub configured_model: String,
    pub auto_download: bool,
    pub cache_dir: Option<String>,
    pub models: Vec<AudioTranscriptionModelResponse>,
}

#[derive(Debug, Serialize)]
pub struct AudioTranscriptionModelResponse {
    pub id: String,
    pub cached: bool,
    pub configured: bool,
}

pub async fn audio_transcription_models(
    State(state): State<Arc<AppState>>,
) -> Json<AudioTranscriptionModelsResponse> {
    let store = audio_transcription_model_store(&state.settings);
    let configured_model = state.settings.audio_transcription_model.clone();
    let models = store
        .catalog()
        .models
        .into_iter()
        .map(|status| {
            let id = status.model.id().to_string();
            AudioTranscriptionModelResponse {
                cached: status.cached,
                configured: id.eq_ignore_ascii_case(&configured_model),
                id,
            }
        })
        .collect();

    Json(AudioTranscriptionModelsResponse {
        enabled: state.settings.audio_transcription_enabled,
        configured_model,
        auto_download: state.settings.audio_transcription_auto_download,
        cache_dir: state
            .settings
            .audio_transcription_cache_dir
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        models,
    })
}

#[derive(Debug, Deserialize)]
pub struct AudioTranscriptionModelJobRequest {
    pub model: Option<String>,
}

pub async fn download_audio_transcription_model(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let model = requested_audio_transcription_model(&state.settings, request.model.as_deref())?;
    let store = audio_transcription_model_store(&state.settings);
    let spec = model_job_spec("model.download", "Download whisper.cpp model", model)?;
    state
        .jobs
        .spawn(spec, move |context| {
            download_whisper_cpp_model(context, store, model)?;
            Ok(())
        })
        .map(Json)
        .map_err(ApiError::from_job)
}

pub async fn enable_audio_transcription_model(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let model = requested_audio_transcription_model(&state.settings, request.model.as_deref())?;
    let store = audio_transcription_model_store(&state.settings);
    let settings = state.settings.clone();
    let spec = model_job_spec("model.enable", "Enable whisper.cpp model", model)?;
    state
        .jobs
        .spawn(spec, move |context| {
            enable_whisper_cpp_model(context, &settings, &store, model)?;
            Ok(())
        })
        .map(Json)
        .map_err(ApiError::from_job)
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
        return search_video_upload(
            state,
            query.limit,
            query.ocr_text.as_deref(),
            &raw,
            upload_kind.filename.as_deref(),
        )
        .await
        .map(Json);
    }

    if upload_kind.is_audio {
        return search_audio_upload(
            state,
            query.limit,
            query.ocr_text.as_deref(),
            &raw,
            upload_kind.filename.as_deref(),
        )
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
        .search_media(&media, query.limit, query.ocr_text.as_deref())
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
    ocr_text: Option<&str>,
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
            .search_media(&segment.media, limit, ocr_text)
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
        query_ocr_text: normalize_ocr_query(ocr_text),
    })
}

async fn search_video_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
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
            .search_media(&scene.media, limit, ocr_text)
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
        query_ocr_text: normalize_ocr_query(ocr_text),
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

fn parse_job_id(value: String) -> Result<JobId, ApiError> {
    JobId::new(value).map_err(|error| ApiError::bad_request(error.to_string()))
}

fn requested_audio_transcription_model(
    settings: &Settings,
    requested: Option<&str>,
) -> Result<WhisperCppModel, ApiError> {
    parse_whisper_cpp_model(requested.unwrap_or(&settings.audio_transcription_model))
        .map_err(ApiError::bad_request)
}

fn model_job_spec(kind: &str, name: &str, model: WhisperCppModel) -> Result<JobSpec, ApiError> {
    JobSpec::new(
        format!("{kind}.whisper.{}.{}", model.id(), Uuid::new_v4()),
        name,
    )
    .and_then(|spec| spec.with_kind(kind))
    .and_then(|spec| spec.with_metadata("provider", "whisper.cpp"))
    .and_then(|spec| spec.with_metadata("model", model.id()))
    .map_err(ApiError::from_job)
}

fn download_whisper_cpp_model(
    context: JobContext,
    store: WhisperCppModelStore,
    model: WhisperCppModel,
) -> jobs_core::Result<()> {
    context.info(format!("checking whisper.cpp model `{}`", model.id()))?;
    context.progress(
        JobProgress::new(0, Some(1))?
            .unit("steps")?
            .message("checking model cache"),
    )?;
    context.check_cancelled()?;

    let model_path = store.model_path(model);
    if model_path.is_file() {
        context.info(format!(
            "whisper.cpp model `{}` is already cached",
            model.id()
        ))?;
        context.progress(
            JobProgress::new(1, Some(1))?
                .unit("steps")?
                .message("model already cached"),
        )?;
        context.artifact(
            JobArtifact::new("model", format!("whisper.cpp model {}", model.id()))
                .kind("model")
                .path(model_path),
        )?;
        return Ok(());
    }

    fs::create_dir_all(store.models_dir()).map_err(job_failed)?;
    let temp_path = model_path.with_extension(format!("{}.part", context.id()));
    if temp_path.exists() {
        fs::remove_file(&temp_path).map_err(job_failed)?;
    }

    context.info(format!("downloading whisper.cpp model `{}`", model.id()))?;
    let mut response = reqwest::blocking::get(model.download_url())
        .and_then(|response| response.error_for_status())
        .map_err(job_failed)?;
    let total_bytes = response.content_length().filter(|total| *total > 0);
    let mut output = File::create(&temp_path).map_err(job_failed)?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        context.check_cancelled()?;
        let read = response.read(&mut buffer).map_err(job_failed)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read]).map_err(job_failed)?;
        hasher.update(&buffer[..read]);
        downloaded += read as u64;
        context.progress(bytes_progress(
            downloaded,
            total_bytes,
            format!("downloaded {} bytes", downloaded),
        )?)?;
    }
    output.flush().map_err(job_failed)?;

    let checksum = format!("{:x}", hasher.finalize());
    if checksum != model.checksum_sha256() {
        let _ = fs::remove_file(&temp_path);
        return Err(JobError::Failed(format!(
            "downloaded model `{}` failed checksum verification",
            model.id()
        )));
    }

    fs::rename(&temp_path, &model_path).map_err(job_failed)?;
    context.info(format!("downloaded whisper.cpp model `{}`", model.id()))?;
    context.artifact(
        JobArtifact::new("model", format!("whisper.cpp model {}", model.id()))
            .kind("model")
            .path(model_path),
    )?;
    Ok(())
}

fn enable_whisper_cpp_model(
    context: JobContext,
    settings: &Settings,
    store: &WhisperCppModelStore,
    model: WhisperCppModel,
) -> jobs_core::Result<()> {
    context.info(format!("checking whisper.cpp model `{}`", model.id()))?;
    context.progress(
        JobProgress::new(0, Some(1))?
            .unit("steps")?
            .message("checking model settings"),
    )?;
    context.check_cancelled()?;

    if !whisper_model_is_cached(store, model) {
        return Err(JobError::Failed(format!(
            "whisper.cpp model `{}` is not cached; download it before enabling",
            model.id()
        )));
    }

    if !settings.audio_transcription_enabled {
        context
            .warn("AUDIO_TRANSCRIPTION_ENABLED is false; set it to true to use transcription")?;
    }
    if !settings
        .audio_transcription_model
        .eq_ignore_ascii_case(model.id())
    {
        context.warn(format!(
            "AUDIO_TRANSCRIPTION_MODEL is `{}`; set it to `{}` to make this the active model",
            settings.audio_transcription_model,
            model.id()
        ))?;
    }
    context.metadata("enabled", settings.audio_transcription_enabled.to_string())?;
    context.metadata(
        "configured_model",
        settings.audio_transcription_model.clone(),
    )?;
    context.progress(
        JobProgress::new(1, Some(1))?
            .unit("steps")?
            .message("model enable check complete"),
    )?;
    context.info(format!("finished enable check for `{}`", model.id()))?;
    Ok(())
}

fn bytes_progress(
    completed: u64,
    total: Option<u64>,
    message: impl Into<String>,
) -> jobs_core::Result<JobProgress> {
    let total = total.filter(|total| *total >= completed && *total > 0);
    let progress = JobProgress::new(completed, total)?
        .unit("bytes")?
        .message(message);
    progress.validate()?;
    Ok(progress)
}

fn job_failed(error: impl std::fmt::Display) -> JobError {
    JobError::Failed(error.to_string())
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

    fn not_found(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            detail: detail.into(),
        }
    }

    fn internal(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            detail: detail.into(),
        }
    }

    fn from_job(error: JobError) -> Self {
        match error {
            JobError::InvalidArgument(message) => Self::bad_request(message),
            JobError::Cancelled => Self::bad_request("job cancelled"),
            JobError::Failed(message) | JobError::StateUnavailable(message) => {
                Self::internal(message)
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "detail": self.detail }))).into_response()
    }
}
