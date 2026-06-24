use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode as AxumStatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpListener;

use image_similarity_service::domain::models::ImagePayload;

pub struct FakeQdrant {
    pub base_url: String,
    state: Arc<Mutex<FakeQdrantState>>,
}

#[derive(Default)]
struct FakeQdrantState {
    collections: BTreeMap<String, FakeCollection>,
    points: BTreeMap<(String, String), FakePoint>,
    operation_counts: FakeQdrantOperationCounts,
    upsert_delay_ms: u64,
}

struct FakeCollection {
    vectors: Value,
    payload_schema: BTreeMap<String, Value>,
}

#[derive(Clone)]
struct FakePoint {
    vector: Vec<f32>,
    payload: Value,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FakeQdrantOperationCounts {
    pub upserted_points: usize,
    pub deleted_points: usize,
}

#[derive(Deserialize)]
struct FakeCreateCollectionRequest {
    vectors: Value,
}

#[derive(Deserialize)]
struct FakeCreatePayloadIndexRequest {
    field_name: String,
    field_schema: Value,
}

#[derive(Deserialize)]
struct FakeUpsertRequest {
    points: Vec<FakeUpsertPoint>,
}

#[derive(Deserialize)]
struct FakeUpsertPoint {
    id: String,
    vector: Value,
    payload: Value,
}

#[derive(Deserialize)]
struct FakeSearchRequest {
    vector: Value,
    limit: u32,
    filter: Option<Value>,
}

#[derive(Deserialize)]
struct FakeScrollRequest {
    limit: u32,
    offset: Option<Value>,
    filter: Option<Value>,
}

#[derive(Deserialize)]
struct FakeDeleteRequest {
    points: Vec<String>,
}

#[derive(Deserialize)]
struct FakeSetPayloadRequest {
    payload: Value,
    points: Vec<String>,
}

impl FakeQdrant {
    pub async fn spawn() -> Self {
        let state = Arc::new(Mutex::new(FakeQdrantState::default()));
        let app = Router::new()
            .route("/collections", get(fake_list_collections))
            .route(
                "/collections/:collection",
                get(fake_get_collection).put(fake_create_collection),
            )
            .route(
                "/collections/:collection/index",
                put(fake_create_payload_index),
            )
            .route("/collections/:collection/points", put(fake_upsert_points))
            .route(
                "/collections/:collection/points/payload",
                post(fake_set_payload),
            )
            .route(
                "/collections/:collection/points/delete",
                post(fake_delete_points),
            )
            .route(
                "/collections/:collection/points/scroll",
                post(fake_scroll_points),
            )
            .route(
                "/collections/:collection/points/search",
                post(fake_search_points),
            )
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base_url: format!("http://{addr}"),
            state,
        }
    }

    pub fn payload_schema(&self, collection: &str) -> BTreeMap<String, Value> {
        self.state
            .lock()
            .unwrap()
            .collections
            .get(collection)
            .map(|collection| collection.payload_schema.clone())
            .unwrap_or_default()
    }

    pub fn media_payloads(&self, collection: &str) -> Vec<ImagePayload> {
        let state = self.state.lock().unwrap();
        let mut payloads = state
            .points
            .iter()
            .filter(|((point_collection, _), _)| point_collection == collection)
            .filter_map(|(_, point)| serde_json::from_value(point.payload.clone()).ok())
            .collect::<Vec<ImagePayload>>();
        payloads.sort_by(|left, right| left.filename.cmp(&right.filename));
        payloads
    }

    pub fn operation_counts(&self) -> FakeQdrantOperationCounts {
        self.state.lock().unwrap().operation_counts
    }

    pub fn delay_upserts(&self, delay: Duration) {
        self.state.lock().unwrap().upsert_delay_ms = delay.as_millis() as u64;
    }
}

async fn fake_list_collections(State(state): State<Arc<Mutex<FakeQdrantState>>>) -> Json<Value> {
    let state = state.lock().unwrap();
    let collections = state
        .collections
        .keys()
        .map(|name| json!({ "name": name }))
        .collect::<Vec<_>>();
    Json(json!({ "result": { "collections": collections } }))
}

async fn fake_create_collection(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeCreateCollectionRequest>,
) -> Json<Value> {
    state.lock().unwrap().collections.insert(
        collection,
        FakeCollection {
            vectors: request.vectors,
            payload_schema: BTreeMap::new(),
        },
    );
    Json(json!({ "result": true }))
}

async fn fake_create_payload_index(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeCreatePayloadIndexRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    if collection.starts_with("fail-payload-index") {
        return Err(AxumStatusCode::SERVICE_UNAVAILABLE);
    }
    let mut state = state.lock().unwrap();
    let Some(collection) = state.collections.get_mut(&collection) else {
        return Err(AxumStatusCode::NOT_FOUND);
    };
    let data_type = request
        .field_schema
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            request
                .field_schema
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .ok_or(AxumStatusCode::UNPROCESSABLE_ENTITY)?;
    collection
        .payload_schema
        .insert(request.field_name, json!({ "data_type": data_type }));
    Ok(Json(json!({ "result": { "status": "completed" } })))
}

async fn fake_get_collection(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
) -> Result<Json<Value>, AxumStatusCode> {
    let state = state.lock().unwrap();
    let Some(collection) = state.collections.get(&collection) else {
        return Err(AxumStatusCode::NOT_FOUND);
    };
    Ok(Json(json!({
        "result": {
            "payload_schema": &collection.payload_schema,
            "config": {
                "params": {
                    "vectors": &collection.vectors
                }
            }
        }
    })))
}

