use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};

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

use crate::config::{parse_media_sources_file, Settings};
use crate::domain::models::{HealthResponse, IndexResponse, SearchResponse};
use crate::domain::models::{SearchResult, SearchSceneResponse};
use crate::storage::qdrant::QdrantImageStore;
use crate::workers::indexer::ImageIndexer;
use crate::workers::jobs::JobManager;
use crate::workers::media::audio::{
    audio_transcription_model_store, audio_upload_path, decode_audio_segments,
    is_audio_content_type, is_audio_extension, parse_whisper_cpp_model, whisper_model_is_cached,
    write_audio_upload,
};
use crate::workers::media::image_io::load_media_bytes;
use crate::workers::media::ocr::normalize_ocr_query;
use crate::workers::media::pdf::{
    decode_pdf, is_pdf_content_type, is_pdf_extension, pdf_upload_path, write_pdf_upload,
};
use crate::workers::media::video::{
    decode_video_scenes, is_video_content_type, is_video_extension, video_upload_path,
    write_video_upload,
};
use crate::workers::media::visual_embedding::{build_visual_embedder, VisualEmbeddingBackend};
use crate::workers::search::ImageSearchService;
use crate::workers::sources::build_image_sources;

pub struct AppState {
    pub settings: Settings,
    source_specs: RwLock<Vec<String>>,
    pub store: QdrantImageStore,
    pub embedder: Arc<dyn VisualEmbeddingBackend>,
    pub jobs: JobManager,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        let store = QdrantImageStore::new(
            settings.qdrant_url.clone(),
            settings.qdrant_collection.clone(),
            settings.visual_embedding_vector_size,
            settings.face_embedding_vector_size,
        );
        let embedder = build_visual_embedder(&settings);
        let source_specs = RwLock::new(settings.source_specs());
        Self {
            settings,
            source_specs,
            store,
            embedder,
            jobs: JobManager::default(),
        }
    }

    pub fn indexing_settings(&self) -> Settings {
        let mut settings = self.settings.clone();
        settings.image_sources = read_source_specs(&self.source_specs);
        settings
    }

    fn replace_source_specs(&self, sources: Vec<String>) {
        let mut source_specs = self
            .source_specs
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *source_specs = sources;
    }
}

#[derive(Deserialize)]
pub struct SearchQuery {
    limit: Option<u32>,
    ocr_text: Option<String>,
    person_id: Option<String>,
}

pub async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let settings = state.indexing_settings();
    let sources = build_image_sources(&settings);
    Json(HealthResponse {
        status: "ok".to_string(),
        collection: settings.qdrant_collection.clone(),
        source_dir: settings.source_image_dir.to_string_lossy().to_string(),
        sources: sources.iter().map(|source| source.uri()).collect(),
    })
}

pub async fn index_images(State(state): State<Arc<AppState>>) -> Json<IndexResponse> {
    let indexer = ImageIndexer::new(
        state.indexing_settings(),
        state.store.clone(),
        state.embedder.clone(),
    );
    Json(indexer.index_sources().await)
}

pub async fn spawn_index_job(
    State(state): State<Arc<AppState>>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let spec = JobSpec::new(
        format!("index.manual.{}", Uuid::new_v4()),
        "Index media sources",
    )
    .and_then(|spec| spec.with_kind("index.manual"))
    .and_then(|spec| spec.with_metadata("collection", state.settings.qdrant_collection.clone()))
    .map_err(ApiError::from_job)?;
    let jobs = state.jobs.clone();
    let settings = state.indexing_settings();
    let store = state.store.clone();
    let embedder = state.embedder.clone();

    jobs.spawn(spec, move |context| {
        run_index_job(context, settings, store, embedder)
    })
    .map(Json)
    .map_err(ApiError::from_job)
}

