use std::fs;
use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ApiError, AppState};
use crate::config::{parse_extensions, parse_media_sources_file, Settings};

#[derive(Debug, Serialize)]
pub struct SourceConfigResponse {
    pub media_sources_file: String,
    pub media_sources_seed_file: Option<String>,
    pub media_sources_writable: bool,
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
    pub face_min_cluster_images: u32,
    pub face_max_frames_per_media: usize,
    pub gif_sample_frames: usize,
    pub gif_max_decode_frames: usize,
    pub gif_preview_frames: usize,
    pub gif_default_frame_delay_ms: u32,
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
    pub sources: Option<Vec<String>>,
    pub indexing: Option<EditableIndexingConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EditableIndexingConfig {
    pub image_extensions: Vec<String>,
    pub audio_extensions: Vec<String>,
    pub pdf_extensions: Vec<String>,
    pub face_analysis_enabled: bool,
    pub face_detection_min_confidence: f32,
    pub face_cluster_threshold: f32,
    pub face_min_cluster_images: u32,
    pub face_max_frames_per_media: usize,
    pub gif_sample_frames: usize,
    pub gif_max_decode_frames: usize,
    pub gif_preview_frames: usize,
    pub gif_default_frame_delay_ms: u32,
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

impl EditableIndexingConfig {
    pub(super) fn from_settings(settings: &Settings) -> Self {
        Self {
            image_extensions: settings.image_extensions.iter().cloned().collect(),
            audio_extensions: settings.audio_extensions.iter().cloned().collect(),
            pdf_extensions: settings.pdf_extensions.iter().cloned().collect(),
            face_analysis_enabled: settings.face_analysis_enabled,
            face_detection_min_confidence: settings.face_detection_min_confidence,
            face_cluster_threshold: settings.face_cluster_threshold,
            face_min_cluster_images: settings.face_min_cluster_images,
            face_max_frames_per_media: settings.face_max_frames_per_media,
            gif_sample_frames: settings.gif_sample_frames,
            gif_max_decode_frames: settings.gif_max_decode_frames,
            gif_preview_frames: settings.gif_preview_frames,
            gif_default_frame_delay_ms: settings.gif_default_frame_delay_ms,
            gif_motion_weight: settings.gif_motion_weight,
            video_frame_stride: settings.video_frame_stride,
            video_max_frames: settings.video_max_frames,
            pdf_render_dpi: settings.pdf_render_dpi,
            pdf_max_pages: settings.pdf_max_pages,
            pdf_summary_pages: settings.pdf_summary_pages,
            ocr_enabled: settings.ocr_enabled,
            ocr_max_frames: settings.ocr_max_frames,
            audio_transcription_enabled: settings.audio_transcription_enabled,
        }
    }

    pub(super) fn apply_to_settings(&self, settings: &mut Settings) {
        settings.image_extensions = parse_extensions(&self.image_extensions.join(","))
            .expect("validated indexing config contains image extensions");
        settings.audio_extensions = parse_extensions(&self.audio_extensions.join(","))
            .expect("validated indexing config contains audio extensions");
        settings.pdf_extensions = parse_extensions(&self.pdf_extensions.join(","))
            .expect("validated indexing config contains PDF extensions");
        settings.face_analysis_enabled = self.face_analysis_enabled;
        settings.face_detection_min_confidence = self.face_detection_min_confidence;
        settings.face_cluster_threshold = self.face_cluster_threshold;
        settings.face_min_cluster_images = self.face_min_cluster_images;
        settings.face_max_frames_per_media = self.face_max_frames_per_media;
        settings.gif_sample_frames = self.gif_sample_frames;
        settings.gif_max_decode_frames = self.gif_max_decode_frames;
        settings.gif_preview_frames = self.gif_preview_frames;
        settings.gif_default_frame_delay_ms = self.gif_default_frame_delay_ms;
        settings.gif_motion_weight = self.gif_motion_weight;
        settings.video_frame_stride = self.video_frame_stride;
        settings.video_max_frames = self.video_max_frames;
        settings.pdf_render_dpi = self.pdf_render_dpi;
        settings.pdf_max_pages = self.pdf_max_pages;
        settings.pdf_summary_pages = self.pdf_summary_pages;
        settings.ocr_enabled = self.ocr_enabled;
        settings.ocr_max_frames = self.ocr_max_frames;
        settings.audio_transcription_enabled = self.audio_transcription_enabled;
    }

    fn validated(mut self) -> Result<Self, ApiError> {
        self.image_extensions = normalized_extensions("image_extensions", &self.image_extensions)?;
        self.audio_extensions = normalized_extensions("audio_extensions", &self.audio_extensions)?;
        self.pdf_extensions = normalized_extensions("pdf_extensions", &self.pdf_extensions)?;
        validate_range(
            "face_detection_min_confidence",
            self.face_detection_min_confidence,
            0.0,
            1.0,
        )?;
        validate_range(
            "face_cluster_threshold",
            self.face_cluster_threshold,
            0.0,
            2.0,
        )?;
        validate_min("face_min_cluster_images", self.face_min_cluster_images, 1)?;
        validate_min_usize(
            "face_max_frames_per_media",
            self.face_max_frames_per_media,
            1,
        )?;
        validate_min_usize("gif_sample_frames", self.gif_sample_frames, 1)?;
        validate_min_usize("gif_max_decode_frames", self.gif_max_decode_frames, 1)?;
        validate_min_usize("gif_preview_frames", self.gif_preview_frames, 1)?;
        validate_min(
            "gif_default_frame_delay_ms",
            self.gif_default_frame_delay_ms,
            1,
        )?;
        validate_range("gif_motion_weight", self.gif_motion_weight, 0.0, 1.0)?;
        validate_min("video_frame_stride", self.video_frame_stride, 1)?;
        if let Some(video_max_frames) = self.video_max_frames {
            validate_min("video_max_frames", video_max_frames, 1)?;
        }
        validate_range_u32("pdf_render_dpi", self.pdf_render_dpi, 72, 300)?;
        validate_range_u32("pdf_max_pages", self.pdf_max_pages, 1, 10_000)?;
        validate_range_usize("pdf_summary_pages", self.pdf_summary_pages, 1, 256)?;
        validate_range_usize("ocr_max_frames", self.ocr_max_frames, 1, 64)?;
        Ok(self)
    }
}

pub async fn get_source_config(State(state): State<Arc<AppState>>) -> Json<SourceConfigResponse> {
    Json(source_config_response(&state))
}

pub async fn update_source_config(
    State(state): State<Arc<AppState>>,
    Json(request): Json<UpdateSourceConfigRequest>,
) -> Result<Json<SourceConfigResponse>, ApiError> {
    if let Some(sources) = request.sources {
        let sources = normalize_source_specs(&sources)?;
        write_media_sources_file(&state.settings.media_sources_file, &sources)?;
        state.replace_source_specs(sources);
    }
    if let Some(indexing) = request.indexing {
        state.replace_indexing_config(indexing.validated()?);
    }
    Ok(Json(source_config_response(&state)))
}

fn source_config_response(state: &AppState) -> SourceConfigResponse {
    let settings = state.indexing_settings();
    SourceConfigResponse {
        media_sources_file: settings.media_sources_file.to_string_lossy().to_string(),
        media_sources_seed_file: settings
            .media_sources_seed_file
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        media_sources_writable: media_sources_file_is_writable(&settings.media_sources_file),
        default_source_dir: settings.source_image_dir.to_string_lossy().to_string(),
        sources: settings
            .source_specs()
            .into_iter()
            .map(|spec| source_config_source(spec, &settings))
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
            face_min_cluster_images: settings.face_min_cluster_images,
            face_max_frames_per_media: settings.face_max_frames_per_media,
            gif_sample_frames: settings.gif_sample_frames,
            gif_max_decode_frames: settings.gif_max_decode_frames,
            gif_preview_frames: settings.gif_preview_frames,
            gif_default_frame_delay_ms: settings.gif_default_frame_delay_ms,
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

fn normalized_extensions(name: &str, values: &[String]) -> Result<Vec<String>, ApiError> {
    let normalized = parse_extensions(&values.join(",")).map_err(|error| {
        ApiError::bad_request(format!(
            "{name} must contain at least one extension: {error}"
        ))
    })?;
    Ok(normalized.into_iter().collect())
}

fn validate_range(name: &str, value: f32, min: f32, max: f32) -> Result<(), ApiError> {
    if value.is_finite() && value >= min && value <= max {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "{name} must be between {min} and {max}"
        )))
    }
}

fn validate_range_u32(name: &str, value: u32, min: u32, max: u32) -> Result<(), ApiError> {
    if value >= min && value <= max {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "{name} must be between {min} and {max}"
        )))
    }
}

