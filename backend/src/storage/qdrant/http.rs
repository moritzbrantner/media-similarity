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

    async fn set_face_payload(&self, payload: &FacePointPayload) -> Result<(), String> {
        QdrantImageStore::set_face_payload(self, payload).await
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

fn qdrant_http_client(options: &QdrantHttpOptions) -> Client {
    Client::builder()
        .timeout(Duration::from_millis(options.request_timeout_ms))
        .connect_timeout(Duration::from_millis(options.connect_timeout_ms))
        .build()
        .expect("Qdrant HTTP client options are valid")
}

#[derive(Debug, Error)]
#[error(
    "Qdrant {operation} failed for collection {collection} at {url} after {attempts} attempt(s): {kind}"
)]
struct QdrantHttpError {
    operation: &'static str,
    collection: String,
    url: String,
    attempts: u32,
    status: Option<StatusCode>,
    kind: QdrantHttpErrorKind,
}

impl QdrantHttpError {
    fn request(
        operation: &'static str,
        collection: &str,
        url: String,
        attempts: u32,
        source: String,
    ) -> Self {
        Self {
            operation,
            collection: collection.to_string(),
            url,
            attempts,
            status: None,
            kind: QdrantHttpErrorKind::Request { source },
        }
    }

    fn http(
        operation: &'static str,
        collection: &str,
        url: String,
        attempts: u32,
        status: StatusCode,
        body: String,
    ) -> Self {
        Self {
            operation,
            collection: collection.to_string(),
            url,
            attempts,
            status: Some(status),
            kind: QdrantHttpErrorKind::Http {
                status,
                body: response_body_snippet(&body),
            },
        }
    }
}

#[derive(Debug)]
enum QdrantHttpErrorKind {
    Request { source: String },
    Http { status: StatusCode, body: String },
}

impl fmt::Display for QdrantHttpErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request { source } => write!(formatter, "{source}"),
            Self::Http { status, body } if body.is_empty() => write!(formatter, "HTTP {status}"),
            Self::Http { status, body } => write!(formatter, "HTTP {status}: {body}"),
        }
    }
}

#[derive(Debug, Error)]
#[error("Qdrant {operation} returned invalid JSON for collection {collection} at {url}: {detail}")]
struct QdrantJsonError {
    operation: &'static str,
    collection: String,
    url: String,
    detail: String,
}

fn response_body_snippet(body: &str) -> String {
    const MAX_BODY_CHARS: usize = 512;
    let trimmed = body.trim();
    let snippet = trimmed.chars().take(MAX_BODY_CHARS).collect::<String>();
    if trimmed.chars().count() > MAX_BODY_CHARS {
        format!("{snippet}...")
    } else {
        snippet
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn retry_delay_ms(base_delay_ms: u64, attempt_index: u32) -> u64 {
    base_delay_ms
        .saturating_mul(2_u64.saturating_pow(attempt_index))
        .min(MAX_RETRY_BACKOFF_MS)
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
    #[serde(default)]
    payload_schema: Option<Value>,
}
