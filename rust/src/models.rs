use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioAnalysis {
    pub speech_detected: bool,
    pub speech_ratio: f32,
    pub speech_segments: Vec<AudioSpeechSegment>,
    #[serde(default)]
    pub audio_segments: Vec<AudioSegmentGuess>,
    #[serde(default)]
    pub recognized_voices: Vec<AudioRecognizedVoice>,
    #[serde(default)]
    pub transcript_text: String,
    #[serde(default)]
    pub transcript_language: Option<String>,
    #[serde(default)]
    pub transcript_segments: Vec<AudioTranscriptSegment>,
    pub tempo_bpm: Option<f32>,
    pub tempo_confidence: f32,
    pub tempo_onset_count: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioSpeechSegment {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub confidence: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioSegmentGuess {
    pub segment_index: usize,
    pub kind: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub confidence: f32,
    #[serde(default)]
    pub speaker_id: Option<String>,
    #[serde(default)]
    pub speaker_label: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioRecognizedVoice {
    pub id: String,
    pub label: String,
    pub segment_count: u32,
    pub total_seconds: f64,
    pub confidence: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioTranscriptSegment {
    pub segment_index: u64,
    #[serde(default)]
    pub start_seconds: Option<f64>,
    #[serde(default)]
    pub end_seconds: Option<f64>,
    pub text: String,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct OcrAnalysis {
    pub text: String,
    pub frames: Vec<OcrFrameText>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OcrFrameText {
    pub frame_index: usize,
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct FaceBoxPayload {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FaceDetectionPayload {
    pub face_id: String,
    pub media_id: String,
    pub frame_index: usize,
    pub bbox: FaceBoxPayload,
    pub confidence: f32,
    #[serde(default)]
    pub person_id: Option<String>,
    #[serde(default)]
    pub person_label: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PersonSummary {
    pub person_id: String,
    #[serde(default)]
    pub label: Option<String>,
    pub face_count: u32,
    pub media_count: u32,
    pub confidence: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FacePointPayload {
    pub face_id: String,
    pub media_id: String,
    pub frame_index: usize,
    pub bbox: FaceBoxPayload,
    pub confidence: f32,
    pub person_id: String,
    #[serde(default)]
    pub person_label: Option<String>,
    pub source_item_uri: Option<String>,
}

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
    pub full_audio_url: Option<String>,
    #[serde(default)]
    pub audio_analysis: Option<AudioAnalysis>,
    #[serde(default)]
    pub ocr_text: String,
    #[serde(default)]
    pub ocr_frames: Vec<OcrFrameText>,
    #[serde(default)]
    pub visual_embedding_model: Option<String>,
    #[serde(default)]
    pub faces: Vec<FaceDetectionPayload>,
    #[serde(default)]
    pub people: Vec<PersonSummary>,
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
    #[serde(default)]
    pub source_item_uri: Option<String>,
    #[serde(default)]
    pub indexing_profile: Option<String>,
    pub source_uri: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchResult {
    pub image: ImagePayload,
    pub vector_score: f32,
    pub hash_distance: Option<u32>,
    #[serde(default)]
    pub ocr_score: Option<f32>,
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
    #[serde(default)]
    pub query_audio_analysis: Option<AudioAnalysis>,
    #[serde(default)]
    pub query_ocr_text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchSceneResponse {
    pub scene_index: usize,
    #[serde(default = "default_scene_kind")]
    pub scene_kind: String,
    pub start_frame: u64,
    pub end_frame: u64,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub clip_url: Option<String>,
    #[serde(default)]
    pub speaker_id: Option<String>,
    #[serde(default)]
    pub speaker_label: Option<String>,
    pub query_phash: String,
    pub count: usize,
    pub results: Vec<SearchResult>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct IndexResponse {
    pub indexed: usize,
    pub skipped: usize,
    pub failed: usize,
    #[serde(default)]
    pub pruned: usize,
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

fn default_scene_kind() -> String {
    "scene".to_string()
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
        assert_eq!(payload.full_audio_url, None);
        assert_eq!(payload.audio_analysis, None);
        assert_eq!(payload.ocr_text, "");
        assert!(payload.ocr_frames.is_empty());
        assert_eq!(payload.visual_embedding_model, None);
        assert!(payload.faces.is_empty());
        assert!(payload.people.is_empty());
        assert_eq!(payload.scene_clip_url, None);
        assert_eq!(payload.scene_index, None);
        assert_eq!(payload.source_item_uri, None);
        assert_eq!(payload.indexing_profile, None);
    }

    #[test]
    fn response_defaults_match_api_contract() {
        let response = IndexResponse {
            indexed: 0,
            skipped: 0,
            failed: 0,
            pruned: 0,
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
            full_audio_url: None,
            audio_analysis: None,
            ocr_text: "SALE 50".to_string(),
            ocr_frames: vec![super::OcrFrameText {
                frame_index: 0,
                text: "SALE 50".to_string(),
            }],
            visual_embedding_model: Some("clip".to_string()),
            faces: Vec::new(),
            people: Vec::new(),
            scene_clip_url: None,
            scene_index: None,
            scene_start_frame: None,
            scene_end_frame: None,
            scene_start_seconds: None,
            scene_end_seconds: None,
            source_type: "local".to_string(),
            source_item_uri: Some("/images/clip.gif".to_string()),
            indexing_profile: Some("profile".to_string()),
            source_uri: Some("/images".to_string()),
        };
        let serialized = serde_json::to_value(payload).unwrap();
        assert_eq!(serialized["animated_thumbnail_url"], "/thumbnails/id.gif");
        assert_eq!(serialized["media_kind"], "animated_gif");
        assert_eq!(serialized["ocr_text"], "SALE 50");
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
            full_audio_url: None,
            audio_analysis: None,
            ocr_text: "Scene title".to_string(),
            ocr_frames: vec![super::OcrFrameText {
                frame_index: 0,
                text: "Scene title".to_string(),
            }],
            visual_embedding_model: Some("clip".to_string()),
            faces: Vec::new(),
            people: Vec::new(),
            scene_clip_url: Some("/uploads/source-scenes/id/scene-001.mp4".to_string()),
            scene_index: Some(0),
            scene_start_frame: Some(10),
            scene_end_frame: Some(25),
            scene_start_seconds: Some(1.0),
            scene_end_seconds: Some(2.5),
            source_type: "local".to_string(),
            source_item_uri: Some("/images/clip.mp4".to_string()),
            indexing_profile: Some("profile".to_string()),
            source_uri: Some("/images".to_string()),
        };
        let serialized = serde_json::to_value(payload).unwrap();
        assert_eq!(serialized["media_kind"], "video_scene");
        assert_eq!(
            serialized["full_video_url"],
            "/uploads/source-videos/id.mp4"
        );
        assert_eq!(serialized["ocr_text"], "Scene title");
        assert_eq!(serialized["scene_start_seconds"], 1.0);
    }

    #[test]
    fn audio_payload_serializes_audio_link() {
        let payload = ImagePayload {
            id: "id".to_string(),
            path: "/images/song.mp3".to_string(),
            relative_path: "song.mp3".to_string(),
            filename: "song.mp3".to_string(),
            width: 512,
            height: 256,
            size_bytes: 30,
            modified_at: 40.5,
            phash: "0000000000000000".to_string(),
            thumbnail_url: Some("/thumbnails/id.jpg".to_string()),
            animated_thumbnail_url: None,
            media_kind: "audio".to_string(),
            frame_count: None,
            duration_ms: Some(4200),
            full_video_url: None,
            full_audio_url: Some("/uploads/source-audio/id.mp3".to_string()),
            audio_analysis: Some(super::AudioAnalysis {
                speech_detected: true,
                speech_ratio: 0.4,
                speech_segments: vec![super::AudioSpeechSegment {
                    start_seconds: 0.5,
                    end_seconds: 2.0,
                    confidence: 0.2,
                }],
                audio_segments: vec![super::AudioSegmentGuess {
                    segment_index: 0,
                    kind: "speech".to_string(),
                    start_seconds: 0.5,
                    end_seconds: 2.0,
                    confidence: 0.2,
                    speaker_id: Some("voice-0001".to_string()),
                    speaker_label: Some("Voice 1".to_string()),
                }],
                recognized_voices: vec![super::AudioRecognizedVoice {
                    id: "voice-0001".to_string(),
                    label: "Voice 1".to_string(),
                    segment_count: 1,
                    total_seconds: 1.5,
                    confidence: 0.8,
                }],
                transcript_text: "hello from the audio".to_string(),
                transcript_language: Some("en".to_string()),
                transcript_segments: vec![super::AudioTranscriptSegment {
                    segment_index: 0,
                    start_seconds: Some(0.5),
                    end_seconds: Some(2.0),
                    text: "hello from the audio".to_string(),
                    confidence: Some(0.75),
                }],
                tempo_bpm: Some(120.0),
                tempo_confidence: 0.8,
                tempo_onset_count: 8,
            }),
            ocr_text: String::new(),
            ocr_frames: Vec::new(),
            visual_embedding_model: Some("clip".to_string()),
            faces: Vec::new(),
            people: Vec::new(),
            scene_clip_url: None,
            scene_index: None,
            scene_start_frame: None,
            scene_end_frame: None,
            scene_start_seconds: None,
            scene_end_seconds: None,
            source_type: "local".to_string(),
            source_item_uri: Some("/images/song.mp3".to_string()),
            indexing_profile: Some("profile".to_string()),
            source_uri: Some("/images".to_string()),
        };
        let serialized = serde_json::to_value(payload).unwrap();
        assert_eq!(serialized["media_kind"], "audio");
        assert_eq!(serialized["full_audio_url"], "/uploads/source-audio/id.mp3");
        assert_eq!(serialized["duration_ms"], 4200);
        assert_eq!(serialized["audio_analysis"]["speech_detected"], true);
        assert_eq!(serialized["audio_analysis"]["tempo_bpm"], 120.0);
        assert_eq!(
            serialized["audio_analysis"]["transcript_text"],
            "hello from the audio"
        );
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
        assert!(response.query_audio_analysis.is_none());
    }
}
