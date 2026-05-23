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

#[async_trait]
pub trait MediaVectorStore: Send + Sync {
    async fn ensure_collection(&self) -> Result<(), String>;

    async fn upsert_media(&self, payload: &ImagePayload, vector: Vec<f32>) -> Result<(), String>;

    async fn upsert_face(&self, payload: &FacePointPayload, vector: Vec<f32>)
        -> Result<(), String>;

    async fn set_media_payload(&self, payload: &ImagePayload) -> Result<(), String>;

    async fn delete_points(&self, ids: &[String]) -> Result<(), String>;

    async fn delete_points_by_ids(&self, ids: &[String]) -> Result<(), String> {
        self.delete_points(ids).await
    }

    async fn search_visual(&self, vector: Vec<f32>, limit: u32)
        -> Result<Vec<ScoredPoint>, String>;

    async fn search_faces(&self, vector: Vec<f32>, limit: u32) -> Result<Vec<ScoredPoint>, String>;

    async fn scroll_media_points(&self) -> Result<Vec<StoredPoint>, String>;

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
