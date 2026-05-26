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
struct CreatePayloadIndexRequest {
    field_name: &'static str,
    field_schema: PayloadFieldSchema,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    visual: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
