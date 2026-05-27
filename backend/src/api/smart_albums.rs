use std::sync::Arc;

use axum::extract::{Path as AxumPath, Query, State};
use axum::Json;
use chrono::Utc;
use serde::Deserialize;

use super::{ApiError, AppState};
use crate::domain::smart_albums::{EditableSmartAlbum, SmartAlbum, SmartAlbumResultsResponse};
use crate::workers::albums::smart_album_results;
use crate::workers::smart_albums::{
    create_smart_album, delete_smart_album, get_smart_album, list_smart_albums, update_smart_album,
    validate_editable_album, DeleteSmartAlbumResponse, SmartAlbumListResponse,
};

#[derive(Debug, Deserialize)]
pub struct AlbumResultsQuery {
    offset: Option<usize>,
    limit: Option<u32>,
}

pub async fn list_albums(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SmartAlbumListResponse>, ApiError> {
    list_smart_albums(&state.settings.smart_albums_file)
        .map(Json)
        .map_err(album_error)
}

pub async fn create_album(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EditableSmartAlbum>,
) -> Result<Json<SmartAlbum>, ApiError> {
    create_smart_album(&state.settings.smart_albums_file, request)
        .map(Json)
        .map_err(album_error)
}

pub async fn update_album(
    State(state): State<Arc<AppState>>,
    AxumPath(album_id): AxumPath<String>,
    Json(request): Json<EditableSmartAlbum>,
) -> Result<Json<SmartAlbum>, ApiError> {
    update_smart_album(&state.settings.smart_albums_file, &album_id, request)
        .map(Json)
        .map_err(album_error)
}

pub async fn delete_album(
    State(state): State<Arc<AppState>>,
    AxumPath(album_id): AxumPath<String>,
) -> Result<Json<DeleteSmartAlbumResponse>, ApiError> {
    delete_smart_album(&state.settings.smart_albums_file, &album_id)
        .map(Json)
        .map_err(album_error)
}

pub async fn album_results(
    State(state): State<Arc<AppState>>,
    AxumPath(album_id): AxumPath<String>,
    Query(query): Query<AlbumResultsQuery>,
) -> Result<Json<SmartAlbumResultsResponse>, ApiError> {
    let album =
        get_smart_album(&state.settings.smart_albums_file, &album_id).map_err(album_error)?;
    smart_album_results(
        &state.indexing_settings(),
        state.store.as_ref(),
        album,
        query.offset.unwrap_or(0),
        query.limit,
    )
    .await
    .map(Json)
    .map_err(results_error)
}

pub async fn preview_album(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AlbumResultsQuery>,
    Json(request): Json<EditableSmartAlbum>,
) -> Result<Json<SmartAlbumResultsResponse>, ApiError> {
    let request = validate_editable_album(request).map_err(ApiError::bad_request)?;
    let now = Utc::now().to_rfc3339();
    let album = SmartAlbum {
        id: "preview".to_string(),
        name: request.name,
        description: request.description,
        criteria: request.criteria,
        sort: request.sort,
        limit: request.limit,
        created_at: now.clone(),
        updated_at: now,
    };
    smart_album_results(
        &state.indexing_settings(),
        state.store.as_ref(),
        album,
        query.offset.unwrap_or(0),
        query.limit,
    )
    .await
    .map(Json)
    .map_err(results_error)
}

fn album_error(error: String) -> ApiError {
    if error.starts_with("Unknown smart album") {
        ApiError::not_found(error)
    } else if error.starts_with("Could not read")
        || error.starts_with("Could not parse")
        || error.starts_with("Could not create smart albums")
        || error.starts_with("Could not write")
        || error.starts_with("Could not flush")
        || error.starts_with("Could not replace")
        || error.starts_with("Unsupported smart albums")
    {
        ApiError::internal(error)
    } else {
        ApiError::bad_request(error)
    }
}

fn results_error(error: String) -> ApiError {
    if error.contains("duplicate grouping is capped") {
        ApiError::bad_request(error)
    } else {
        ApiError::internal(error)
    }
}
