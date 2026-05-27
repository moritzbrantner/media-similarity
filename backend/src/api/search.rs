use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Multipart, Query, State};
use axum::Json;
use serde::Deserialize;

use super::{ApiError, AppState};
use crate::domain::models::{SearchResponse, SearchResult, SearchSceneResponse};
use crate::workers::media::audio::{
    audio_upload_path, decode_audio_segments, is_audio_content_type, is_audio_extension,
    write_audio_upload,
};
use crate::workers::media::image_io::load_media_bytes;
use crate::workers::media::ocr::normalize_ocr_query;
use crate::workers::media::pdf::{
    decode_pdf, is_pdf_content_type, is_pdf_extension, pdf_upload_path, write_pdf_upload,
};
use crate::workers::media::video::{
    decode_video_scenes, is_video_content_type, is_video_extension, video_upload_path,
    write_video_upload,
};
use crate::workers::search::{
    ImageSearchService, NearDuplicateFilter, OrientationFilter, SearchFilters,
};

#[derive(Deserialize)]
pub struct SearchQuery {
    limit: Option<u32>,
    ocr_text: Option<String>,
    person_id: Option<String>,
    source_type: Option<String>,
    media_kind: Option<String>,
    name_query: Option<String>,
    camera_query: Option<String>,
    keyword_query: Option<String>,
    has_gps: Option<String>,
    near_duplicate: Option<String>,
    orientation: Option<String>,
    min_width: Option<u32>,
    max_width: Option<u32>,
    min_height: Option<u32>,
    max_height: Option<u32>,
    min_size_bytes: Option<u64>,
    max_size_bytes: Option<u64>,
    modified_from: Option<f64>,
    modified_to: Option<f64>,
    captured_from: Option<f64>,
    captured_to: Option<f64>,
}

