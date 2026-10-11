use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::api::{ApiError, AppState};
use crate::workers::media::models::{model_status, model_statuses, ModelRole};

#[derive(Debug, Serialize)]
pub struct AudioTranscriptionModelsResponse {
    pub enabled: bool,
    pub provider: String,
    pub configured_model: String,
    pub device: String,
    pub compute_type: String,
    pub language: Option<String>,
    pub batch_chunks: bool,
    pub max_batch_size: Option<usize>,
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

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub models: Vec<crate::workers::media::models::ModelRuntimeStatus>,
}

/// Full model status verifies cached files against recorded checksums, which
/// can hash multi-GB bundles on a cold cache, so it runs off the async workers.
async fn blocking_status<T: Send + 'static>(
    compute: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(compute).await.map_err(|error| {
        ApiError::service_unavailable(format!("model status check failed: {error}"))
    })
}

pub async fn get_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ModelsResponse>, ApiError> {
    let settings = state.indexing_settings();
    let models = blocking_status(move || model_statuses(&settings)).await?;
    Ok(Json(ModelsResponse { models }))
}

pub async fn audio_transcription_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AudioTranscriptionModelsResponse>, ApiError> {
    let settings = state.settings.clone();
    let status =
        blocking_status(move || model_status(ModelRole::AudioTranscription, &settings)).await?;
    let configured_model = status.configured.clone();
    let models = status
        .options
        .into_iter()
        .map(|model| AudioTranscriptionModelResponse {
            cached: model.cached,
            configured: model.configured,
            id: model.id,
        })
        .collect();

    Ok(Json(AudioTranscriptionModelsResponse {
        enabled: state.settings.audio_transcription_enabled,
        provider: state.settings.audio_transcription_provider.clone(),
        configured_model,
        device: state.settings.audio_transcription_device.clone(),
        compute_type: state.settings.audio_transcription_compute_type.clone(),
        language: state.settings.audio_transcription_language.clone(),
        batch_chunks: state.settings.audio_transcription_batch_chunks,
        max_batch_size: state.settings.audio_transcription_max_batch_size,
        auto_download: state.settings.audio_transcription_auto_download,
        cache_dir: Some(
            state
                .settings
                .model_bundle_dir
                .to_string_lossy()
                .to_string(),
        ),
        models,
    }))
}
