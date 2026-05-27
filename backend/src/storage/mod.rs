use async_trait::async_trait;
use serde_json::Value;

use crate::domain::models::{FacePointPayload, ImagePayload};

pub mod qdrant;

#[derive(Clone, Debug)]
pub struct ScoredPoint {
    pub payload: Option<Value>,
    pub score: f32,
}

#[derive(Clone, Debug)]
pub struct StoredPoint {
    pub id: String,
    pub payload: Option<Value>,
}

#[derive(Clone, Debug, Default)]
pub struct MediaSearchFilter {
    pub source_type: Option<String>,
    pub media_kind: Option<String>,
    pub has_gps: Option<bool>,
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
}

impl MediaSearchFilter {
    pub fn is_empty(&self) -> bool {
        self.source_type.is_none()
            && self.media_kind.is_none()
            && self.has_gps.is_none()
            && self.min_width.is_none()
            && self.max_width.is_none()
            && self.min_height.is_none()
            && self.max_height.is_none()
            && self.min_size_bytes.is_none()
            && self.max_size_bytes.is_none()
            && self.modified_from.is_none()
            && self.modified_to.is_none()
            && self.captured_from.is_none()
            && self.captured_to.is_none()
    }
}

#[async_trait]
pub trait MediaVectorStore: Send + Sync {
    async fn ensure_collection(&self) -> Result<(), String>;

    async fn upsert_media(&self, payload: &ImagePayload, vector: Vec<f32>) -> Result<(), String>;

    async fn upsert_face(&self, payload: &FacePointPayload, vector: Vec<f32>)
        -> Result<(), String>;

    async fn set_media_payload(&self, payload: &ImagePayload) -> Result<(), String>;

    async fn set_face_payload(&self, payload: &FacePointPayload) -> Result<(), String>;

    async fn delete_points(&self, ids: &[String]) -> Result<(), String>;

    async fn delete_points_by_ids(&self, ids: &[String]) -> Result<(), String> {
        self.delete_points(ids).await
    }

    async fn search_visual(&self, vector: Vec<f32>, limit: u32)
        -> Result<Vec<ScoredPoint>, String>;

    async fn search_visual_filtered(
        &self,
        vector: Vec<f32>,
        limit: u32,
        filter: Option<MediaSearchFilter>,
    ) -> Result<Vec<ScoredPoint>, String> {
        let _ = filter;
        self.search_visual(vector, limit).await
    }

    async fn search_faces(&self, vector: Vec<f32>, limit: u32) -> Result<Vec<ScoredPoint>, String>;

    async fn scroll_media_points(&self) -> Result<Vec<StoredPoint>, String>;

    async fn scroll_media_points_filtered(
        &self,
        filter: Option<MediaSearchFilter>,
    ) -> Result<Vec<StoredPoint>, String> {
        let _ = filter;
        self.scroll_media_points().await
    }

    async fn scroll_face_points(&self) -> Result<Vec<StoredPoint>, String>;

    async fn scroll_media_points_by_filter(
        &self,
        id: Option<&str>,
        source_uri: Option<&str>,
        source_item_uri: Option<&str>,
    ) -> Result<Vec<StoredPoint>, String>;

    async fn scroll_face_points_by_media_ids(
        &self,
        media_ids: &[String],
    ) -> Result<Vec<StoredPoint>, String>;
}
