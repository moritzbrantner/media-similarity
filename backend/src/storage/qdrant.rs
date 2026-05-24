use reqwest::{Client, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::domain::models::{FacePointPayload, ImagePayload};
use crate::storage::{MediaSearchFilter, MediaVectorStore, ScoredPoint, StoredPoint};

const EXPECTED_DISTANCE: &str = "Cosine";
const VISUAL_VECTOR_NAME: &str = "visual";
const FACE_VECTOR_NAME: &str = "face";

#[derive(Clone)]
pub struct QdrantImageStore {
    client: Client,
    base_urls: Vec<String>,
    collection: String,
    visual_vector_size: usize,
    face_vector_size: usize,
}

impl QdrantImageStore {
    pub fn new(
        url: impl Into<String>,
        collection: impl Into<String>,
        visual_vector_size: usize,
        face_vector_size: usize,
    ) -> Self {
        Self {
            client: Client::new(),
            base_urls: qdrant_base_urls(&url.into()),
            collection: collection.into(),
            visual_vector_size,
            face_vector_size,
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
            vectors: NamedVectors {
                visual: VectorParams {
                    size: self.visual_vector_size,
                    distance: EXPECTED_DISTANCE,
                },
                face: VectorParams {
                    size: self.face_vector_size,
                    distance: EXPECTED_DISTANCE,
                },
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
            self.visual_vector_size,
            self.face_vector_size,
            &response.result.config.params.vectors,
        )
    }

    pub async fn upsert_media(
        &self,
        payload: &ImagePayload,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        let mut payload_value = serde_json::to_value(payload).map_err(|error| error.to_string())?;
        set_payload_kind(&mut payload_value, "media");
        add_media_filter_payload_fields(&mut payload_value, payload);
        let request = UpsertRequest {
            points: vec![PointStruct {
                id: payload.id.clone(),
                vector: NamedPointVectors::visual(vector),
                payload: payload_value,
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

    pub async fn upsert_face(
        &self,
        payload: &FacePointPayload,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        let mut payload_value = serde_json::to_value(payload).map_err(|error| error.to_string())?;
        set_payload_kind(&mut payload_value, "face");
        let request = UpsertRequest {
            points: vec![PointStruct {
                id: payload.face_id.clone(),
                vector: NamedPointVectors::face(vector),
                payload: payload_value,
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

    pub async fn set_media_payload(&self, payload: &ImagePayload) -> Result<(), String> {
        let mut payload_value = serde_json::to_value(payload).map_err(|error| error.to_string())?;
        set_payload_kind(&mut payload_value, "media");
        add_media_filter_payload_fields(&mut payload_value, payload);
        let request = SetPayloadRequest {
            payload: payload_value,
            points: vec![payload.id.clone()],
        };
        self.send_with_fallback(|base_url| {
            self.client
                .post(format!(
                    "{base_url}/collections/{}/points/payload?wait=true",
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

    pub async fn delete_points_by_ids(&self, ids: &[String]) -> Result<(), String> {
        self.delete_points(ids).await
    }

    pub async fn search_visual(
        &self,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<ScoredPoint>, String> {
        self.search_named(VISUAL_VECTOR_NAME, vector, limit, Some("media"))
            .await
    }

    pub async fn search_visual_filtered(
        &self,
        vector: Vec<f32>,
        limit: u32,
        filter: Option<MediaSearchFilter>,
    ) -> Result<Vec<ScoredPoint>, String> {
        let filter = media_search_filter(filter);
        self.search_named_with_filter(VISUAL_VECTOR_NAME, vector, limit, filter)
            .await
    }

    pub async fn search_faces(
        &self,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<ScoredPoint>, String> {
        self.search_named(FACE_VECTOR_NAME, vector, limit, Some("face"))
            .await
    }

    async fn search_named(
        &self,
        name: &'static str,
        vector: Vec<f32>,
        limit: u32,
        point_kind: Option<&'static str>,
    ) -> Result<Vec<ScoredPoint>, String> {
        self.search_named_with_filter(name, vector, limit, point_kind.map(kind_filter))
            .await
    }

    async fn search_named_with_filter(
        &self,
        name: &'static str,
        vector: Vec<f32>,
        limit: u32,
        filter: Option<Filter>,
    ) -> Result<Vec<ScoredPoint>, String> {
        let request = SearchRequest {
            vector: NamedSearchVector { name, vector },
            limit,
            with_payload: true,
            filter,
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
            .scroll_media_points()
            .await?
            .into_iter()
            .filter_map(|point| point.payload)
            .collect())
    }

    pub async fn scroll_media_points(&self) -> Result<Vec<StoredPoint>, String> {
        self.scroll_points(Some("media")).await
    }

    pub async fn scroll_face_points(&self) -> Result<Vec<StoredPoint>, String> {
        self.scroll_points(Some("face")).await
    }

    pub async fn scroll_media_points_by_filter(
        &self,
        id: Option<&str>,
        source_uri: Option<&str>,
        source_item_uri: Option<&str>,
    ) -> Result<Vec<StoredPoint>, String> {
        let mut conditions = vec![field_condition("point_kind", "media")];
        if let Some(id) = id {
            conditions.push(field_condition("id", id));
        }
        if let Some(source_uri) = source_uri {
            conditions.push(field_condition("source_uri", source_uri));
        }
        if let Some(source_item_uri) = source_item_uri {
            conditions.push(field_condition("source_item_uri", source_item_uri));
        }
        self.scroll_points_with_filter(Some(Filter { must: conditions }))
            .await
    }

    pub async fn scroll_face_points_by_media_ids(
        &self,
        media_ids: &[String],
    ) -> Result<Vec<StoredPoint>, String> {
        if media_ids.is_empty() {
            return Ok(Vec::new());
        }
        let media_ids = media_ids
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        Ok(self
            .scroll_face_points()
            .await?
            .into_iter()
            .filter(|point| {
                point
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("media_id"))
                    .and_then(Value::as_str)
                    .map(|media_id| media_ids.contains(media_id))
                    .unwrap_or(false)
            })
            .collect())
    }

    async fn scroll_points(
        &self,
        point_kind: Option<&'static str>,
    ) -> Result<Vec<StoredPoint>, String> {
        self.scroll_points_with_filter(point_kind.map(kind_filter))
            .await
    }

    async fn scroll_points_with_filter(
        &self,
        filter: Option<Filter>,
    ) -> Result<Vec<StoredPoint>, String> {
        let mut offset = None;
        let mut points = Vec::new();

        loop {
            let request = ScrollRequest {
                limit: 256,
                with_payload: true,
                with_vector: false,
                offset: offset.clone(),
                filter: filter.clone(),
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

#[async_trait::async_trait]
impl MediaVectorStore for QdrantImageStore {
    async fn ensure_collection(&self) -> Result<(), String> {
        QdrantImageStore::ensure_collection(self).await
    }

    async fn upsert_media(&self, payload: &ImagePayload, vector: Vec<f32>) -> Result<(), String> {
        QdrantImageStore::upsert_media(self, payload, vector).await
    }

    async fn upsert_face(
        &self,
        payload: &FacePointPayload,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        QdrantImageStore::upsert_face(self, payload, vector).await
    }

    async fn set_media_payload(&self, payload: &ImagePayload) -> Result<(), String> {
        QdrantImageStore::set_media_payload(self, payload).await
    }

    async fn delete_points(&self, ids: &[String]) -> Result<(), String> {
        QdrantImageStore::delete_points(self, ids).await
    }

    async fn delete_points_by_ids(&self, ids: &[String]) -> Result<(), String> {
        QdrantImageStore::delete_points_by_ids(self, ids).await
    }

    async fn search_visual(
        &self,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<ScoredPoint>, String> {
        QdrantImageStore::search_visual(self, vector, limit).await
    }

    async fn search_visual_filtered(
        &self,
        vector: Vec<f32>,
        limit: u32,
        filter: Option<MediaSearchFilter>,
    ) -> Result<Vec<ScoredPoint>, String> {
        QdrantImageStore::search_visual_filtered(self, vector, limit, filter).await
    }

    async fn search_faces(&self, vector: Vec<f32>, limit: u32) -> Result<Vec<ScoredPoint>, String> {
        QdrantImageStore::search_faces(self, vector, limit).await
    }

    async fn scroll_media_points(&self) -> Result<Vec<StoredPoint>, String> {
        QdrantImageStore::scroll_media_points(self).await
    }

    async fn scroll_face_points(&self) -> Result<Vec<StoredPoint>, String> {
        QdrantImageStore::scroll_face_points(self).await
    }

    async fn scroll_media_points_by_filter(
        &self,
        id: Option<&str>,
        source_uri: Option<&str>,
        source_item_uri: Option<&str>,
    ) -> Result<Vec<StoredPoint>, String> {
        QdrantImageStore::scroll_media_points_by_filter(self, id, source_uri, source_item_uri).await
    }

    async fn scroll_face_points_by_media_ids(
        &self,
        media_ids: &[String],
    ) -> Result<Vec<StoredPoint>, String> {
        QdrantImageStore::scroll_face_points_by_media_ids(self, media_ids).await
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
    vectors: NamedVectors,
}

#[derive(Serialize)]
struct NamedVectors {
    visual: VectorParams,
    face: VectorParams,
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
struct SetPayloadRequest {
    payload: Value,
    points: Vec<String>,
}

#[derive(Serialize)]
struct PointStruct {
    id: String,
    vector: NamedPointVectors,
    payload: Value,
}

#[derive(Serialize)]
struct NamedPointVectors {
    visual: Option<Vec<f32>>,
    face: Option<Vec<f32>>,
}

impl NamedPointVectors {
    fn visual(vector: Vec<f32>) -> Self {
        Self {
            visual: Some(vector),
            face: None,
        }
    }

    fn face(vector: Vec<f32>) -> Self {
        Self {
            visual: None,
            face: Some(vector),
        }
    }
}

#[derive(Serialize)]
struct SearchRequest {
    vector: NamedSearchVector,
    limit: u32,
    with_payload: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<Filter>,
}

#[derive(Serialize)]
struct NamedSearchVector {
    name: &'static str,
    vector: Vec<f32>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<Filter>,
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
    expected_visual_size: usize,
    expected_face_size: usize,
    vectors: &Value,
) -> Result<(), String> {
    if unnamed_vector_schema(vectors).is_some() {
        return Err(legacy_collection_schema_error(collection));
    }

    let Some(visual_schema) = named_vector_schema(vectors, VISUAL_VECTOR_NAME) else {
        return Err(collection_schema_error(
            collection,
            expected_visual_size,
            expected_face_size,
            vector_schema_description(vectors),
        ));
    };
    let Some(face_schema) = named_vector_schema(vectors, FACE_VECTOR_NAME) else {
        return Err(collection_schema_error(
            collection,
            expected_visual_size,
            expected_face_size,
            vector_schema_description(vectors),
        ));
    };

    if visual_schema.size == expected_visual_size
        && visual_schema
            .distance
            .eq_ignore_ascii_case(EXPECTED_DISTANCE)
        && face_schema.size == expected_face_size
        && face_schema.distance.eq_ignore_ascii_case(EXPECTED_DISTANCE)
    {
        return Ok(());
    }

    Err(collection_schema_error(
        collection,
        expected_visual_size,
        expected_face_size,
        format!(
            "visual vector size {} with {} distance and face vector size {} with {} distance",
            visual_schema.size, visual_schema.distance, face_schema.size, face_schema.distance
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

fn named_vector_schema<'a>(vectors: &'a Value, name: &str) -> Option<VectorSchema<'a>> {
    let vector = vectors.get(name)?;
    Some(VectorSchema {
        size: vector.get("size")?.as_u64()?.try_into().ok()?,
        distance: vector.get("distance")?.as_str()?,
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

fn collection_schema_error(
    collection: &str,
    expected_visual_size: usize,
    expected_face_size: usize,
    found: String,
) -> String {
    format!(
        "Qdrant collection `{collection}` is incompatible with this service: expected named vectors `visual` size {expected_visual_size} and `face` size {expected_face_size} with {EXPECTED_DISTANCE} distance, found {found}. Recreate the collection, or set QDRANT_COLLECTION to a new empty collection name and re-index media."
    )
}

fn legacy_collection_schema_error(collection: &str) -> String {
    format!(
        "Collection {collection} uses legacy vector schema; reindex into a new collection or delete/recreate it."
    )
}

#[derive(Clone, Serialize)]
struct Filter {
    must: Vec<FieldCondition>,
}

#[derive(Clone, Serialize)]
struct FieldCondition {
    key: String,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    condition_match: Option<MatchValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<RangeCondition>,
}

#[derive(Clone, Serialize)]
struct RangeCondition {
    #[serde(skip_serializing_if = "Option::is_none")]
    gte: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lte: Option<f64>,
}

#[derive(Clone, Serialize)]
struct MatchValue {
    value: Value,
}

fn kind_filter(kind: &'static str) -> Filter {
    Filter {
        must: vec![field_condition("point_kind", kind)],
    }
}

fn media_search_filter(filter: Option<MediaSearchFilter>) -> Option<Filter> {
    let mut conditions = vec![field_condition("point_kind", "media")];
    let Some(filter) = filter else {
        return Some(Filter { must: conditions });
    };
    if let Some(source_type) = filter.source_type {
        conditions.push(field_condition("source_type", source_type));
    }
    if let Some(media_kind) = filter.media_kind {
        conditions.push(field_condition("media_kind", media_kind));
    }
    if let Some(has_gps) = filter.has_gps {
        conditions.push(bool_field_condition("photo_has_gps", has_gps));
    }
    push_range(
        &mut conditions,
        "width",
        filter.min_width.map(f64::from),
        filter.max_width.map(f64::from),
    );
    push_range(
        &mut conditions,
        "height",
        filter.min_height.map(f64::from),
        filter.max_height.map(f64::from),
    );
    push_range(
        &mut conditions,
        "size_bytes",
        filter.min_size_bytes.map(|value| value as f64),
        filter.max_size_bytes.map(|value| value as f64),
    );
    push_range(
        &mut conditions,
        "modified_at",
        filter.modified_from,
        filter.modified_to,
    );
    push_range(
        &mut conditions,
        "photo_capture_time_epoch",
        filter.captured_from,
        filter.captured_to,
    );
    Some(Filter { must: conditions })
}

fn push_range(
    conditions: &mut Vec<FieldCondition>,
    key: &'static str,
    gte: Option<f64>,
    lte: Option<f64>,
) {
    if gte.is_some() || lte.is_some() {
        conditions.push(range_field_condition(key, gte, lte));
    }
}

fn field_condition(key: impl Into<String>, value: impl Into<String>) -> FieldCondition {
    FieldCondition {
        key: key.into(),
        condition_match: Some(MatchValue {
            value: Value::String(value.into()),
        }),
        range: None,
    }
}

fn bool_field_condition(key: impl Into<String>, value: bool) -> FieldCondition {
    FieldCondition {
        key: key.into(),
        condition_match: Some(MatchValue {
            value: Value::Bool(value),
        }),
        range: None,
    }
}

fn range_field_condition(
    key: impl Into<String>,
    gte: Option<f64>,
    lte: Option<f64>,
) -> FieldCondition {
    FieldCondition {
        key: key.into(),
        condition_match: None,
        range: Some(RangeCondition { gte, lte }),
    }
}

fn set_payload_kind(payload: &mut Value, kind: &'static str) {
    if let Some(object) = payload.as_object_mut() {
        object.insert("point_kind".to_string(), Value::String(kind.to_string()));
    }
}

fn add_media_filter_payload_fields(payload: &mut Value, media: &ImagePayload) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let Some(metadata) = &media.photo_metadata else {
        object.insert("photo_has_gps".to_string(), Value::Bool(false));
        return;
    };
    object.insert(
        "photo_has_gps".to_string(),
        Value::Bool(metadata.gps.is_some()),
    );
    if let Some(capture_time) = metadata
        .capture_time
        .as_deref()
        .and_then(parse_capture_time_epoch)
    {
        if let Some(number) = serde_json::Number::from_f64(capture_time) {
            object.insert(
                "photo_capture_time_epoch".to_string(),
                Value::Number(number),
            );
        }
    }
    let camera_text = [
        metadata.camera_make.as_deref(),
        metadata.camera_model.as_deref(),
        metadata.lens_model.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    if !camera_text.is_empty() {
        object.insert("photo_camera_text".to_string(), Value::String(camera_text));
    }
    if !metadata.keywords.is_empty() {
        object.insert(
            "photo_keywords".to_string(),
            Value::Array(
                metadata
                    .keywords
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
}

fn parse_capture_time_epoch(value: &str) -> Option<f64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| {
            datetime.timestamp() as f64
                + f64::from(datetime.timestamp_subsec_nanos()) / 1_000_000_000.0
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{media_search_filter, qdrant_base_urls, validate_collection_vectors};
    use crate::storage::MediaSearchFilter;

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
        let vectors = json!({
            "visual": { "size": 512, "distance": "Cosine" },
            "face": { "size": 256, "distance": "Cosine" }
        });

        assert!(validate_collection_vectors("images", 512, 256, &vectors).is_ok());
    }

    #[test]
    fn mismatched_collection_schema_reports_remediation() {
        let vectors = json!({
            "visual": { "size": 384, "distance": "Dot" },
            "face": { "size": 256, "distance": "Cosine" }
        });

        let error = validate_collection_vectors("images", 512, 256, &vectors).unwrap_err();

        assert!(error.contains("collection `images` is incompatible"));
        assert!(error.contains("expected named vectors `visual` size 512 and `face` size 256"));
        assert!(error.contains("found visual vector size 384 with Dot distance"));
        assert!(error.contains("set QDRANT_COLLECTION to a new empty collection name"));
    }

    #[test]
    fn legacy_collection_schema_is_rejected() {
        let vectors = json!({ "size": 512, "distance": "Cosine" });

        let error = validate_collection_vectors("images", 512, 256, &vectors).unwrap_err();

        assert!(error.contains("uses legacy vector schema"));
    }

    #[test]
    fn media_search_filter_serializes_exact_and_range_conditions() {
        let filter = media_search_filter(Some(MediaSearchFilter {
            source_type: Some("s3".to_string()),
            media_kind: Some("static_image".to_string()),
            has_gps: Some(true),
            min_width: Some(640),
            max_width: Some(1920),
            modified_from: Some(1_700_000_000.0),
            ..MediaSearchFilter::default()
        }))
        .unwrap();

        let value = serde_json::to_value(filter).unwrap();
        assert_eq!(
            value,
            json!({
                "must": [
                    { "key": "point_kind", "match": { "value": "media" } },
                    { "key": "source_type", "match": { "value": "s3" } },
                    { "key": "media_kind", "match": { "value": "static_image" } },
                    { "key": "photo_has_gps", "match": { "value": true } },
                    { "key": "width", "range": { "gte": 640.0, "lte": 1920.0 } },
                    { "key": "modified_at", "range": { "gte": 1_700_000_000.0 } }
                ]
            })
        );
    }
}
