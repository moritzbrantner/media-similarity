use std::collections::BTreeSet;
use std::sync::Arc;

use axum::extract::{Path as AxumPath, Query, State};
use axum::Json;
use serde::Deserialize;

use super::{ApiError, AppState};
use crate::domain::models::ImagePayload;
use crate::workers::deletion::{
    delete_indexed_media, delete_indexed_source, DeleteIndexResponse, DeleteIndexedSourceFilter,
};

const MAX_MEDIA_TAGS: usize = 64;
const MAX_MEDIA_TAG_LENGTH: usize = 80;

pub async fn delete_indexed_media_route(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<DeleteIndexResponse>, ApiError> {
    let response =
        delete_indexed_media(&state.indexing_settings(), state.store.as_ref(), &id).await;
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
pub struct UpdateMediaTagsRequest {
    tags: Vec<String>,
}

pub async fn update_indexed_media_tags_route(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(request): Json<UpdateMediaTagsRequest>,
) -> Result<Json<ImagePayload>, ApiError> {
    let tags = normalize_media_tags(request.tags)?;
    let point = state
        .store
        .scroll_media_points_by_filter(Some(&id), None, None)
        .await
        .map_err(ApiError::internal)?
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::not_found(format!("Unknown indexed media `{id}`")))?;
    let payload_value = point
        .payload
        .ok_or_else(|| ApiError::internal(format!("Indexed media `{id}` has no payload")))?;
    let mut payload = serde_json::from_value::<ImagePayload>(payload_value)
        .map_err(|error| ApiError::internal(format!("could not decode media payload: {error}")))?;

    payload.tags = tags;
    state
        .store
        .set_media_payload(&payload)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(payload))
}

pub async fn delete_indexed_sources_route(
    State(state): State<Arc<AppState>>,
    Query(filter): Query<DeleteIndexedSourceFilter>,
) -> Result<Json<DeleteIndexResponse>, ApiError> {
    if filter.source_uri.is_none() && filter.source_item_uri.is_none() {
        return Err(ApiError::bad_request(
            "source_uri or source_item_uri is required",
        ));
    }
    let response =
        delete_indexed_source(&state.indexing_settings(), state.store.as_ref(), filter).await;
    Ok(Json(response))
}

fn normalize_media_tags(tags: Vec<String>) -> Result<Vec<String>, ApiError> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();

    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        if tag.chars().any(char::is_control) {
            return Err(ApiError::bad_request(
                "Tags cannot contain control characters",
            ));
        }
        if tag.chars().count() > MAX_MEDIA_TAG_LENGTH {
            return Err(ApiError::bad_request(format!(
                "Tags must be {MAX_MEDIA_TAG_LENGTH} characters or fewer"
            )));
        }
        if seen.insert(tag.to_lowercase()) {
            normalized.push(tag.to_string());
        }
    }

    if normalized.len() > MAX_MEDIA_TAGS {
        return Err(ApiError::bad_request(format!(
            "Media can have at most {MAX_MEDIA_TAGS} tags"
        )));
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::normalize_media_tags;

    #[test]
    fn media_tags_are_trimmed_and_deduplicated() {
        assert_eq!(
            normalize_media_tags(vec![
                " travel ".to_string(),
                "Travel".to_string(),
                "".to_string(),
                "archive".to_string(),
            ])
            .unwrap(),
            vec!["travel", "archive"]
        );
    }

    #[test]
    fn media_tags_reject_control_characters() {
        let error = normalize_media_tags(vec!["bad\ntag".to_string()]).unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
    }
}
