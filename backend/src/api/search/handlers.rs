use std::sync::Arc;

use axum::extract::{Multipart, Query, State};
use axum::Json;

use super::media_upload::{
    search_audio_upload, search_local_image, search_pdf_upload, search_video_upload,
};
use super::query::SearchQuery;
use crate::api::ApiError;
use crate::api::AppState;
use crate::domain::models::SearchResponse;
use crate::workers::media::audio::{is_audio_content_type, is_audio_extension};
use crate::workers::media::pdf::{is_pdf_content_type, is_pdf_extension};
use crate::workers::media::video::{is_video_content_type, is_video_extension};

pub async fn search_upload(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
    mut multipart: Multipart,
) -> Result<Json<SearchResponse>, ApiError> {
    let filters = query.search_filters()?;
    let indexing_settings = state.indexing_settings();
    let mut uploaded = None;
    let mut upload_kind = None;
    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        if field.name() != Some("file") {
            continue;
        }
        let content_type = field.content_type().unwrap_or_default().to_string();
        let filename = field.file_name().map(ToOwned::to_owned);
        let filename_extension = filename
            .as_deref()
            .and_then(|name| std::path::Path::new(name).extension())
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{}", extension.to_ascii_lowercase()));
        let is_image = content_type.starts_with("image/")
            || filename_extension
                .as_ref()
                .map(|extension| indexing_settings.image_extensions.contains(extension))
                .unwrap_or(false);
        let is_video = is_video_content_type(&content_type)
            || filename_extension
                .as_deref()
                .map(is_video_extension)
                .unwrap_or(false);
        let is_audio = is_audio_content_type(&content_type)
            || filename_extension
                .as_deref()
                .map(is_audio_extension)
                .unwrap_or(false);
        let is_pdf = is_pdf_content_type(&content_type)
            || filename_extension
                .as_deref()
                .map(is_pdf_extension)
                .unwrap_or(false);
        if !is_image && !is_video && !is_audio && !is_pdf {
            return Err(ApiError::bad_request(
                "Upload must be an image, video, audio, or PDF file",
            ));
        }
        let raw = field.bytes().await.map_err(multipart_error)?;
        uploaded = Some(raw);
        upload_kind = Some(super::UploadedFileKind {
            is_video,
            is_audio,
            is_pdf,
            filename,
        });
        break;
    }

    let raw = uploaded.ok_or_else(|| {
        ApiError::bad_request("Upload must be an image, video, audio, or PDF file")
    })?;
    let upload_kind = upload_kind.ok_or_else(|| {
        ApiError::bad_request("Upload must be an image, video, audio, or PDF file")
    })?;
    let max_bytes = indexing_settings.max_upload_mb as usize * 1024 * 1024;
    if raw.len() > max_bytes {
        return Err(ApiError::payload_too_large(format!(
            "Upload is larger than {} MB",
            indexing_settings.max_upload_mb
        )));
    }

    if upload_kind.is_video {
        return search_video_upload(
            Arc::clone(&state),
            query.limit,
            query.ocr_text.as_deref(),
            filters,
            &raw,
            upload_kind.filename.as_deref(),
        )
        .await
        .map(Json);
    }

    if upload_kind.is_audio {
        return search_audio_upload(
            Arc::clone(&state),
            query.limit,
            query.ocr_text.as_deref(),
            filters,
            &raw,
            upload_kind.filename.as_deref(),
        )
        .await
        .map(Json);
    }

    if upload_kind.is_pdf {
        return search_pdf_upload(
            Arc::clone(&state),
            query.limit,
            query.ocr_text.as_deref(),
            filters,
            &raw,
            upload_kind.filename.as_deref(),
        )
        .await
        .map(Json);
    }

    let response =
        search_local_image(state, query, raw, filters, upload_kind.filename.as_deref()).await?;
    Ok(Json(response))
}

fn multipart_error(error: axum::extract::multipart::MultipartError) -> ApiError {
    match error.status() {
        axum::http::StatusCode::PAYLOAD_TOO_LARGE => ApiError::payload_too_large(error.body_text()),
        _ => ApiError::bad_request(error.body_text()),
    }
}