pub fn spawn_startup_index_job(state: Arc<AppState>) -> jobs_core::Result<JobSnapshot> {
    let spec = JobSpec::new(
        format!("index.startup.{}", Uuid::new_v4()),
        "Index missing media on startup",
    )?
    .with_kind("index.startup")?
    .with_metadata("collection", state.settings.qdrant_collection.clone())?;
    let jobs = state.jobs.clone();
    let settings = state.indexing_settings();
    let store = state.store.clone();
    let embedder = state.embedder.clone();

    jobs.spawn(spec, move |context| {
        run_index_job(context, settings, store, embedder)
    })
}

#[derive(Debug, Serialize)]
pub struct SourceConfigResponse {
    pub media_sources_file: String,
    pub default_source_dir: String,
    pub sources: Vec<SourceConfigSource>,
    pub supported_source_types: Vec<SupportedSourceType>,
    pub indexing: SourceIndexingConfig,
}

#[derive(Debug, Serialize)]
pub struct SourceConfigSource {
    pub spec: String,
    pub kind: String,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SupportedSourceType {
    pub kind: String,
    pub label: String,
    pub implemented: bool,
    pub example: String,
}

#[derive(Debug, Serialize)]
pub struct SourceIndexingConfig {
    pub collection: String,
    pub image_extensions: Vec<String>,
    pub audio_extensions: Vec<String>,
    pub pdf_extensions: Vec<String>,
    pub video_extensions: Vec<String>,
    pub visual_embedding_enabled: bool,
    pub visual_embedding_model: String,
    pub visual_embedding_vector_size: usize,
    pub face_analysis_enabled: bool,
    pub face_detection_min_confidence: f32,
    pub face_cluster_threshold: f32,
    pub gif_sample_frames: usize,
    pub gif_max_decode_frames: usize,
    pub gif_preview_frames: usize,
    pub gif_motion_weight: f32,
    pub video_frame_stride: u32,
    pub video_max_frames: Option<u32>,
    pub pdf_render_dpi: u32,
    pub pdf_max_pages: u32,
    pub pdf_summary_pages: usize,
    pub ocr_enabled: bool,
    pub ocr_max_frames: usize,
    pub audio_transcription_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSourceConfigRequest {
    pub sources: Vec<String>,
}

pub async fn get_source_config(State(state): State<Arc<AppState>>) -> Json<SourceConfigResponse> {
    Json(source_config_response(&state))
}

pub async fn update_source_config(
    State(state): State<Arc<AppState>>,
    Json(request): Json<UpdateSourceConfigRequest>,
) -> Result<Json<SourceConfigResponse>, ApiError> {
    let sources = normalize_source_specs(&request.sources)?;
    write_media_sources_file(&state.settings.media_sources_file, &sources)?;
    state.replace_source_specs(sources);
    Ok(Json(source_config_response(&state)))
}

fn source_config_response(state: &AppState) -> SourceConfigResponse {
    let settings = state.indexing_settings();
    SourceConfigResponse {
        media_sources_file: settings.media_sources_file.to_string_lossy().to_string(),
        default_source_dir: settings.source_image_dir.to_string_lossy().to_string(),
        sources: settings
            .source_specs()
            .into_iter()
            .map(source_config_source)
            .collect(),
        supported_source_types: supported_source_types(),
        indexing: SourceIndexingConfig {
            collection: settings.qdrant_collection,
            image_extensions: settings.image_extensions.into_iter().collect(),
            audio_extensions: settings.audio_extensions.into_iter().collect(),
            pdf_extensions: settings.pdf_extensions.into_iter().collect(),
            video_extensions: video_source_extensions(),
            visual_embedding_enabled: settings.visual_embedding_enabled,
            visual_embedding_model: settings.clip_model_name.clone(),
            visual_embedding_vector_size: settings.visual_embedding_vector_size,
            face_analysis_enabled: settings.face_analysis_enabled,
            face_detection_min_confidence: settings.face_detection_min_confidence,
            face_cluster_threshold: settings.face_cluster_threshold,
            gif_sample_frames: settings.gif_sample_frames,
            gif_max_decode_frames: settings.gif_max_decode_frames,
            gif_preview_frames: settings.gif_preview_frames,
            gif_motion_weight: settings.gif_motion_weight,
            video_frame_stride: settings.video_frame_stride,
            video_max_frames: settings.video_max_frames,
            pdf_render_dpi: settings.pdf_render_dpi,
            pdf_max_pages: settings.pdf_max_pages,
            pdf_summary_pages: settings.pdf_summary_pages,
            ocr_enabled: settings.ocr_enabled,
            ocr_max_frames: settings.ocr_max_frames,
            audio_transcription_enabled: settings.audio_transcription_enabled,
        },
    }
}

fn read_source_specs(source_specs: &RwLock<Vec<String>>) -> Vec<String> {
    source_specs
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn normalize_source_specs(sources: &[String]) -> Result<Vec<String>, ApiError> {
    let input = sources
        .iter()
        .map(|source| source.trim())
        .filter(|source| !source.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let parsed = parse_media_sources_file(&input).map_err(ApiError::bad_request)?;
    if parsed.is_empty() {
        return Err(ApiError::bad_request(
            "At least one media source must be configured",
        ));
    }
    Ok(parsed)
}

fn write_media_sources_file(path: &std::path::Path, sources: &[String]) -> Result<(), ApiError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ApiError::internal(format!(
                "Could not create media source config directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let mut content = "# Managed by image-similarity-service.\n".to_string();
    content.push_str("# One source per line. Supported now: local paths, file://, local://.\n");
    content.push_str(
        "# Planned source specs can be kept here, but unsupported types will be skipped.\n",
    );
    for source in sources {
        content.push_str(source);
        content.push('\n');
    }
    fs::write(path, content).map_err(|error| {
        ApiError::internal(format!(
            "Could not write media source config file {}: {error}",
            path.display()
        ))
    })
}

fn source_config_source(spec: String) -> SourceConfigSource {
    let kind = source_kind(&spec);
    let (status, detail) = match kind.as_str() {
        "local" => {
            let path = local_source_path(&spec);
            if path.is_dir() {
                ("ready".to_string(), None)
            } else {
                (
                    "unavailable".to_string(),
                    Some(format!("Directory does not exist: {}", path.display())),
                )
            }
        }
        "minio" => (
            "not_implemented".to_string(),
            Some("MinIO sources are not implemented in the native Rust service yet".to_string()),
        ),
        "video" => (
            "not_implemented".to_string(),
            Some(
                "Video source specs are not implemented; local folders can include video files"
                    .to_string(),
            ),
        ),
        "camera" => (
            "not_implemented".to_string(),
            Some("Camera sources are not implemented in the native Rust service yet".to_string()),
        ),
        _ => (
            "unsupported".to_string(),
            Some(format!("Unsupported media source: {spec}")),
        ),
    };

    SourceConfigSource {
        spec,
        kind,
        status,
        detail,
    }
}

fn source_kind(spec: &str) -> String {
    if let Some((scheme, _)) = spec.split_once(':') {
        if scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        }) {
            return match scheme {
                "file" | "local" => "local".to_string(),
                other => other.to_string(),
            };
        }
    }
    "local".to_string()
}

fn local_source_path(spec: &str) -> std::path::PathBuf {
    match url::Url::parse(spec) {
        Ok(url) if url.scheme() == "file" => {
            url.to_file_path().unwrap_or_else(|_| url.path().into())
        }
        Ok(url) if url.scheme() == "local" => {
            let mut path = String::new();
            if let Some(host) = url.host_str() {
                path.push('/');
                path.push_str(host);
            }
            path.push_str(url.path());
            path.into()
        }
        _ => spec.into(),
    }
}

fn supported_source_types() -> Vec<SupportedSourceType> {
    vec![
        SupportedSourceType {
            kind: "local".to_string(),
            label: "Local folder".to_string(),
            implemented: true,
            example: "/images or local:///images".to_string(),
        },
        SupportedSourceType {
            kind: "minio".to_string(),
            label: "MinIO bucket".to_string(),
            implemented: false,
            example: "minio://bucket/prefix".to_string(),
        },
        SupportedSourceType {
            kind: "video".to_string(),
            label: "Video stream".to_string(),
            implemented: false,
            example: "video:///clips/demo.mp4".to_string(),
        },
        SupportedSourceType {
            kind: "camera".to_string(),
            label: "Camera".to_string(),
            implemented: false,
            example: "camera://front-door".to_string(),
        },
    ]
}

fn video_source_extensions() -> Vec<String> {
    [".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
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
        let is_pdf = is_pdf_content_type(&content_type)
            || filename_extension
                .as_deref()
                .map(is_pdf_extension)
                .unwrap_or(false);
        if !is_image && !is_video && !is_audio && !is_pdf {
            return Err(ApiError::bad_request(
                "Upload must be an image, video, audio, or PDF file",
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
            is_pdf,
            filename,
        });
        break;
    }

    let raw = uploaded.ok_or_else(|| {
        ApiError::bad_request("Upload must be an image, video, audio, or PDF file")
    })?;
    let upload_kind = upload_kind.ok_or_else(|| {
        ApiError::bad_request("Upload must be an image, video, audio, or PDF file")
    })?;
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
            query.person_id.as_deref(),
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
            query.person_id.as_deref(),
            &raw,
            upload_kind.filename.as_deref(),
        )
        .await
        .map(Json);
    }

    if upload_kind.is_pdf {
        return search_pdf_upload(
            state,
            query.limit,
            query.ocr_text.as_deref(),
            query.person_id.as_deref(),
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
        .search_media(
            &media,
            query.limit,
            query.ocr_text.as_deref(),
            query.person_id.as_deref(),
        )
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

struct UploadedFileKind {
    is_video: bool,
    is_audio: bool,
    is_pdf: bool,
    filename: Option<String>,
}

async fn search_pdf_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    person_id: Option<&str>,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let upload_path = pdf_upload_path(&state.settings.upload_dir, filename);
    write_pdf_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let pdf = match decode_pdf(&upload_path, &state.settings) {
        Ok(pdf) => pdf,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process PDF: {error}"
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

    for page in &pdf.pages {
        let mut response = service
            .search_media(&page.media, limit, ocr_text, person_id)
            .await
            .map_err(ApiError::internal)?;
        for result in &mut response.results {
            result.query_scene_index = Some(page.page_index);
        }
        flattened.extend(response.results.clone());
        scene_responses.push(SearchSceneResponse {
            scene_index: page.page_index,
            scene_kind: "pdf_page".to_string(),
            start_frame: page.page_number as u64,
            end_frame: page.page_number as u64,
            start_seconds: 0.0,
            end_seconds: 0.0,
            clip_url: None,
            page_index: Some(page.page_index),
            page_number: Some(page.page_number),
            page_label: Some(format!("Page {}", page.page_number)),
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
        query_media_kind: "pdf".to_string(),
        scenes: scene_responses,
        query_audio_analysis: None,
        query_ocr_text: normalize_ocr_query(ocr_text),
    })
}

async fn search_audio_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    person_id: Option<&str>,
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
            .search_media(&segment.media, limit, ocr_text, person_id)
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
            page_index: None,
            page_number: None,
            page_label: None,
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
    person_id: Option<&str>,
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
            .search_media(&scene.media, limit, ocr_text, person_id)
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
            page_index: None,
            page_number: None,
            page_label: None,
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

fn run_index_job(
    context: JobContext,
    settings: Settings,
    store: QdrantImageStore,
    embedder: Arc<dyn VisualEmbeddingBackend>,
) -> jobs_core::Result<()> {
    context.info("checking indexed media sources")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(job_failed)?;
    let indexer = ImageIndexer::new(settings, store, embedder);
    let response = runtime.block_on(indexer.index_missing_sources(Some(&context)));

    for error in &response.errors {
        context.warn(error.clone())?;
    }
    if context.is_cancelled() {
        return Err(JobError::Cancelled);
    }
    if response.failed > 0 {
        return Err(JobError::Failed(format!(
            "indexing finished with {} failed source file(s)",
            response.failed
        )));
    }
    Ok(())
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
