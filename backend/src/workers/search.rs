use crate::config::Settings;
use crate::domain::models::{AudioAnalysis, ImagePayload, SearchResponse, SearchResult};
use crate::storage::{MediaSearchFilter, MediaVectorStore, StoredPoint};
use crate::workers::media::hashing::{hash_distance, phash_image};
use crate::workers::media::media::DecodedMedia;
use crate::workers::media::ocr::{normalize_ocr_query, ocr_match_score};
use crate::workers::media::visual_embedding::VisualEmbeddingBackend;

use std::sync::Arc;

#[derive(Clone)]
pub struct ImageSearchService {
    settings: Settings,
    store: Arc<dyn MediaVectorStore>,
    embedder: Arc<dyn VisualEmbeddingBackend>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchFilters {
    pub source_type: Option<String>,
    pub media_kind: Option<String>,
    pub name_query: Option<String>,
    pub camera_query: Option<String>,
    pub keyword_query: Option<String>,
    pub has_gps: Option<bool>,
    pub near_duplicate: Option<NearDuplicateFilter>,
    pub orientation: Option<OrientationFilter>,
    pub min_width: Option<u32>,
    pub max_width: Option<u32>,
    pub min_height: Option<u32>,
    pub max_height: Option<u32>,
    pub min_size_bytes: Option<u64>,
    pub max_size_bytes: Option<u64>,
    pub modified_from: Option<f64>,
    pub modified_to: Option<f64>,
    pub captured_from: Option<f64>,
    pub captured_to: Option<f64>,
    pub person_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NearDuplicateFilter {
    Exclude,
    Only,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrientationFilter {
    Landscape,
    Portrait,
    Square,
}

impl ImageSearchService {
    pub fn new(
        settings: Settings,
        store: Arc<dyn MediaVectorStore>,
        embedder: Arc<dyn VisualEmbeddingBackend>,
    ) -> Self {
        Self {
            settings,
            store,
            embedder,
        }
    }

    pub async fn search_media(
        &self,
        media: &DecodedMedia,
        limit: Option<u32>,
        ocr_text: Option<&str>,
        person_id: Option<&str>,
    ) -> Result<SearchResponse, String> {
        let filters = SearchFilters {
            person_id: person_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
            ..Default::default()
        };
        self.search_media_filtered(media, limit, ocr_text, filters)
            .await
    }

    pub async fn search_media_filtered(
        &self,
        media: &DecodedMedia,
        limit: Option<u32>,
        ocr_text: Option<&str>,
        filters: SearchFilters,
    ) -> Result<SearchResponse, String> {
        self.store.ensure_collection().await?;
        let query_phash = phash_image(&media.poster);
        let requested_limit = limit.unwrap_or(self.settings.default_search_limit);
        let normalized_ocr_query = normalize_ocr_query(ocr_text);
        let needs_post_filter = filters.needs_post_filter() || !normalized_ocr_query.is_empty();
        let search_limit = if needs_post_filter {
            requested_limit
                .saturating_mul(8)
                .max(requested_limit)
                .min(500)
        } else {
            requested_limit
        };
        let query_vector = self
            .embedder
            .embed_media(&media.sampled_frames, self.settings.gif_motion_weight)?;
        let points = self
            .store
            .search_visual_filtered(query_vector, search_limit, Some(filters.qdrant_filter()))
            .await?;

        let mut results = Vec::new();
        for point in points {
            let Some(payload) = point.payload else {
                continue;
            };
            let image: ImagePayload =
                serde_json::from_value(payload).map_err(|error| error.to_string())?;
            if let Some(person_id) = filters.person_id.as_deref() {
                if !image
                    .people
                    .iter()
                    .any(|person| person.person_id == person_id)
                {
                    continue;
                }
            }
            let ocr_score = if normalized_ocr_query.is_empty() {
                None
            } else {
                match ocr_match_score(&image.ocr_text, &normalized_ocr_query) {
                    Some(score) => Some(score),
                    None => continue,
                }
            };
            let distance = hash_distance(&query_phash, &image.phash)?;
            if !filters.matches_after_hash(&image, distance, self.settings.duplicate_hash_distance)
            {
                continue;
            }
            results.push(SearchResult {
                image,
                vector_score: point.score,
                hash_distance: Some(distance),
                ocr_score,
                near_duplicate: distance <= self.settings.duplicate_hash_distance,
                query_scene_index: None,
            });
        }

        if !normalized_ocr_query.is_empty() {
            results.sort_by(|left, right| {
                right
                    .ocr_score
                    .unwrap_or_default()
                    .total_cmp(&left.ocr_score.unwrap_or_default())
                    .then_with(|| right.vector_score.total_cmp(&left.vector_score))
            });
        }
        results.truncate(requested_limit as usize);

        Ok(SearchResponse {
            query_phash,
            count: results.len(),
            results,
            query_media_kind: media.kind.as_str().to_string(),
            scenes: Vec::new(),
            query_audio_analysis: media.audio_analysis.clone(),
            query_ocr_text: normalized_ocr_query,
        })
    }

    pub async fn search_text_filtered(
        &self,
        limit: Option<u32>,
        text: &str,
        filters: SearchFilters,
    ) -> Result<SearchResponse, String> {
        self.store.ensure_collection().await?;
        let requested_limit = limit.unwrap_or(self.settings.default_search_limit);
        let normalized_text_query = normalize_ocr_query(Some(text));
        if normalized_text_query.is_empty() {
            return Err("text query is required".to_string());
        }

        let points = self
            .store
            .scroll_media_points_filtered(Some(filters.qdrant_filter()))
            .await?;
        let mut results = Vec::new();
        for point in points {
            let Some(image) = payload_from_stored_point(point)? else {
                continue;
            };
            if !filters.matches_without_hash(&image) {
                continue;
            }
            let Some(ocr_score) = media_text_match_score(&image, &normalized_text_query) else {
                continue;
            };
            results.push(SearchResult {
                image,
                vector_score: ocr_score,
                hash_distance: None,
                ocr_score: Some(ocr_score),
                near_duplicate: false,
                query_scene_index: None,
            });
        }

        results.sort_by(|left, right| {
            right
                .ocr_score
                .unwrap_or_default()
                .total_cmp(&left.ocr_score.unwrap_or_default())
                .then_with(|| left.image.filename.cmp(&right.image.filename))
        });
        results.truncate(requested_limit as usize);

        Ok(SearchResponse {
            query_phash: String::new(),
            count: results.len(),
            results,
            query_media_kind: "text".to_string(),
            scenes: Vec::new(),
            query_audio_analysis: None,
            query_ocr_text: normalized_text_query,
        })
    }
}

impl SearchFilters {
    fn qdrant_filter(&self) -> MediaSearchFilter {
        MediaSearchFilter {
            source_type: self.source_type.clone(),
            media_kind: self.media_kind.clone(),
            has_gps: self.has_gps,
            min_width: self.min_width,
            max_width: self.max_width,
            min_height: self.min_height,
            max_height: self.max_height,
            min_size_bytes: self.min_size_bytes,
            max_size_bytes: self.max_size_bytes,
            modified_from: self.modified_from,
            modified_to: self.modified_to,
            captured_from: self.captured_from,
            captured_to: self.captured_to,
        }
    }

    fn needs_post_filter(&self) -> bool {
        self.name_query.is_some()
            || self.camera_query.is_some()
            || self.keyword_query.is_some()
            || self.near_duplicate.is_some()
            || self.orientation.is_some()
            || self.person_id.is_some()
    }

    fn matches_after_hash(
        &self,
        image: &ImagePayload,
        hash_distance: u32,
        duplicate_hash_distance: u32,
    ) -> bool {
        if let Some(near_duplicate) = self.near_duplicate {
            let is_near_duplicate = hash_distance <= duplicate_hash_distance;
            match near_duplicate {
                NearDuplicateFilter::Only if !is_near_duplicate => return false,
                NearDuplicateFilter::Exclude if is_near_duplicate => return false,
                _ => {}
            }
        }
        if let Some(orientation) = self.orientation {
            let image_orientation = image_orientation(image.width, image.height);
            if image_orientation != orientation {
                return false;
            }
        }
        if let Some(query) = self.name_query.as_deref() {
            if !matches_any_text(
                query,
                [
                    image.filename.as_str(),
                    image.relative_path.as_str(),
                    image.path.as_str(),
                    image.source_uri.as_deref().unwrap_or_default(),
                ],
            ) {
                return false;
            }
        }
        if let Some(query) = self.camera_query.as_deref() {
            let Some(metadata) = &image.photo_metadata else {
                return false;
            };
            if !matches_any_text(
                query,
                [
                    metadata.camera_make.as_deref().unwrap_or_default(),
                    metadata.camera_model.as_deref().unwrap_or_default(),
                    metadata.lens_model.as_deref().unwrap_or_default(),
                ],
            ) {
                return false;
            }
        }
        if let Some(query) = self.keyword_query.as_deref() {
            let metadata_matches = image
                .photo_metadata
                .as_ref()
                .map(|metadata| {
                    metadata
                        .keywords
                        .iter()
                        .any(|keyword| text_matches(keyword, query))
                })
                .unwrap_or(false);
            let tag_matches = image.tags.iter().any(|tag| text_matches(tag, query));
            if !metadata_matches && !tag_matches {
                return false;
            }
        }
        true
    }

    fn matches_without_hash(&self, image: &ImagePayload) -> bool {
        if matches!(self.near_duplicate, Some(NearDuplicateFilter::Only)) {
            return false;
        }
        if let Some(orientation) = self.orientation {
            let image_orientation = image_orientation(image.width, image.height);
            if image_orientation != orientation {
                return false;
            }
        }
        if let Some(person_id) = self.person_id.as_deref() {
            if !image
                .people
                .iter()
                .any(|person| person.person_id == person_id)
            {
                return false;
            }
        }
        if let Some(query) = self.name_query.as_deref() {
            if !matches_any_text(
                query,
                [
                    image.filename.as_str(),
                    image.relative_path.as_str(),
                    image.path.as_str(),
                    image.source_uri.as_deref().unwrap_or_default(),
                ],
            ) {
                return false;
            }
        }
        if let Some(query) = self.camera_query.as_deref() {
            let Some(metadata) = &image.photo_metadata else {
                return false;
            };
            if !matches_any_text(
                query,
                [
                    metadata.camera_make.as_deref().unwrap_or_default(),
                    metadata.camera_model.as_deref().unwrap_or_default(),
                    metadata.lens_model.as_deref().unwrap_or_default(),
                ],
            ) {
                return false;
            }
        }
        if let Some(query) = self.keyword_query.as_deref() {
            let metadata_matches = image
                .photo_metadata
                .as_ref()
                .map(|metadata| {
                    metadata
                        .keywords
                        .iter()
                        .any(|keyword| text_matches(keyword, query))
                })
                .unwrap_or(false);
            let tag_matches = image.tags.iter().any(|tag| text_matches(tag, query));
            if !metadata_matches && !tag_matches {
                return false;
            }
        }
        true
    }
}

fn payload_from_stored_point(point: StoredPoint) -> Result<Option<ImagePayload>, String> {
    let Some(payload) = point.payload else {
        return Ok(None);
    };
    serde_json::from_value(payload)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn media_text_match_score(image: &ImagePayload, normalized_query: &str) -> Option<f32> {
    [
        ocr_match_score(&image.ocr_text, normalized_query),
        image
            .ocr_frames
            .iter()
            .filter_map(|frame| ocr_match_score(&frame.text, normalized_query))
            .max_by(f32::total_cmp),
        image
            .audio_analysis
            .as_ref()
            .and_then(|analysis| audio_text_match_score(analysis, normalized_query)),
    ]
    .into_iter()
    .flatten()
    .max_by(f32::total_cmp)
}

fn audio_text_match_score(analysis: &AudioAnalysis, normalized_query: &str) -> Option<f32> {
    std::iter::once(ocr_match_score(&analysis.transcript_text, normalized_query))
        .chain(
            analysis
                .transcript_segments
                .iter()
                .map(|segment| ocr_match_score(&segment.text, normalized_query)),
        )
        .flatten()
        .max_by(f32::total_cmp)
}

fn matches_any_text<'a>(query: &str, values: impl IntoIterator<Item = &'a str>) -> bool {
    values.into_iter().any(|value| text_matches(value, query))
}

fn text_matches(value: &str, query: &str) -> bool {
    value.to_lowercase().contains(&query.to_lowercase())
}

fn image_orientation(width: u32, height: u32) -> OrientationFilter {
    if width == height {
        OrientationFilter::Square
    } else if width > height {
        OrientationFilter::Landscape
    } else {
        OrientationFilter::Portrait
    }
}

#[cfg(test)]
mod tests {
    use super::{media_text_match_score, NearDuplicateFilter, OrientationFilter, SearchFilters};
    use crate::domain::models::{AudioAnalysis, AudioTranscriptSegment, ImagePayload};

    #[test]
    fn media_text_score_covers_ocr_frames_and_transcripts() {
        let mut image = test_payload("clip.mp4");
        image.ocr_text = "Opening title".to_string();
        image.audio_analysis = Some(AudioAnalysis {
            speech_detected: true,
            speech_ratio: 1.0,
            speech_segments: Vec::new(),
            audio_segments: Vec::new(),
            recognized_voices: Vec::new(),
            transcript_text: String::new(),
            transcript_language: Some("en".to_string()),
            transcript_segments: vec![AudioTranscriptSegment {
                segment_index: 0,
                start_seconds: Some(0.0),
                end_seconds: Some(1.0),
                text: "Quarterly budget review".to_string(),
                confidence: Some(0.9),
            }],
            tempo_bpm: None,
            tempo_confidence: 0.0,
            tempo_onset_count: 0,
        });

        assert!(media_text_match_score(&image, "budget review").is_some());
        assert!(media_text_match_score(&image, "missing phrase").is_none());
    }

    #[test]
    fn text_only_filters_apply_metadata_without_hash_distance() {
        let image = test_payload("portrait.jpg");
        let filters = SearchFilters {
            orientation: Some(OrientationFilter::Portrait),
            near_duplicate: Some(NearDuplicateFilter::Exclude),
            name_query: Some("portrait".to_string()),
            ..SearchFilters::default()
        };

        assert!(filters.matches_without_hash(&image));

        let near_duplicate_only = SearchFilters {
            near_duplicate: Some(NearDuplicateFilter::Only),
            ..SearchFilters::default()
        };
        assert!(!near_duplicate_only.matches_without_hash(&image));
    }

    fn test_payload(filename: &str) -> ImagePayload {
        ImagePayload {
            id: filename.to_string(),
            path: format!("/media/{filename}"),
            relative_path: filename.to_string(),
            filename: filename.to_string(),
            width: 800,
            height: 1200,
            size_bytes: 1_024,
            modified_at: 1_700_000_000.0,
            phash: "0000000000000000".to_string(),
            thumbnail_url: None,
            animated_thumbnail_url: None,
            media_kind: "static_image".to_string(),
            frame_count: None,
            duration_ms: None,
            full_video_url: None,
            full_audio_url: None,
            full_pdf_url: None,
            pdf_page_url: None,
            pdf_document_id: None,
            pdf_page_index: None,
            pdf_page_number: None,
            pdf_page_count: None,
            audio_analysis: None,
            ocr_text: String::new(),
            ocr_frames: Vec::new(),
            visual_embedding_model: None,
            faces: Vec::new(),
            people: Vec::new(),
            artifacts: Vec::new(),
            tags: Vec::new(),
            photo_metadata: None,
            scene_clip_url: None,
            scene_index: None,
            scene_start_frame: None,
            scene_end_frame: None,
            scene_start_seconds: None,
            scene_end_seconds: None,
            source_type: "local".to_string(),
            source_item_uri: None,
            indexing_profile: None,
            source_uri: Some("/media".to_string()),
        }
    }
}
