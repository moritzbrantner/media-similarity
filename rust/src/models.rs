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
    #[serde(default)]
    pub animated_thumbnail_url: Option<String>,
    #[serde(default = "default_media_kind")]
    pub media_kind: String,
    #[serde(default)]
    pub frame_count: Option<u32>,
    #[serde(default)]
    pub duration_ms: Option<u32>,
    #[serde(default)]
    pub full_video_url: Option<String>,
    #[serde(default)]
    pub scene_clip_url: Option<String>,
    #[serde(default)]
    pub scene_index: Option<usize>,
    #[serde(default)]
    pub scene_start_frame: Option<u64>,
    #[serde(default)]
    pub scene_end_frame: Option<u64>,
    #[serde(default)]
    pub scene_start_seconds: Option<f64>,
    #[serde(default)]
    pub scene_end_seconds: Option<f64>,
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
    #[serde(default)]
    pub query_scene_index: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchResponse {
    pub query_phash: String,
    pub count: usize,
    pub results: Vec<SearchResult>,
    #[serde(default = "default_query_media_kind")]
    pub query_media_kind: String,
    #[serde(default)]
    pub scenes: Vec<SearchSceneResponse>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchSceneResponse {
    pub scene_index: usize,
    pub start_frame: u64,
    pub end_frame: u64,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub clip_url: Option<String>,
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

fn default_media_kind() -> String {
    "static_image".to_string()
}

fn default_query_media_kind() -> String {
    "static_image".to_string()
}

#[cfg(test)]
mod tests {
    use super::{HealthResponse, ImagePayload, IndexResponse};

    #[test]
    fn image_payload_defaults_match_api_contract() {
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
        assert_eq!(payload.animated_thumbnail_url, None);
        assert_eq!(payload.media_kind, "static_image");
        assert_eq!(payload.frame_count, None);
        assert_eq!(payload.duration_ms, None);
        assert_eq!(payload.full_video_url, None);
        assert_eq!(payload.scene_clip_url, None);
        assert_eq!(payload.scene_index, None);
    }

    #[test]
    fn response_defaults_match_api_contract() {
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

    #[test]
    fn gif_payload_serializes_media_metadata() {
        let payload = ImagePayload {
            id: "id".to_string(),
            path: "/images/clip.gif".to_string(),
            relative_path: "clip.gif".to_string(),
            filename: "clip.gif".to_string(),
            width: 10,
            height: 20,
            size_bytes: 30,
            modified_at: 40.5,
            phash: "0000000000000000".to_string(),
            thumbnail_url: Some("/thumbnails/id.jpg".to_string()),
            animated_thumbnail_url: Some("/thumbnails/id.gif".to_string()),
            media_kind: "animated_gif".to_string(),
            frame_count: Some(6),
            duration_ms: Some(600),
            full_video_url: None,
            scene_clip_url: None,
            scene_index: None,
            scene_start_frame: None,
            scene_end_frame: None,
            scene_start_seconds: None,
            scene_end_seconds: None,
            source_type: "local".to_string(),
            source_uri: Some("/images".to_string()),
        };
        let serialized = serde_json::to_value(payload).unwrap();
        assert_eq!(serialized["animated_thumbnail_url"], "/thumbnails/id.gif");
        assert_eq!(serialized["media_kind"], "animated_gif");
        assert_eq!(serialized["frame_count"], 6);
        assert_eq!(serialized["duration_ms"], 600);
    }

    #[test]
    fn video_scene_payload_serializes_links_and_time_window() {
        let payload = ImagePayload {
            id: "id".to_string(),
            path: "/images/clip.mp4#scene=1".to_string(),
            relative_path: "clip.mp4#scene=1".to_string(),
            filename: "clip.mp4 scene 1".to_string(),
            width: 10,
            height: 20,
            size_bytes: 30,
            modified_at: 40.5,
            phash: "0000000000000000".to_string(),
            thumbnail_url: Some("/thumbnails/id.jpg".to_string()),
            animated_thumbnail_url: None,
            media_kind: "video_scene".to_string(),
            frame_count: Some(2),
            duration_ms: Some(1200),
            full_video_url: Some("/uploads/source-videos/id.mp4".to_string()),
            scene_clip_url: Some("/uploads/source-scenes/id/scene-001.mp4".to_string()),
            scene_index: Some(0),
            scene_start_frame: Some(10),
            scene_end_frame: Some(25),
            scene_start_seconds: Some(1.0),
            scene_end_seconds: Some(2.5),
            source_type: "local".to_string(),
            source_uri: Some("/images".to_string()),
        };
        let serialized = serde_json::to_value(payload).unwrap();
        assert_eq!(serialized["media_kind"], "video_scene");
        assert_eq!(
            serialized["full_video_url"],
            "/uploads/source-videos/id.mp4"
        );
        assert_eq!(serialized["scene_start_seconds"], 1.0);
    }

    #[test]
    fn search_response_defaults_match_existing_api_contract() {
        let json = r#"{
            "query_phash": "0000000000000000",
            "count": 0,
            "results": []
        }"#;
        let response: super::SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.query_media_kind, "static_image");
        assert!(response.scenes.is_empty());
    }
}
