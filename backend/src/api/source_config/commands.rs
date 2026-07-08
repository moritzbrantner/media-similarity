use std::fs;
use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use super::contracts::{
    EditableIndexingConfig, SourceConfigResponse, SourceIndexingConfig, SourceInventory,
    SourceInventoryItem, SourcePreview, SourcePreviewRequest, SourcePreviewResponse,
    UpdateSourceConfigRequest,
};
use super::queries::{source_config_source, supported_source_types, video_source_extensions};
use crate::api::ApiError;
use crate::api::AppState;
use crate::config::{parse_extensions, parse_media_sources_file};
use crate::workers::media::models::{model_status, ModelRole};
use crate::workers::sources::{build_media_sources, SourceDiagnostic, SourceDiagnosticCode};

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

pub async fn preview_source_config(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SourcePreviewRequest>,
) -> Result<Json<SourcePreviewResponse>, ApiError> {
    let sources = normalize_source_specs(&request.sources)?;
    let mut settings = state.indexing_settings();
    settings.image_sources = sources.clone();
    let limit = request.limit_per_source.unwrap_or(25).clamp(1, 500);
    let mut previews = Vec::new();

    for spec in sources {
        let mut source = source_config_source(spec.clone(), &settings);
        let provider_settings = crate::config::Settings {
            image_sources: vec![spec],
            ..settings.clone()
        };
        let provider = build_media_sources(&provider_settings).into_iter().next();
        let inventory = match provider {
            Some(provider) if source.capabilities.enumerates_items => {
                match provider.iter_items().await {
                    Ok(items) => Some(source_inventory(&settings, &items, limit, &mut source)),
                    Err(error) => {
                        source.diagnostics.push(SourceDiagnostic::error(
                            SourceDiagnosticCode::EnumerationFailed,
                            error.0,
                        ));
                        source.status = super::queries::source_status(
                            &source.diagnostics,
                            &source.capabilities,
                        );
                        source.detail = source
                            .diagnostics
                            .first()
                            .map(|diagnostic| diagnostic.message.clone());
                        None
                    }
                }
            }
            _ => None,
        };
        previews.push(SourcePreview { source, inventory });
    }

    Ok(Json(SourcePreviewResponse { sources: previews }))
}

pub fn source_config_response(state: &AppState) -> SourceConfigResponse {
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

fn source_inventory(
    settings: &crate::config::Settings,
    items: &[crate::workers::sources::SourceImage],
    limit: usize,
    source: &mut super::contracts::SourceConfigSource,
) -> SourceInventory {
    let scanned_count = items.len();
    let truncated = scanned_count > limit;
    let mut media_kind_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut extension_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut required_model_roles = std::collections::BTreeSet::<String>::new();
    let mut sample_items = Vec::new();

    for item in items {
        let media_kind = item.media_kind().to_string();
        *media_kind_counts.entry(media_kind.clone()).or_default() += 1;
        if let Some(extension) = item.extension() {
            *extension_counts.entry(extension).or_default() += 1;
        }
        required_model_roles.insert(ModelRole::VisualEmbedding.as_str().to_string());
        if settings.face_analysis_enabled {
            required_model_roles.insert(ModelRole::FaceDetection.as_str().to_string());
            required_model_roles.insert(ModelRole::FaceEmbedding.as_str().to_string());
        }
        if settings.audio_transcription_enabled && (item.is_audio() || item.is_video()) {
            required_model_roles.insert(ModelRole::AudioTranscription.as_str().to_string());
        }
        if sample_items.len() < limit {
            sample_items.push(SourceInventoryItem {
                item_uri: item.item_uri.clone(),
                relative_path: item.relative_path.clone(),
                media_kind,
                size_bytes: item.size_bytes,
                modified_at: item.modified_at,
            });
        }
    }

    if items.is_empty() {
        source.diagnostics.push(SourceDiagnostic::warning(
            SourceDiagnosticCode::EmptySource,
            "No supported source items found in preview",
        ));
    }

    let degraded_model_roles = required_model_roles
        .iter()
        .filter(|role| {
            role.parse::<ModelRole>()
                .ok()
                .map(|role| !model_status(role, settings).active)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    for role in &degraded_model_roles {
        source.diagnostics.push(SourceDiagnostic::warning(
            SourceDiagnosticCode::ModelFeatureUnavailable,
            format!("Model role `{role}` is not active; related analysis will be degraded"),
        ));
    }
    source.status = super::queries::source_status(&source.diagnostics, &source.capabilities);
    source.detail = source
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone());

    SourceInventory {
        scanned_count,
        truncated,
        media_kind_counts,
        extension_counts,
        sample_items,
        required_model_roles: required_model_roles.into_iter().collect(),
        degraded_model_roles,
    }
}

impl EditableIndexingConfig {
    pub(super) fn validated(mut self) -> Result<Self, ApiError> {
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
    content.push_str(
        "# One source per line. Supported now: local paths, file://, local://, s3://, minio://.\n",
    );
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
        uuid::Uuid::new_v4()
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

#[cfg(test)]
mod tests {
    use super::{normalize_source_specs, EditableIndexingConfig};
    use crate::api::source_config::contracts::SourceConfigResponse;
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
    }

    #[test]
    fn indexed_source_config_response_is_buildable() {
        let _response = SourceConfigResponse {
            media_sources_file: String::new(),
            media_sources_seed_file: None,
            media_sources_writable: false,
            default_source_dir: String::new(),
            sources: Vec::new(),
            supported_source_types: Vec::new(),
            indexing: crate::api::source_config::contracts::SourceIndexingConfig {
                collection: String::new(),
                image_extensions: Vec::new(),
                audio_extensions: Vec::new(),
                pdf_extensions: Vec::new(),
                video_extensions: Vec::new(),
                visual_embedding_enabled: false,
                visual_embedding_model: String::new(),
                visual_embedding_vector_size: 0,
                face_analysis_enabled: false,
                face_detection_min_confidence: 0.0,
                face_cluster_threshold: 0.0,
                face_min_cluster_images: 0,
                face_max_frames_per_media: 0,
                gif_sample_frames: 0,
                gif_max_decode_frames: 0,
                gif_preview_frames: 0,
                gif_default_frame_delay_ms: 0,
                gif_motion_weight: 0.0,
                video_frame_stride: 0,
                video_max_frames: None,
                pdf_render_dpi: 0,
                pdf_max_pages: 0,
                pdf_summary_pages: 0,
                ocr_enabled: false,
                ocr_max_frames: 0,
                audio_transcription_enabled: false,
            },
        };
    }
}
