use std::collections::BTreeMap;
use std::sync::Arc;

use crate::api::{ApiError, AppState};
use crate::domain::models::{SearchResponse, SearchResult, SearchSceneResponse};
use crate::workers::media::audio::{audio_upload_path, decode_audio_segments, write_audio_upload};
use crate::workers::media::image_io::load_media_bytes;
use crate::workers::media::ocr::normalize_ocr_query;
use crate::workers::media::pdf::{decode_pdf, pdf_upload_path, write_pdf_upload};
use crate::workers::media::video::{
    decode_video_scenes, video_upload_path, write_video_upload, DecodedVideoScene,
};
use crate::workers::search::{ImageSearchService, SearchFilters};
use crate::workers::workflows::{MediaFileKind, WorkflowMode};
use bytes::Bytes;

#[derive(Debug)]
pub struct UploadedFileKind {
    pub is_video: bool,
    pub is_audio: bool,
    pub is_pdf: bool,
    pub filename: Option<String>,
}

pub async fn search_local_image(
    state: Arc<AppState>,
    query: crate::api::search::SearchQuery,
    raw: Bytes,
    filters: SearchFilters,
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let kind = if filename
        .and_then(|name| std::path::Path::new(name).extension())
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("gif"))
        .unwrap_or(false)
    {
        MediaFileKind::AnimatedGif
    } else {
        MediaFileKind::StaticImage
    };
    let settings = workflow_settings_for_upload(&state, kind)?;
    let media = load_media_bytes(&raw, &settings)
        .map_err(|_| ApiError::bad_request("Could not decode image"))?;
    let service = ImageSearchService::new(settings, state.store.clone(), state.embedder.clone());
    service
        .search_media_filtered(&media, query.limit, query.ocr_text.as_deref(), filters)
        .await
        .map_err(search_error)
}

pub async fn search_text_query(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    filters: SearchFilters,
) -> Result<SearchResponse, ApiError> {
    let text = ocr_text
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::bad_request("Search requires either a media upload or text query")
        })?;
    let service = ImageSearchService::new(
        state.indexing_settings(),
        state.store.clone(),
        state.embedder.clone(),
    );
    service
        .search_text_filtered(limit, text, filters)
        .await
        .map_err(search_error)
}

pub async fn search_pdf_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    filters: SearchFilters,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let settings = workflow_settings_for_upload(&state, MediaFileKind::Pdf)?;
    let upload_path = pdf_upload_path(&settings.upload_dir, filename);
    write_pdf_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let pdf = match decode_pdf(&upload_path, &settings) {
        Ok(pdf) => pdf,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process PDF: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(settings, state.store.clone(), state.embedder.clone());
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
        query_visual_embedding_model: Some(state.embedder.model_name().to_string()),
        query_visual_embedding_degraded: state.embedder.is_degraded(),
    })
}

pub async fn search_audio_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    filters: SearchFilters,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let settings = workflow_settings_for_upload(&state, MediaFileKind::Audio)?;
    let upload_path = audio_upload_path(&settings.upload_dir, filename);
    write_audio_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let segments = match decode_audio_segments(&upload_path, &settings) {
        Ok(segments) => segments,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process audio: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(settings, state.store.clone(), state.embedder.clone());
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
        query_visual_embedding_model: Some(state.embedder.model_name().to_string()),
        query_visual_embedding_degraded: state.embedder.is_degraded(),
    })
}

pub async fn search_video_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    filters: SearchFilters,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let settings = workflow_settings_for_upload(&state, MediaFileKind::Video)?;
    let upload_path = video_upload_path(&settings.upload_dir, filename);
    write_video_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let scenes = match decode_video_scenes(&upload_path, &settings) {
        Ok(scenes) => scenes,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process video: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(settings, state.store.clone(), state.embedder.clone());
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
        query_audio_analysis: video_query_audio_analysis(&scenes),
        query_ocr_text: normalize_ocr_query(ocr_text),
        query_visual_embedding_model: Some(state.embedder.model_name().to_string()),
        query_visual_embedding_degraded: state.embedder.is_degraded(),
    })
}

fn video_query_audio_analysis(
    scenes: &[DecodedVideoScene],
) -> Option<crate::domain::models::AudioAnalysis> {
    scenes
        .iter()
        .find_map(|scene| scene.media.audio_analysis.clone())
}

fn workflow_settings_for_upload(
    state: &AppState,
    kind: MediaFileKind,
) -> Result<crate::config::Settings, ApiError> {
    let workflow = state
        .compiled_workflow(kind, WorkflowMode::Search)
        .map_err(ApiError::internal)?;
    let mut settings = state.indexing_settings();
    workflow.apply_to_settings(&mut settings);
    Ok(settings)
}

fn deduplicate_flat_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut by_image_id = BTreeMap::<String, SearchResult>::new();
    for result in results {
        by_image_id
            .entry(result.image.id.clone())
            .and_modify(|existing| {
                if result.relevance_score.unwrap_or(result.vector_score)
                    > existing.relevance_score.unwrap_or(existing.vector_score)
                {
                    *existing = result.clone();
                }
            })
            .or_insert(result);
    }
    let mut deduped = by_image_id.into_values().collect::<Vec<_>>();
    deduped.sort_by(|left, right| {
        right
            .relevance_score
            .unwrap_or(right.vector_score)
            .total_cmp(&left.relevance_score.unwrap_or(left.vector_score))
    });
    deduped
}

fn search_error(error: String) -> ApiError {
    if error.contains("model is not available") || error.contains("model unavailable") {
        ApiError::service_unavailable(error)
    } else {
        ApiError::internal(error)
    }
}

#[cfg(test)]
mod tests {
    use super::video_query_audio_analysis;
    use crate::domain::models::AudioAnalysis;
    use crate::workers::media::media::{DecodedMedia, MediaFrame, MediaKind};
    use crate::workers::media::video::DecodedVideoScene;
    use image::RgbImage;
    use num_rational::Rational64;
    use video_analysis_core::FramePosition;

    #[test]
    fn video_query_audio_analysis_uses_first_scene_with_audio_analysis() {
        let scene_without_audio = decoded_scene(0, None);
        let scene_with_audio = decoded_scene(
            1,
            Some(AudioAnalysis {
                speech_detected: true,
                speech_ratio: 1.0,
                speech_segments: Vec::new(),
                audio_segments: Vec::new(),
                recognized_voices: Vec::new(),
                transcript_text: "budget scene".to_string(),
                transcript_language: Some("en".to_string()),
                transcript_segments: Vec::new(),
                tempo_bpm: None,
                tempo_confidence: 0.0,
                tempo_onset_count: 0,
            }),
        );

        let analysis =
            video_query_audio_analysis(&[scene_without_audio, scene_with_audio]).unwrap();

        assert_eq!(analysis.transcript_text, "budget scene");
    }

    fn decoded_scene(
        scene_index: usize,
        audio_analysis: Option<AudioAnalysis>,
    ) -> DecodedVideoScene {
        let image = RgbImage::new(1, 1);
        let frame = MediaFrame {
            image: image.clone(),
            delay_ms: 1,
        };
        DecodedVideoScene {
            scene_index,
            start: FramePosition::from_frame_index(0, Rational64::new(24, 1)),
            end: FramePosition::from_frame_index(1, Rational64::new(24, 1)),
            clip_url: None,
            media: DecodedMedia {
                kind: MediaKind::VideoScene,
                width: 1,
                height: 1,
                frame_count: Some(1),
                duration_ms: Some(42),
                poster: image,
                sampled_frames: vec![frame.clone()],
                preview_frames: vec![frame],
                audio_analysis,
            },
        }
    }
}
