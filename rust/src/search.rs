use crate::config::Settings;
use crate::embedder::ImageEmbedder;
use crate::hashing::{hash_distance, phash_image};
use crate::media::DecodedMedia;
use crate::models::{ImagePayload, SearchResponse, SearchResult};
use crate::qdrant::QdrantImageStore;

#[derive(Clone)]
pub struct ImageSearchService {
    settings: Settings,
    store: QdrantImageStore,
    embedder: ImageEmbedder,
}

impl ImageSearchService {
    pub fn new(settings: Settings, store: QdrantImageStore, embedder: ImageEmbedder) -> Self {
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
    ) -> Result<SearchResponse, String> {
        self.store.ensure_collection().await?;
        let query_phash = phash_image(&media.poster);
        let query_vector = self
            .embedder
            .encode_media(&media.sampled_frames, self.settings.gif_motion_weight);
        let points = self
            .store
            .search(
                query_vector,
                limit.unwrap_or(self.settings.default_search_limit),
            )
            .await?;

        let mut results = Vec::new();
        for point in points {
            let Some(payload) = point.payload else {
                continue;
            };
            let image: ImagePayload =
                serde_json::from_value(payload).map_err(|error| error.to_string())?;
            let distance = hash_distance(&query_phash, &image.phash)?;
            results.push(SearchResult {
                image,
                vector_score: point.score,
                hash_distance: Some(distance),
                near_duplicate: distance <= self.settings.duplicate_hash_distance,
                query_scene_index: None,
            });
        }

        Ok(SearchResponse {
            query_phash,
            count: results.len(),
            results,
            query_media_kind: media.kind.as_str().to_string(),
            scenes: Vec::new(),
        })
    }
}
