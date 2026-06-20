use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::api::AppState;
use crate::workers::media::models::model_statuses;

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
    let store = crate::workers::media::models::audio_transcription_model_store(&state.settings);
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
