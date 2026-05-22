use reqwest::{Client, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::models::ImagePayload;

const EXPECTED_DISTANCE: &str = "Cosine";

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

#[derive(Clone, Debug)]
pub struct StoredPoint {
    pub id: String,
    pub payload: Option<Value>,
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
            self.validate_collection_schema().await?;
            return Ok(());
        }

        let request = CreateCollectionRequest {
            vectors: VectorParams {
                size: self.vector_size,
                distance: EXPECTED_DISTANCE,
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

    async fn validate_collection_schema(&self) -> Result<(), String> {
        let response = self
            .send_with_fallback(|base_url| {
                self.client
                    .get(format!("{base_url}/collections/{}", self.collection))
            })
            .await?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<CollectionInfoResponse>()
            .await
            .map_err(|error| error.to_string())?;

        validate_collection_vectors(
            &self.collection,
            self.vector_size,
            &response.result.config.params.vectors,
        )
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

    pub async fn delete_points(&self, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let request = DeletePointsRequest {
            points: ids.to_vec(),
        };
        self.send_with_fallback(|base_url| {
            self.client
                .post(format!(
                    "{base_url}/collections/{}/points/delete?wait=true",
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
    pub async fn scroll_payloads(&self) -> Result<Vec<Value>, String> {
        Ok(self
            .scroll_points()
            .await?
            .into_iter()
            .filter_map(|point| point.payload)
            .collect())
    }

    pub async fn scroll_points(&self) -> Result<Vec<StoredPoint>, String> {
        let mut offset = None;
        let mut points = Vec::new();

        loop {
            let request = ScrollRequest {
                limit: 256,
                with_payload: true,
                with_vector: false,
                offset: offset.clone(),
            };
            let response = self
                .send_with_fallback(|base_url| {
                    self.client
                        .post(format!(
                            "{base_url}/collections/{}/points/scroll",
                            self.collection
                        ))
                        .json(&request)
                })
                .await?
                .error_for_status()
                .map_err(|error| error.to_string())?
                .json::<ScrollResponse>()
                .await
                .map_err(|error| error.to_string())?;

            points.extend(response.result.points.into_iter().map(|point| StoredPoint {
                id: point.id,
                payload: point.payload,
            }));

            match response.result.next_page_offset {
                Some(next_offset) => offset = Some(next_offset),
                None => break,
            }
        }

        Ok(points)
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

#[derive(Deserialize)]
struct CollectionInfoResponse {
    result: CollectionInfoResult,
}

#[derive(Deserialize)]
struct CollectionInfoResult {
    config: CollectionConfig,
}

#[derive(Deserialize)]
struct CollectionConfig {
    params: CollectionParams,
}

#[derive(Deserialize)]
struct CollectionParams {
    vectors: Value,
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
struct DeletePointsRequest {
    points: Vec<String>,
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

#[derive(Serialize)]
struct ScrollRequest {
    limit: u32,
    with_payload: bool,
    with_vector: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<Value>,
}

#[derive(Deserialize)]
struct ScrollResponse {
    result: ScrollResult,
}

#[derive(Deserialize)]
struct ScrollResult {
    points: Vec<ScrollPoint>,
    next_page_offset: Option<Value>,
}

#[derive(Deserialize)]
struct ScrollPoint {
    id: String,
    payload: Option<Value>,
}

#[derive(Deserialize)]
struct CountResponse {
    result: CountResult,
}

#[derive(Deserialize)]
struct CountResult {
    count: u64,
}

fn validate_collection_vectors(
    collection: &str,
    expected_size: usize,
    vectors: &Value,
) -> Result<(), String> {
    let Some(schema) = unnamed_vector_schema(vectors) else {
        return Err(collection_schema_error(
            collection,
            expected_size,
            vector_schema_description(vectors),
        ));
    };

    if schema.size == expected_size && schema.distance.eq_ignore_ascii_case(EXPECTED_DISTANCE) {
        return Ok(());
    }

    Err(collection_schema_error(
        collection,
        expected_size,
        format!(
            "unnamed vector size {} with {} distance",
            schema.size, schema.distance
        ),
    ))
}

struct VectorSchema<'a> {
    size: usize,
    distance: &'a str,
}

fn unnamed_vector_schema(vectors: &Value) -> Option<VectorSchema<'_>> {
    Some(VectorSchema {
        size: vectors.get("size")?.as_u64()?.try_into().ok()?,
        distance: vectors.get("distance")?.as_str()?,
    })
}

fn vector_schema_description(vectors: &Value) -> String {
    if vectors
        .as_object()
        .map(|object| object.values().all(Value::is_object))
        .unwrap_or(false)
    {
        return "named vectors or unsupported vector schema".to_string();
    }

    "unsupported vector schema".to_string()
}

fn collection_schema_error(collection: &str, expected_size: usize, found: String) -> String {
    format!(
        "Qdrant collection `{collection}` is incompatible with this service: expected unnamed vector size {expected_size} with {EXPECTED_DISTANCE} distance, found {found}. This can happen after changing VECTOR_SIZE or embedder settings. Recreate the collection, or set QDRANT_COLLECTION to a new empty collection name and re-index media."
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{qdrant_base_urls, validate_collection_vectors};

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

    #[test]
    fn matching_collection_schema_is_valid() {
        let vectors = json!({ "size": 512, "distance": "Cosine" });

        assert!(validate_collection_vectors("images", 512, &vectors).is_ok());
    }

    #[test]
    fn mismatched_collection_schema_reports_remediation() {
        let vectors = json!({ "size": 384, "distance": "Dot" });

        let error = validate_collection_vectors("images", 512, &vectors).unwrap_err();

        assert!(error.contains("collection `images` is incompatible"));
        assert!(error.contains("expected unnamed vector size 512 with Cosine distance"));
        assert!(error.contains("found unnamed vector size 384 with Dot distance"));
        assert!(error.contains("set QDRANT_COLLECTION to a new empty collection name"));
    }

    #[test]
    fn named_collection_schema_is_rejected() {
        let vectors = json!({ "clip": { "size": 512, "distance": "Cosine" } });

        let error = validate_collection_vectors("images", 512, &vectors).unwrap_err();

        assert!(error.contains("named vectors or unsupported vector schema"));
    }
}
