//! Optional Tantivy candidate filtering around the authoritative media vector store.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, MutexGuard};

use super::metadata_index::MetadataIndex;
use super::{MediaSearchFilter, MediaVectorStore, ScoredPoint, StoredPoint};
use crate::domain::models::{FacePointPayload, ImagePayload};

pub struct MetadataIndexStore {
    inner: Arc<dyn MediaVectorStore>,
    directory: PathBuf,
    state: Mutex<Option<MetadataIndex>>,
}

impl MetadataIndexStore {
    pub fn new(inner: Arc<dyn MediaVectorStore>, directory: &Path, collection: &str) -> Self {
        let namespace: String = Sha256::digest(collection.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Self {
            inner,
            directory: directory.join(namespace),
            state: Mutex::new(None),
        }
    }

    async fn projection(&self) -> Result<MutexGuard<'_, Option<MetadataIndex>>, String> {
        let mut state = self.state.lock().await;
        if state.is_none() {
            let mut index = MetadataIndex::open(&self.directory)?;
            index.reconcile(&self.inner.scroll_media_points().await?)?;
            *state = Some(index);
        }
        Ok(state)
    }

    fn index(state: &mut Option<MetadataIndex>) -> Result<&mut MetadataIndex, String> {
        state
            .as_mut()
            .ok_or_else(|| "metadata projection is not initialized".to_owned())
    }

    fn finish_write(
        state: &mut Option<MetadataIndex>,
        result: Result<(), String>,
    ) -> Result<(), String> {
        if result.is_err() {
            // A failed or ambiguous write must reopen/reconcile from Qdrant before reuse.
            // No attempt is made to turn a Qdrant payload into an authoritative local copy.
            *state = None;
        }
        result
    }
}

#[async_trait]
impl MediaVectorStore for MetadataIndexStore {
    async fn ensure_collection(&self) -> Result<(), String> {
        self.inner.ensure_collection().await?;
        let _state = self.projection().await?;
        Ok(())
    }

    async fn upsert_media(&self, payload: &ImagePayload, vector: Vec<f32>) -> Result<(), String> {
        let mut state = self.projection().await?;
        let result = match self.inner.upsert_media(payload, vector).await {
            Ok(()) => Self::index(&mut state)?.upsert(payload),
            Err(error) => Err(error),
        };
        Self::finish_write(&mut state, result)
    }

    async fn set_media_payload(&self, payload: &ImagePayload) -> Result<(), String> {
        let mut state = self.projection().await?;
        let result = match self.inner.set_media_payload(payload).await {
            Ok(()) => Self::index(&mut state)?.upsert(payload),
            Err(error) => Err(error),
        };
        Self::finish_write(&mut state, result)
    }

    async fn delete_points(&self, ids: &[String]) -> Result<(), String> {
        let mut state = self.projection().await?;
        let result = match self.inner.delete_points(ids).await {
            Ok(()) => Self::index(&mut state)?.delete(ids),
            Err(error) => Err(error),
        };
        Self::finish_write(&mut state, result)
    }

    async fn search_visual_filtered(
        &self,
        vector: Vec<f32>,
        limit: u32,
        filter: Option<MediaSearchFilter>,
    ) -> Result<Vec<ScoredPoint>, String> {
        let Some(filter) = filter.filter(|filter| !filter.is_empty()) else {
            return self.inner.search_visual_filtered(vector, limit, None).await;
        };
        let mut state = self.projection().await?;
        let ids = Self::index(&mut state)?.ids(&filter)?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // Keep the mutation lock until Qdrant has read candidates, so an application
        // metadata update cannot race between the predicate and the vector query.
        self.inner
            .search_visual_filtered(
                vector,
                limit,
                Some(MediaSearchFilter {
                    ids: Some(ids),
                    ..Default::default()
                }),
            )
            .await
    }

    async fn scroll_media_points_filtered(
        &self,
        filter: Option<MediaSearchFilter>,
    ) -> Result<Vec<StoredPoint>, String> {
        let Some(filter) = filter.filter(|filter| !filter.is_empty()) else {
            return self.inner.scroll_media_points_filtered(None).await;
        };
        let mut state = self.projection().await?;
        let ids = Self::index(&mut state)?.ids(&filter)?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.inner
            .scroll_media_points_filtered(Some(MediaSearchFilter {
                ids: Some(ids),
                ..Default::default()
            }))
            .await
    }

    async fn set_legacy_media_source_identity(
        &self,
        point_id: &str,
        source_item_uri: &str,
        source_uri: Option<&str>,
    ) -> Result<(), String> {
        let mut state = self.state.lock().await;
        let result = self
            .inner
            .set_legacy_media_source_identity(point_id, source_item_uri, source_uri)
            .await;
        // The inner operation is conditional and authoritative. Reconcile its actual
        // result rather than guessing whether it replaced a missing legacy identity.
        *state = None;
        result
    }

    async fn upsert_face(
        &self,
        payload: &FacePointPayload,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        self.inner.upsert_face(payload, vector).await
    }
    async fn set_face_payload(&self, payload: &FacePointPayload) -> Result<(), String> {
        self.inner.set_face_payload(payload).await
    }
    async fn search_visual(
        &self,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<ScoredPoint>, String> {
        self.inner.search_visual(vector, limit).await
    }
    async fn search_faces(&self, vector: Vec<f32>, limit: u32) -> Result<Vec<ScoredPoint>, String> {
        self.inner.search_faces(vector, limit).await
    }
    async fn scroll_media_points(&self) -> Result<Vec<StoredPoint>, String> {
        self.inner.scroll_media_points().await
    }
    async fn scroll_face_points(&self) -> Result<Vec<StoredPoint>, String> {
        self.inner.scroll_face_points().await
    }
    async fn scroll_media_points_by_filter(
        &self,
        id: Option<&str>,
        source_uri: Option<&str>,
        source_item_uri: Option<&str>,
    ) -> Result<Vec<StoredPoint>, String> {
        self.inner
            .scroll_media_points_by_filter(id, source_uri, source_item_uri)
            .await
    }
    async fn scroll_legacy_media_points(
        &self,
        limit: Option<u32>,
    ) -> Result<Vec<StoredPoint>, String> {
        self.inner.scroll_legacy_media_points(limit).await
    }
    async fn scroll_face_points_by_media_ids(
        &self,
        media_ids: &[String],
    ) -> Result<Vec<StoredPoint>, String> {
        self.inner.scroll_face_points_by_media_ids(media_ids).await
    }
}
