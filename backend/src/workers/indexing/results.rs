use crate::config::Settings;
use crate::domain::models::ImagePayload;

pub fn legacy_source_item_uri(payload: &crate::domain::models::ImagePayload) -> Option<String> {
    payload
        .source_item_uri
        .as_deref()
        .or(Some(payload.path.as_str()))
        .and_then(|value| {
            value.split_once('#').map_or_else(
                || Some(value.to_string()),
                |(value, _)| Some(value.to_string()),
            )
        })
}

pub fn payload_analysis_complete(payload: &ImagePayload, settings: &Settings) -> bool {
    if payload.media_kind == "video_scene"
        && (payload.scene_index.is_none()
            || payload.scene_start_seconds.is_none()
            || payload.scene_end_seconds.is_none())
    {
        return false;
    }

    if settings.audio_transcription_enabled
        && (payload.media_kind == "audio" || payload.media_kind == "video_scene")
    {
        let Some(analysis) = &payload.audio_analysis else {
            return payload.media_kind == "video_scene";
        };
        if analysis.speech_detected && analysis.transcript_text.trim().is_empty() {
            return false;
        }
    }

    true
}
