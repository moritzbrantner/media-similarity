use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::domain::models::HealthResponse;
use crate::workers::sources::build_image_sources;

use super::AppState;

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