async fn fake_upsert_points(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeUpsertRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    let upsert_delay_ms = state.lock().unwrap().upsert_delay_ms;
    if upsert_delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(upsert_delay_ms)).await;
    }

    let mut state = state.lock().unwrap();
    if !state.collections.contains_key(&collection) {
        return Err(AxumStatusCode::NOT_FOUND);
    }
    for point in request.points {
        let vector = named_vector(&point.vector).ok_or(AxumStatusCode::UNPROCESSABLE_ENTITY)?;
        state.operation_counts.upserted_points += 1;
        state.points.insert(
            (collection.clone(), point.id),
            FakePoint {
                vector,
                payload: point.payload,
            },
        );
    }
    Ok(Json(json!({ "result": { "status": "completed" } })))
}

async fn fake_set_payload(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeSetPayloadRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    let mut state = state.lock().unwrap();
    if !state.collections.contains_key(&collection) {
        return Err(AxumStatusCode::NOT_FOUND);
    }
    for id in request.points {
        let Some(point) = state.points.get_mut(&(collection.clone(), id)) else {
            continue;
        };
        point.payload = request.payload.clone();
    }
    Ok(Json(json!({ "result": { "status": "completed" } })))
}

async fn fake_delete_points(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeDeleteRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    let mut state = state.lock().unwrap();
    if !state.collections.contains_key(&collection) {
        return Err(AxumStatusCode::NOT_FOUND);
    }
    for id in request.points {
        if state.points.remove(&(collection.clone(), id)).is_some() {
            state.operation_counts.deleted_points += 1;
        }
    }
    Ok(Json(json!({ "result": { "status": "completed" } })))
}

async fn fake_search_points(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeSearchRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    let state = state.lock().unwrap();
    if !state.collections.contains_key(&collection) {
        return Err(AxumStatusCode::NOT_FOUND);
    }
    let mut scored = state
        .points
        .iter()
        .filter(|((point_collection, _), _)| point_collection == &collection)
        .filter(|(_, point)| payload_matches_filter(&point.payload, request.filter.as_ref()))
        .map(|((_, id), point)| {
            json!({
                "id": id,
                "score": cosine_similarity(&named_vector(&request.vector).unwrap_or_default(), &point.vector),
                "payload": point.payload,
            })
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right["score"]
            .as_f64()
            .unwrap()
            .total_cmp(&left["score"].as_f64().unwrap())
    });
    scored.truncate(request.limit as usize);
    Ok(Json(json!({ "result": scored })))
}

async fn fake_scroll_points(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeScrollRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    let state = state.lock().unwrap();
    if !state.collections.contains_key(&collection) {
        return Err(AxumStatusCode::NOT_FOUND);
    }
    let offset = request.offset.as_ref().and_then(Value::as_str);
    let mut points = state
        .points
        .iter()
        .filter(|((point_collection, id), _)| {
            point_collection == &collection
                && offset.map(|offset| id.as_str() > offset).unwrap_or(true)
        })
        .filter(|(_, point)| payload_matches_filter(&point.payload, request.filter.as_ref()))
        .map(|((_, id), point)| {
            json!({
                "id": id,
                "payload": point.payload,
            })
        })
        .collect::<Vec<_>>();
    points.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let limit = request.limit as usize;
    let next_page_offset = if points.len() > limit {
        points
            .get(limit - 1)
            .and_then(|point| point["id"].as_str())
            .map(|id| json!(id))
    } else {
        None
    };
    points.truncate(limit);
    Ok(Json(json!({
        "result": {
            "points": points,
            "next_page_offset": next_page_offset,
        }
    })))
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let len = left.len().min(right.len());
    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for index in 0..len {
        dot += left[index] * right[index];
        left_norm += left[index] * left[index];
        right_norm += right[index] * right[index];
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

fn named_vector(value: &Value) -> Option<Vec<f32>> {
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .map(|value| value.as_f64().map(|number| number as f32))
            .collect();
    }
    if let Some(vector) = value.get("vector") {
        return named_vector(vector);
    }
    for name in ["visual", "face"] {
        if let Some(vector) = value.get(name) {
            return named_vector(vector);
        }
    }
    None
}

fn payload_matches_filter(payload: &Value, filter: Option<&Value>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let Some(must) = filter.get("must").and_then(Value::as_array) else {
        return true;
    };
    must.iter().all(|condition| {
        let Some(key) = condition.get("key").and_then(Value::as_str) else {
            return true;
        };
        let actual = payload_value(payload, key);
        if let Some(expected) = condition.get("match").and_then(|value| value.get("value")) {
            return actual.map(|actual| actual == expected).unwrap_or(false);
        }
        if let Some(range) = condition.get("range") {
            let Some(actual) = actual.and_then(Value::as_f64) else {
                return false;
            };
            if let Some(gte) = range.get("gte").and_then(Value::as_f64) {
                if actual < gte {
                    return false;
                }
            }
            if let Some(lte) = range.get("lte").and_then(Value::as_f64) {
                if actual > lte {
                    return false;
                }
            }
        }
        true
    })
}

fn payload_value<'a>(payload: &'a Value, key: &str) -> Option<&'a Value> {
    let mut value = payload;
    for part in key.split('.') {
        value = value.get(part)?;
    }
    Some(value)
}
