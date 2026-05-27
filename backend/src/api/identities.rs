use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use super::{ApiError, AppState};
use crate::workers::identities::{
    merge_person_identities, merge_speaker_identities, normalize_label, normalize_source_ids,
    rename_person_identity, rename_speaker_identity, IdentityMutationError,
    IdentityMutationResponse,
};

#[derive(Debug, Deserialize)]
pub struct RenameIdentityRequest {
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MergeIdentityRequest {
    source_ids: Option<Vec<String>>,
}

pub async fn rename_person(
    State(state): State<Arc<AppState>>,
    Path(person_id): Path<String>,
    Json(request): Json<RenameIdentityRequest>,
) -> Result<Json<IdentityMutationResponse>, ApiError> {
    let target_id = normalized_path_id(&person_id)?;
    let label = normalize_label(request.label).map_err(ApiError::bad_request)?;
    rename_person_identity(state.store.as_ref(), &target_id, &label)
        .await
        .map(Json)
        .map_err(identity_error)
}

pub async fn merge_people(
    State(state): State<Arc<AppState>>,
    Path(target_person_id): Path<String>,
    Json(request): Json<MergeIdentityRequest>,
) -> Result<Json<IdentityMutationResponse>, ApiError> {
    let target_id = normalized_path_id(&target_person_id)?;
    let source_ids =
        normalize_source_ids(&target_id, request.source_ids).map_err(ApiError::bad_request)?;
    merge_person_identities(state.store.as_ref(), &target_id, &source_ids)
        .await
        .map(Json)
        .map_err(identity_error)
}

pub async fn rename_speaker(
    State(state): State<Arc<AppState>>,
    Path(speaker_id): Path<String>,
    Json(request): Json<RenameIdentityRequest>,
) -> Result<Json<IdentityMutationResponse>, ApiError> {
    let target_id = normalized_path_id(&speaker_id)?;
    let label = normalize_label(request.label).map_err(ApiError::bad_request)?;
    rename_speaker_identity(state.store.as_ref(), &state.settings, &target_id, &label)
        .await
        .map(Json)
        .map_err(identity_error)
}

pub async fn merge_speakers(
    State(state): State<Arc<AppState>>,
    Path(target_speaker_id): Path<String>,
    Json(request): Json<MergeIdentityRequest>,
) -> Result<Json<IdentityMutationResponse>, ApiError> {
    let target_id = normalized_path_id(&target_speaker_id)?;
    let source_ids =
        normalize_source_ids(&target_id, request.source_ids).map_err(ApiError::bad_request)?;
    merge_speaker_identities(
        state.store.as_ref(),
        &state.settings,
        &target_id,
        &source_ids,
    )
    .await
    .map(Json)
    .map_err(identity_error)
}

fn normalized_path_id(id: &str) -> Result<String, ApiError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(ApiError::bad_request("identity id must not be empty"));
    }
    Ok(id.to_string())
}

fn identity_error(error: IdentityMutationError) -> ApiError {
    match error {
        IdentityMutationError::NotFound(message) => ApiError::not_found(message),
        IdentityMutationError::Internal(message) => ApiError::internal(message),
    }
}
