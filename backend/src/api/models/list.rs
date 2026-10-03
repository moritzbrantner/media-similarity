use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::api::AppState;
use crate::workers::media::models::model_statuses;

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

pub async fn get_models(State(state): State<Arc<AppState>>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        models: model_statuses(&state.indexing_settings()),
    })
}

pub async fn audio_transcription_models(
    State(state): State<Arc<AppState>>,
) -> Json<AudioTranscriptionModelsResponse> {
    let status = crate::workers::media::models::model_status(
        crate::workers::media::models::ModelRole::AudioTranscription,
        &state.settings,
    );
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

    Json(AudioTranscriptionModelsResponse {
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
    })
}
