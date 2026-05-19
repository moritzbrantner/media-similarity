use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ImagePayload {
    pub id: String,
    pub path: String,
    pub relative_path: String,
    pub filename: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub modified_at: f64,
    pub phash: String,
    pub thumbnail_url: Option<String>,
    #[serde(default = "default_source_type")]
    pub source_type: String,
    pub source_uri: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchResult {
    pub image: ImagePayload,
    pub vector_score: f32,
    pub hash_distance: Option<u32>,
    #[serde(default)]
    pub near_duplicate: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchResponse {
    pub query_phash: String,
    pub count: usize,
    pub results: Vec<SearchResult>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct IndexResponse {
    pub indexed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub collection: String,
    pub source_dir: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub collection: String,
    pub source_dir: String,
    #[serde(default)]
    pub sources: Vec<String>,
}

fn default_source_type() -> String {
    "local".to_string()
}

#[cfg(test)]
mod tests {
    use super::{HealthResponse, ImagePayload, IndexResponse};

    #[test]
    fn image_payload_defaults_match_python_contract() {
        let json = r#"{
            "id": "id",
            "path": "/images/cat.jpg",
            "relative_path": "cat.jpg",
            "filename": "cat.jpg",
            "width": 10,
            "height": 20,
            "size_bytes": 30,
            "modified_at": 40.5,
            "phash": "0000000000000000",
            "thumbnail_url": null,
            "source_uri": null
        }"#;
        let payload: ImagePayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.source_type, "local");
        assert_eq!(payload.relative_path, "cat.jpg");
    }

    #[test]
    fn response_defaults_match_python_contract() {
        let response = IndexResponse {
            indexed: 0,
            skipped: 0,
            failed: 0,
            collection: "image_similarity".to_string(),
            source_dir: "/images".to_string(),
            sources: Vec::new(),
            errors: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(response).unwrap()["errors"],
            serde_json::json!([])
        );

        let health = HealthResponse {
            status: "ok".to_string(),
            collection: "image_similarity".to_string(),
            source_dir: "/images".to_string(),
            sources: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(health).unwrap()["sources"],
            serde_json::json!([])
        );
    }
}