pub async fn search_upload(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
    mut multipart: Multipart,
) -> Result<Json<SearchResponse>, ApiError> {
    let filters = query.search_filters()?;
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
                .map(|extension| state.settings.image_extensions.contains(extension))
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
        upload_kind = Some(UploadedFileKind {
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
    let max_bytes = state.settings.max_upload_mb as usize * 1024 * 1024;
    if raw.len() > max_bytes {
        return Err(ApiError::payload_too_large(format!(
            "Upload is larger than {} MB",
            state.settings.max_upload_mb
        )));
    }

    if upload_kind.is_video {
        return search_video_upload(
            state,
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
            state,
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
            state,
            query.limit,
            query.ocr_text.as_deref(),
            filters,
            &raw,
            upload_kind.filename.as_deref(),
        )
        .await
        .map(Json);
    }

    let media = load_media_bytes(&raw, &state.settings)
        .map_err(|_| ApiError::bad_request("Could not decode image"))?;
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    service
        .search_media_filtered(&media, query.limit, query.ocr_text.as_deref(), filters)
        .await
        .map(Json)
        .map_err(search_error)
}

impl SearchQuery {
    fn search_filters(&self) -> Result<SearchFilters, ApiError> {
        Ok(SearchFilters {
            source_type: normalized_filter(self.source_type.as_deref())
                .filter(|value| value != "all"),
            media_kind: normalized_filter(self.media_kind.as_deref())
                .filter(|value| value != "all")
                .map(validate_media_kind)
                .transpose()?,
            name_query: normalized_filter(self.name_query.as_deref()),
            camera_query: normalized_filter(self.camera_query.as_deref()),
            keyword_query: normalized_filter(self.keyword_query.as_deref()),
            has_gps: parse_has_gps(self.has_gps.as_deref())?,
            near_duplicate: parse_near_duplicate(self.near_duplicate.as_deref())?,
            orientation: parse_orientation(self.orientation.as_deref())?,
            min_width: self.min_width,
            max_width: self.max_width,
            min_height: self.min_height,
            max_height: self.max_height,
            min_size_bytes: self.min_size_bytes,
            max_size_bytes: self.max_size_bytes,
            modified_from: validate_optional_seconds("modified_from", self.modified_from)?,
            modified_to: validate_optional_seconds("modified_to", self.modified_to)?,
            captured_from: validate_optional_seconds("captured_from", self.captured_from)?,
            captured_to: validate_optional_seconds("captured_to", self.captured_to)?,
            person_id: normalized_filter(self.person_id.as_deref()),
        })
    }
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn validate_media_kind(value: String) -> Result<String, ApiError> {
    match value.as_str() {
        "static_image" | "animated_gif" | "video_scene" | "audio" | "pdf_page"
        | "pdf_document" => Ok(value),
        _ => Err(ApiError::bad_request(
            "media_kind must be one of all, static_image, animated_gif, video_scene, audio, pdf_page, pdf_document",
        )),
    }
}

fn parse_has_gps(value: Option<&str>) -> Result<Option<bool>, ApiError> {
    match normalized_filter(value).as_deref() {
        None | Some("all") => Ok(None),
        Some("yes") => Ok(Some(true)),
        Some("no") => Ok(Some(false)),
        Some(_) => Err(ApiError::bad_request("has_gps must be one of all, yes, no")),
    }
}

fn parse_near_duplicate(value: Option<&str>) -> Result<Option<NearDuplicateFilter>, ApiError> {
    match normalized_filter(value).as_deref() {
        None | Some("all") => Ok(None),
        Some("only") => Ok(Some(NearDuplicateFilter::Only)),
        Some("exclude") => Ok(Some(NearDuplicateFilter::Exclude)),
        Some(_) => Err(ApiError::bad_request(
            "near_duplicate must be one of all, only, exclude",
        )),
    }
}

fn parse_orientation(value: Option<&str>) -> Result<Option<OrientationFilter>, ApiError> {
    match normalized_filter(value).as_deref() {
        None | Some("all") => Ok(None),
        Some("landscape") => Ok(Some(OrientationFilter::Landscape)),
        Some("portrait") => Ok(Some(OrientationFilter::Portrait)),
        Some("square") => Ok(Some(OrientationFilter::Square)),
        Some(_) => Err(ApiError::bad_request(
            "orientation must be one of all, landscape, portrait, square",
        )),
    }
}

fn validate_optional_seconds(name: &str, value: Option<f64>) -> Result<Option<f64>, ApiError> {
    match value {
        Some(value) if !value.is_finite() || value < 0.0 => Err(ApiError::bad_request(format!(
            "{name} must be a non-negative Unix timestamp in seconds"
        ))),
        _ => Ok(value),
    }
}

struct UploadedFileKind {
    is_video: bool,
    is_audio: bool,
    is_pdf: bool,
    filename: Option<String>,
}

async fn search_pdf_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    filters: SearchFilters,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let upload_path = pdf_upload_path(&state.settings.upload_dir, filename);
    write_pdf_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let pdf = match decode_pdf(&upload_path, &state.settings) {
        Ok(pdf) => pdf,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process PDF: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    let mut scene_responses = Vec::new();
    let mut flattened = Vec::new();

    for page in &pdf.pages {
        let mut response = service
            .search_media_filtered(&page.media, limit, ocr_text, filters.clone())
            .await
            .map_err(search_error)?;
        for result in &mut response.results {
            result.query_scene_index = Some(page.page_index);
        }
        flattened.extend(response.results.clone());
        scene_responses.push(SearchSceneResponse {
            scene_index: page.page_index,
            scene_kind: "pdf_page".to_string(),
            start_frame: page.page_number as u64,
            end_frame: page.page_number as u64,
            start_seconds: 0.0,
            end_seconds: 0.0,
            clip_url: None,
            page_index: Some(page.page_index),
            page_number: Some(page.page_number),
            page_label: Some(format!("Page {}", page.page_number)),
            speaker_id: None,
            speaker_label: None,
            query_phash: response.query_phash,
            count: response.count,
            results: response.results,
        });
    }

    let results = deduplicate_flat_results(flattened);
    Ok(SearchResponse {
        query_phash: scene_responses
            .first()
            .map(|scene| scene.query_phash.clone())
            .unwrap_or_default(),
        count: results.len(),
        results,
        query_media_kind: "pdf".to_string(),
        scenes: scene_responses,
        query_audio_analysis: None,
        query_ocr_text: normalize_ocr_query(ocr_text),
    })
}

async fn search_audio_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    filters: SearchFilters,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let upload_path = audio_upload_path(&state.settings.upload_dir, filename);
    write_audio_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let segments = match decode_audio_segments(&upload_path, &state.settings) {
        Ok(segments) => segments,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process audio: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    let mut scene_responses = Vec::new();
    let mut flattened = Vec::new();

    for segment in &segments {
        let mut response = service
            .search_media_filtered(&segment.media, limit, ocr_text, filters.clone())
            .await
            .map_err(search_error)?;
        for result in &mut response.results {
            result.query_scene_index = Some(segment.scene_index);
        }
        flattened.extend(response.results.clone());
        scene_responses.push(SearchSceneResponse {
            scene_index: segment.scene_index,
            scene_kind: "audio_bit".to_string(),
            start_frame: (segment.start_seconds * 1000.0).round() as u64,
            end_frame: (segment.end_seconds * 1000.0).round() as u64,
            start_seconds: segment.start_seconds,
            end_seconds: segment.end_seconds,
            clip_url: None,
            page_index: None,
            page_number: None,
            page_label: None,
            speaker_id: segment.speaker_id.clone(),
            speaker_label: segment.speaker_label.clone(),
            query_phash: response.query_phash,
            count: response.count,
            results: response.results,
        });
    }

    let results = deduplicate_flat_results(flattened);
    Ok(SearchResponse {
        query_phash: scene_responses
            .first()
            .map(|scene| scene.query_phash.clone())
            .unwrap_or_default(),
        count: results.len(),
        results,
        query_media_kind: "audio".to_string(),
        scenes: scene_responses,
        query_audio_analysis: segments
            .first()
            .and_then(|segment| segment.media.audio_analysis.clone()),
        query_ocr_text: normalize_ocr_query(ocr_text),
    })
}

async fn search_video_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    filters: SearchFilters,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let upload_path = video_upload_path(&state.settings.upload_dir, filename);
    write_video_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let scenes = match decode_video_scenes(&upload_path, &state.settings) {
        Ok(scenes) => scenes,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process video: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    let mut scene_responses = Vec::new();
    let mut flattened = Vec::new();

    for scene in &scenes {
        let mut response = service
            .search_media_filtered(&scene.media, limit, ocr_text, filters.clone())
            .await
            .map_err(search_error)?;
        for result in &mut response.results {
            result.query_scene_index = Some(scene.scene_index);
        }
        flattened.extend(response.results.clone());
        scene_responses.push(SearchSceneResponse {
            scene_index: scene.scene_index,
            scene_kind: "scene".to_string(),
            start_frame: scene.start.frame_index,
            end_frame: scene.end.frame_index,
            start_seconds: scene.start.timestamp.seconds(),
            end_seconds: scene.end.timestamp.seconds(),
            clip_url: scene.clip_url.clone(),
            page_index: None,
            page_number: None,
            page_label: None,
            speaker_id: None,
            speaker_label: None,
            query_phash: response.query_phash,
            count: response.count,
            results: response.results,
        });
    }

    let results = deduplicate_flat_results(flattened);
    Ok(SearchResponse {
        query_phash: scene_responses
            .first()
            .map(|scene| scene.query_phash.clone())
            .unwrap_or_default(),
        count: results.len(),
        results,
        query_media_kind: "video".to_string(),
        scenes: scene_responses,
        query_audio_analysis: None,
        query_ocr_text: normalize_ocr_query(ocr_text),
    })
}

fn deduplicate_flat_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut by_image_id = BTreeMap::<String, SearchResult>::new();
    for result in results {
        by_image_id
            .entry(result.image.id.clone())
            .and_modify(|existing| {
                if result.vector_score > existing.vector_score {
                    *existing = result.clone();
                }
            })
            .or_insert(result);
    }
    let mut deduped = by_image_id.into_values().collect::<Vec<_>>();
    deduped.sort_by(|left, right| right.vector_score.total_cmp(&left.vector_score));
    deduped
}

fn search_error(error: String) -> ApiError {
    if error.contains("model is not available") || error.contains("model unavailable") {
        ApiError::service_unavailable(error)
    } else {
        ApiError::internal(error)
    }
}

fn multipart_error(error: axum::extract::multipart::MultipartError) -> ApiError {
    match error.status() {
        axum::http::StatusCode::PAYLOAD_TOO_LARGE => ApiError::payload_too_large(error.body_text()),
        _ => ApiError::bad_request(error.body_text()),
    }
}
