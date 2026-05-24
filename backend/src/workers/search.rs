use crate::config::Settings;
use crate::domain::models::{ImagePayload, SearchResponse, SearchResult};
use crate::storage::{MediaSearchFilter, MediaVectorStore};
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
