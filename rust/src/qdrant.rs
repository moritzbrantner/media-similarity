use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::ImagePayload;

#[derive(Clone)]
pub struct QdrantImageStore {
    client: Client,
    base_url: String,
    collection: String,
    vector_size: usize,
}

#[derive(Clone, Debug)]
pub struct ScoredPoint {
    pub payload: Option<Value>,
    pub score: f32,
}

impl QdrantImageStore {
    pub fn new(url: impl Into<String>, collection: impl Into<String>, vector_size: usize) -> Self {
        Self {
            client: Client::new(),
            base_url: url.into().trim_end_matches('/').to_string(),
            collection: collection.into(),
            vector_size,
        }
    }

    pub async fn ensure_collection(&self) -> Result<(), String> {
        let response = self
            .client
            .get(format!("{}/collections", self.base_url))
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<CollectionsResponse>()
            .await
            .map_err(|error| error.to_string())?;

        if response
            .result
            .collections
            .iter()
            .any(|collection| collection.name == self.collection)
        {
            return Ok(());
        }

        self.client
            .put(format!("{}/collections/{}", self.base_url, self.collection))
            .json(&CreateCollectionRequest {
                vectors: VectorParams {
                    size: self.vector_size,
                    distance: "Cosine",
                },
            })
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn upsert_image(
        &self,
        payload: &ImagePayload,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        self.client
            .put(format!(
                "{}/collections/{}/points?wait=true",
                self.base_url, self.collection
            ))
            .json(&UpsertRequest {
                points: vec![PointStruct {
                    id: payload.id.clone(),
                    vector,
                    payload: serde_json::to_value(payload).map_err(|error| error.to_string())?,
                }],
            })
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn search(&self, vector: Vec<f32>, limit: u32) -> Result<Vec<ScoredPoint>, String> {
        let response = self
            .client
            .post(format!(
                "{}/collections/{}/points/search",
                self.base_url, self.collection
            ))
            .json(&SearchRequest {
                vector,
                limit,
                with_payload: true,
            })
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<SearchResponse>()
            .await
            .map_err(|error| error.to_string())?;
        Ok(response
            .result
            .into_iter()
            .map(|point| ScoredPoint {
                payload: point.payload,
                score: point.score,
            })
            .collect())
    }

    #[allow(dead_code)]
    pub async fn count(&self) -> Result<u64, String> {
        let response = self
            .client
            .post(format!(
                "{}/collections/{}/points/count",
                self.base_url, self.collection
            ))
            .json(&serde_json::json!({ "exact": true }))
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<CountResponse>()
            .await
            .map_err(|error| error.to_string())?;
        Ok(response.result.count)
    }
}

#[derive(Deserialize)]
struct CollectionsResponse {
    result: CollectionsResult,
}

#[derive(Deserialize)]
struct CollectionsResult {
    collections: Vec<CollectionDescription>,
}

#[derive(Deserialize)]
struct CollectionDescription {
    name: String,
}

#[derive(Serialize)]
struct CreateCollectionRequest {
    vectors: VectorParams,
}

#[derive(Serialize)]
struct VectorParams {
    size: usize,
    distance: &'static str,
}

#[derive(Serialize)]
struct UpsertRequest {
    points: Vec<PointStruct>,
}

#[derive(Serialize)]
struct PointStruct {
    id: String,
    vector: Vec<f32>,
    payload: Value,
}

#[derive(Serialize)]
struct SearchRequest {
    vector: Vec<f32>,
    limit: u32,
    with_payload: bool,
}

#[derive(Deserialize)]
struct SearchResponse {
    result: Vec<SearchPoint>,
}

#[derive(Deserialize)]
struct SearchPoint {
    payload: Option<Value>,
    score: f32,
}

#[derive(Deserialize)]
struct CountResponse {
    result: CountResult,
}

#[derive(Deserialize)]
struct CountResult {
    count: u64,
}