fn validate_range_usize(name: &str, value: usize, min: usize, max: usize) -> Result<(), ApiError> {
    if value >= min && value <= max {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "{name} must be between {min} and {max}"
        )))
    }
}

fn validate_min(name: &str, value: u32, min: u32) -> Result<(), ApiError> {
    if value >= min {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "{name} must be at least {min}"
        )))
    }
}

fn validate_min_usize(name: &str, value: usize, min: usize) -> Result<(), ApiError> {
    if value >= min {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "{name} must be at least {min}"
        )))
    }
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

fn media_sources_file_is_writable(path: &std::path::Path) -> bool {
    if path.is_file() {
        return fs::OpenOptions::new().append(true).open(path).is_ok();
    }

    let Some(parent) = path.parent() else {
        return false;
    };
    if fs::create_dir_all(parent).is_err() {
        return false;
    }
    let probe = parent.join(format!(
        ".media-sources-writable-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

pub(crate) fn source_config_source(spec: String, settings: &Settings) -> SourceConfigSource {
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
        "minio" | "s3" => object_source_config_status(&spec, &kind, settings),
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

fn object_source_config_status(
    spec: &str,
    kind: &str,
    settings: &Settings,
) -> (String, Option<String>) {
    let Ok(url) = url::Url::parse(spec) else {
        return (
            "unavailable".to_string(),
            Some(format!("Invalid object-store source URI: {spec}")),
        );
    };
    if url.host_str().filter(|bucket| !bucket.is_empty()).is_none() {
        return (
            "unavailable".to_string(),
            Some(format!("Missing bucket in object-store source URI: {spec}")),
        );
    }

    let endpoint = match kind {
        "minio" => settings
            .minio_endpoint
            .clone()
            .or_else(|| settings.s3_endpoint.clone()),
        "s3" => settings
            .s3_endpoint
            .clone()
            .or_else(|| settings.minio_endpoint.clone()),
        _ => None,
    };
    let access_key = match kind {
        "minio" => settings
            .minio_access_key
            .clone()
            .or_else(|| settings.s3_access_key_id.clone()),
        "s3" => settings
            .s3_access_key_id
            .clone()
            .or_else(|| settings.minio_access_key.clone()),
        _ => None,
    };
    let secret_key = match kind {
        "minio" => settings
            .minio_secret_key
            .clone()
            .or_else(|| settings.s3_secret_access_key.clone()),
        "s3" => settings
            .s3_secret_access_key
            .clone()
            .or_else(|| settings.minio_secret_key.clone()),
        _ => None,
    };

    if kind == "minio" && endpoint.is_none() {
        return (
            "unavailable".to_string(),
            Some("MINIO_ENDPOINT or S3_ENDPOINT is required for MinIO sources".to_string()),
        );
    }
    if endpoint.is_some() && (access_key.is_none() || secret_key.is_none()) {
        return (
            "unavailable".to_string(),
            Some(format!(
                "{} object-store credentials are incomplete",
                kind.to_ascii_uppercase()
            )),
        );
    }

    ("ready".to_string(), None)
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
            implemented: true,
            example: "minio://bucket/prefix".to_string(),
        },
        SupportedSourceType {
            kind: "s3".to_string(),
            label: "S3 bucket".to_string(),
            implemented: true,
            example: "s3://bucket/prefix".to_string(),
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

#[cfg(test)]
mod tests {
    use super::{normalize_source_specs, source_config_source, EditableIndexingConfig};
    use crate::config::Settings;

    #[test]
    fn editable_indexing_config_normalizes_extensions_and_rejects_empty_lists() {
        let mut config = EditableIndexingConfig::from_settings(&Settings::default());
        config.image_extensions = vec!["png".to_string(), ".JPG".to_string()];
        config.audio_extensions = vec!["mp3".to_string()];
        config.pdf_extensions = vec!["pdf".to_string()];

        let validated = config.clone().validated().unwrap();

        assert_eq!(validated.image_extensions, vec![".jpg", ".png"]);
        assert_eq!(validated.audio_extensions, vec![".mp3"]);
        assert_eq!(validated.pdf_extensions, vec![".pdf"]);

        config.image_extensions.clear();
        let error = config.validated().unwrap_err();
        assert!(error.detail.contains("image_extensions"));
    }

    #[test]
    fn source_specs_reject_empty_input_and_report_unsupported_kinds() {
        let error = normalize_source_specs(&["  ".to_string()]).unwrap_err();
        assert!(error.detail.contains("At least one media source"));

        let source = source_config_source(
            "ftp://example.test/archive".to_string(),
            &Settings::default(),
        );
        assert_eq!(source.kind, "ftp");
        assert_eq!(source.status, "unsupported");
        assert!(source.detail.unwrap().contains("Unsupported media source"));
    }
}
