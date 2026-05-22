use crate::config::Settings;
use crate::domain::models::{ImagePayload, SearchResponse, SearchResult};
use crate::storage::qdrant::QdrantImageStore;
use crate::workers::media::hashing::{hash_distance, phash_image};
use crate::workers::media::media::DecodedMedia;
use crate::workers::media::ocr::{normalize_ocr_query, ocr_match_score};
use crate::workers::media::visual_embedding::VisualEmbeddingBackend;

use std::sync::Arc;

#[derive(Clone)]
pub struct ImageSearchService {
    settings: Settings,
    store: QdrantImageStore,
    embedder: Arc<dyn VisualEmbeddingBackend>,
}

impl ImageSearchService {
    pub fn new(
        settings: Settings,
        store: QdrantImageStore,
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
        self.store.ensure_collection().await?;
        let query_phash = phash_image(&media.poster);
        let requested_limit = limit.unwrap_or(self.settings.default_search_limit);
        let normalized_ocr_query = normalize_ocr_query(ocr_text);
        let search_limit = if normalized_ocr_query.is_empty() {
            requested_limit
        } else {
            requested_limit
                .saturating_mul(8)
                .max(requested_limit)
                .min(500)
        };
        let query_vector = self
            .embedder
            .embed_media(&media.sampled_frames, self.settings.gif_motion_weight)?;
        let points = self.store.search_visual(query_vector, search_limit).await?;

        let mut results = Vec::new();
        for point in points {
            let Some(payload) = point.payload else {
                continue;
            };
            let image: ImagePayload =
                serde_json::from_value(payload).map_err(|error| error.to_string())?;
            if let Some(person_id) = person_id {
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
            results.truncate(requested_limit as usize);
        }

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
