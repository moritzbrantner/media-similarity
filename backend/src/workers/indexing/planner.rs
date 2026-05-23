use crate::config::Settings;
use crate::domain::models::ImagePayload;
use crate::workers::sources::SourceImage;

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

pub fn source_is_current(
    indexed_records: &[IndexedSourceRecord],
    source_image: &SourceImage,
    indexing_profile: &str,
) -> bool {
    indexed_records
        .iter()
        .any(|record| record_is_current(record, source_image, indexing_profile))
}

pub fn record_is_current(
    record: &IndexedSourceRecord,
    source_image: &SourceImage,
    indexing_profile: &str,
) -> bool {
    record.size_bytes == source_image.size_bytes
        && (record.modified_at - source_image.modified_at).abs() <= 0.001
        && record.indexing_profile.as_deref() == Some(indexing_profile)
        && record.analysis_complete
}

pub fn payload_analysis_complete(payload: &ImagePayload, settings: &Settings) -> bool {
    if payload.media_kind == "video_scene"
        && (payload.scene_index.is_none()
            || payload.scene_start_seconds.is_none()
            || payload.scene_end_seconds.is_none())
    {
        return false;
    }

    if settings.audio_transcription_enabled && payload.media_kind == "audio" {
        let Some(analysis) = &payload.audio_analysis else {
            return false;
        };
        if analysis.speech_detected && analysis.transcript_text.trim().is_empty() {
            return false;
        }
    }

    true
}

pub fn legacy_source_item_uri(payload: &ImagePayload) -> Option<String> {
    let source_path = payload
        .path
        .split_once('#')
        .map_or(payload.path.as_str(), |(path, _)| path);
    if source_path.is_empty() {
        None
    } else {
        Some(source_path.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{record_is_current, IndexedSourceRecord};
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
}
