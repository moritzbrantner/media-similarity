use crate::workers::sources::SourceImage;

#[path = "plan_builders.rs"]
mod plan_builders;
#[path = "record_match.rs"]
mod record_match;
#[path = "results.rs"]
mod results;

pub use plan_builders::{committed_records_are_current, source_is_current};
pub use record_match::{record_is_current, source_signature_matches};
pub use results::legacy_source_item_uri;
pub use results::payload_analysis_complete;

pub struct SourceIndexPlan {
    pub source_uris: Vec<String>,
    pub pending: Vec<PendingSource>,
    pub already_indexed: usize,
    pub skipped: usize,
    pub prune_point_ids: Vec<String>,
    pub errors: Vec<String>,
}

pub struct PendingSource {
    pub source_image: SourceImage,
    pub indexed_point_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct IndexedSourceRecord {
    pub point_id: String,
    pub size_bytes: u64,
    pub modified_at: f64,
    pub indexing_profile: Option<String>,
    pub analysis_complete: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        legacy_source_item_uri, payload_analysis_complete, record_is_current, source_is_current,
        IndexedSourceRecord,
    };
    use crate::config::Settings;
    use crate::domain::models::{AudioAnalysis, ImagePayload};
    use crate::workers::sources::SourceImage;

    fn source_image() -> SourceImage {
        SourceImage::test_local_image("/images/cat.jpg", 42, 100.0)
    }

    #[test]
    fn current_records_match_source_stat_profile_and_analysis() {
        let record = IndexedSourceRecord {
            point_id: "point".to_string(),
            size_bytes: 42,
            modified_at: 100.0,
            indexing_profile: Some("profile".to_string()),
            analysis_complete: true,
        };

        assert!(record_is_current(&record, &source_image(), "profile"));
    }

    #[test]
    fn stale_records_fail_when_analysis_is_incomplete() {
        let record = IndexedSourceRecord {
            point_id: "point".to_string(),
            size_bytes: 42,
            modified_at: 100.0,
            indexing_profile: Some("profile".to_string()),
            analysis_complete: false,
        };

        assert!(!record_is_current(&record, &source_image(), "profile"));
    }

    #[test]
    fn source_is_current_accepts_any_matching_current_record() {
        let stale = IndexedSourceRecord {
            point_id: "stale".to_string(),
            size_bytes: 41,
            modified_at: 100.0,
            indexing_profile: Some("profile".to_string()),
            analysis_complete: true,
        };
        let current = IndexedSourceRecord {
            point_id: "current".to_string(),
            size_bytes: 42,
            modified_at: 100.0,
            indexing_profile: Some("profile".to_string()),
            analysis_complete: true,
        };

        assert!(source_is_current(
            &[stale, current],
            &source_image(),
            "profile"
        ));
    }

    #[test]
    fn committed_records_require_every_point_to_be_current() {
        let first = IndexedSourceRecord {
            point_id: "first".to_string(),
            size_bytes: 42,
            modified_at: 100.0,
            indexing_profile: Some("profile".to_string()),
            analysis_complete: true,
        };
        let second = IndexedSourceRecord {
            point_id: "second".to_string(),
            size_bytes: 42,
            modified_at: 100.0,
            indexing_profile: Some("profile".to_string()),
            analysis_complete: true,
        };

        assert!(super::committed_records_are_current(
            &[first.clone(), second.clone()],
            &["first".to_string(), "second".to_string()],
            &source_image(),
            "profile"
        ));
        assert!(!super::committed_records_are_current(
            &[first, second],
            &["first".to_string(), "missing".to_string()],
            &source_image(),
            "profile"
        ));
    }

    #[test]
    fn payload_analysis_completion_covers_video_and_audio_transcription() {
        let settings = Settings {
            audio_transcription_enabled: true,
            ..Settings::default()
        };

        let mut video = media("video_scene");
        assert!(!payload_analysis_complete(&video, &settings));
        video.scene_index = Some(0);
        video.scene_start_seconds = Some(0.0);
        video.scene_end_seconds = Some(1.0);
        assert!(payload_analysis_complete(&video, &settings));

        let mut audio = media("audio");
        audio.audio_analysis = Some(AudioAnalysis {
            speech_detected: true,
            speech_ratio: 1.0,
            speech_segments: Vec::new(),
            audio_segments: Vec::new(),
            recognized_voices: Vec::new(),
            transcript_text: String::new(),
            transcript_language: None,
            transcript_segments: Vec::new(),
            tempo_bpm: None,
            tempo_confidence: 0.0,
            tempo_onset_count: 0,
        });
        assert!(!payload_analysis_complete(&audio, &settings));
        audio.audio_analysis.as_mut().unwrap().transcript_text = "hello".to_string();
        assert!(payload_analysis_complete(&audio, &settings));
    }

    #[test]
    fn legacy_source_item_uri_strips_generated_fragments() {
        let mut payload = media("pdf_page");
        payload.path = "/images/doc.pdf#page=1".to_string();

        assert_eq!(
            legacy_source_item_uri(&payload).as_deref(),
            Some("/images/doc.pdf")
        );
    }

    fn media(kind: &str) -> ImagePayload {
        ImagePayload {
            id: "media".to_string(),
            path: "/images/media".to_string(),
            relative_path: "media".to_string(),
            filename: "media".to_string(),
            width: 10,
            height: 8,
            size_bytes: 42,
            modified_at: 100.0,
            phash: "0000000000000000".to_string(),
            thumbnail_url: None,
            animated_thumbnail_url: None,
            media_kind: kind.to_string(),
            frame_count: None,
            duration_ms: None,
            full_video_url: None,
            full_audio_url: None,
            full_pdf_url: None,
            pdf_page_url: None,
            pdf_document_id: None,
            pdf_page_index: None,
            pdf_page_number: None,
            pdf_page_count: None,
            audio_analysis: None,
            ocr_text: String::new(),
            ocr_frames: Vec::new(),
            visual_embedding_model: None,
            faces: Vec::new(),
            people: Vec::new(),
            artifacts: Vec::new(),
            tags: Vec::new(),
            photo_metadata: None,
            scene_clip_url: None,
            scene_index: None,
            scene_start_frame: None,
            scene_end_frame: None,
            scene_start_seconds: None,
            scene_end_seconds: None,
            source_type: "local".to_string(),
            source_item_uri: None,
            indexing_profile: None,
            source_uri: None,
        }
    }
}
