use std::sync::Arc;

use axum::extract::{Multipart, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::config::Settings;
use crate::embedder::ImageEmbedder;
use crate::image_io::load_media_bytes;
use crate::indexer::ImageIndexer;
use crate::models::{HealthResponse, IndexResponse, SearchResponse};
use crate::qdrant::QdrantImageStore;
use crate::search::ImageSearchService;
use crate::sources::build_image_sources;

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
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let content_type = field.content_type().unwrap_or_default().to_string();
        if !content_type.starts_with("image/") {
            return Err(ApiError::bad_request("Upload must be an image file"));
        }
        let raw = field
            .bytes()
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        uploaded = Some(raw);
        break;
    }

    let raw = uploaded.ok_or_else(|| ApiError::bad_request("Upload must be an image file"))?;
    let max_bytes = state.settings.max_upload_mb as usize * 1024 * 1024;
    if raw.len() > max_bytes {
        return Err(ApiError::payload_too_large(format!(
            "Upload is larger than {} MB",
            state.settings.max_upload_mb
        )));
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
