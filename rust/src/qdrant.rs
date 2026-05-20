use reqwest::{Client, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::models::ImagePayload;

#[derive(Clone)]
pub struct QdrantImageStore {
    client: Client,
    base_urls: Vec<String>,
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
            base_urls: qdrant_base_urls(&url.into()),
            collection: collection.into(),
            vector_size,
        }
    }

    pub async fn ensure_collection(&self) -> Result<(), String> {
        let response = self
            .send_with_fallback(|base_url| self.client.get(format!("{base_url}/collections")))
            .await?
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

        let request = CreateCollectionRequest {
            vectors: VectorParams {
                size: self.vector_size,
                distance: "Cosine",
            },
        };
        self.send_with_fallback(|base_url| {
            self.client
                .put(format!("{base_url}/collections/{}", self.collection))
                .json(&request)
        })
        .await?
        .error_for_status()
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn upsert_image(
        &self,
        payload: &ImagePayload,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        let request = UpsertRequest {
            points: vec![PointStruct {
                id: payload.id.clone(),
                vector,
                payload: serde_json::to_value(payload).map_err(|error| error.to_string())?,
            }],
        };
        self.send_with_fallback(|base_url| {
            self.client
                .put(format!(
                    "{base_url}/collections/{}/points?wait=true",
                    self.collection
                ))
                .json(&request)
        })
        .await?
        .error_for_status()
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn search(&self, vector: Vec<f32>, limit: u32) -> Result<Vec<ScoredPoint>, String> {
        let request = SearchRequest {
            vector,
            limit,
            with_payload: true,
        };
        let response = self
            .send_with_fallback(|base_url| {
                self.client
                    .post(format!(
                        "{base_url}/collections/{}/points/search",
                        self.collection
                    ))
                    .json(&request)
            })
            .await?
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
        let request = serde_json::json!({ "exact": true });
        let response = self
            .send_with_fallback(|base_url| {
                self.client
                    .post(format!(
                        "{base_url}/collections/{}/points/count",
                        self.collection
                    ))
                    .json(&request)
            })
            .await?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<CountResponse>()
            .await
            .map_err(|error| error.to_string())?;
        Ok(response.result.count)
    }

    async fn send_with_fallback(
        &self,
        build_request: impl Fn(&str) -> RequestBuilder,
    ) -> Result<Response, String> {
        let mut errors = Vec::new();

        for base_url in &self.base_urls {
            match build_request(base_url).send().await {
                Ok(response) => return Ok(response),
                Err(error) => errors.push(format!("{base_url}: {error}")),
            }
        }

        Err(format!(
            "Qdrant request failed for all configured URLs: {}",
            errors.join("; ")
        ))
    }
}

fn qdrant_base_urls(url: &str) -> Vec<String> {
    let primary = url.trim().trim_end_matches('/').to_string();
    let mut urls = vec![primary.clone()];
    if let Some(fallback) = qdrant_local_fallback(&primary) {
        urls.push(fallback);
    }
    urls
}

fn qdrant_local_fallback(base_url: &str) -> Option<String> {
    let mut url = Url::parse(base_url).ok()?;
    if url.host_str() != Some("qdrant") {
        return None;
    }
    url.set_host(Some("127.0.0.1")).ok()?;
    Some(url.as_str().trim_end_matches('/').to_string())
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

#[cfg(test)]
mod tests {
    use super::qdrant_base_urls;

    #[test]
    fn qdrant_service_hostname_falls_back_to_host_port() {
        assert_eq!(
            qdrant_base_urls("http://qdrant:6333/"),
            vec!["http://qdrant:6333", "http://127.0.0.1:6333"]
        );
    }

    #[test]
    fn non_compose_qdrant_urls_are_left_alone() {
        assert_eq!(
            qdrant_base_urls("http://localhost:6333"),
            vec!["http://localhost:6333"]
        );
        assert_eq!(
            qdrant_base_urls("http://qdrant.internal:6333"),
            vec!["http://qdrant.internal:6333"]
        );
    }
}
