use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use super::{ApiError, AppState};
use crate::domain::models::ImagePayload;

#[derive(Debug, Serialize)]
pub struct InverseIndexResponse {
    pub indexed_media: usize,
    pub people: Vec<InversePersonEntry>,
    pub speakers: Vec<InverseSpeakerEntry>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct InversePersonEntry {
    pub id: String,
    pub label: Option<String>,
    pub face_count: u32,
    pub media_count: usize,
    pub confidence: f32,
    pub locations: Vec<InverseIndexLocation>,
}

#[derive(Debug, Serialize)]
pub struct InverseSpeakerEntry {
    pub id: String,
    pub label: Option<String>,
    pub segment_count: u32,
    pub total_seconds: f64,
    pub media_count: usize,
    pub confidence: f32,
    pub locations: Vec<InverseIndexLocation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InverseIndexLocation {
    pub media_id: String,
    pub filename: String,
    pub relative_path: String,
    pub path: String,
    pub media_kind: String,
    pub source_type: String,
    pub source_uri: Option<String>,
    pub source_item_uri: Option<String>,
    pub thumbnail_url: Option<String>,
    pub media_url: Option<String>,
    pub scene_clip_url: Option<String>,
    pub occurrence_count: u32,
    pub frame_indices: Vec<usize>,
    pub start_seconds: Option<f64>,
    pub end_seconds: Option<f64>,
    pub page_number: Option<usize>,
    pub confidence: f32,
}

#[derive(Default)]
struct PersonAccumulator {
    label: Option<String>,
    face_count: u32,
    confidence_total: f32,
    confidence_count: u32,
    locations: BTreeMap<String, InverseIndexLocation>,
}

#[derive(Default)]
struct SpeakerAccumulator {
    label: Option<String>,
    segment_count: u32,
    total_seconds: f64,
    confidence_total: f32,
    confidence_count: u32,
    locations: BTreeMap<String, InverseIndexLocation>,
}

#[derive(Default)]
struct PersonMediaStats {
    label: Option<String>,
    face_count: u32,
    confidence_total: f32,
    confidence_count: u32,
    frame_indices: Vec<usize>,
}

pub async fn inverse_index(
    State(state): State<Arc<AppState>>,
) -> Result<Json<InverseIndexResponse>, ApiError> {
    let points = state
        .store
        .scroll_media_points()
        .await
        .map_err(ApiError::internal)?;
    let mut people = BTreeMap::<String, PersonAccumulator>::new();
    let mut speakers = BTreeMap::<String, SpeakerAccumulator>::new();
    let mut indexed_media = 0;
    let mut errors = Vec::new();

    for point in points {
        let Some(payload_value) = point.payload else {
            errors.push(format!("{}: missing payload", point.id));
            continue;
        };
        let payload = match serde_json::from_value::<ImagePayload>(payload_value) {
            Ok(payload) => payload,
            Err(error) => {
                errors.push(format!("{}: could not decode payload: {error}", point.id));
                continue;
            }
        };

        indexed_media += 1;
        collect_people(&payload, &mut people);
        collect_speakers(&payload, &mut speakers);
    }

    Ok(Json(InverseIndexResponse {
        indexed_media,
        people: people
            .into_iter()
            .map(|(id, entry)| InversePersonEntry {
                confidence: average_confidence(entry.confidence_total, entry.confidence_count),
                face_count: entry.face_count,
                id,
                label: entry.label,
                media_count: entry.locations.len(),
                locations: sorted_locations(entry.locations),
            })
            .collect(),
        speakers: speakers
            .into_iter()
            .map(|(id, entry)| InverseSpeakerEntry {
                confidence: average_confidence(entry.confidence_total, entry.confidence_count),
                id,
                label: entry.label,
                media_count: entry.locations.len(),
                segment_count: entry.segment_count,
                total_seconds: entry.total_seconds,
                locations: sorted_locations(entry.locations),
            })
            .collect(),
        errors,
    }))
}

fn collect_people(payload: &ImagePayload, people: &mut BTreeMap<String, PersonAccumulator>) {
    let mut by_person = BTreeMap::<String, PersonMediaStats>::new();

    for face in &payload.faces {
        let Some(person_id) = face.person_id.as_deref().filter(|id| !id.is_empty()) else {
            continue;
        };
        let stats = by_person.entry(person_id.to_string()).or_default();
        stats.label = stats.label.clone().or_else(|| face.person_label.clone());
        stats.face_count += 1;
        stats.confidence_total += face.confidence;
        stats.confidence_count += 1;
        if !stats.frame_indices.contains(&face.frame_index) {
            stats.frame_indices.push(face.frame_index);
        }
    }

    for person in &payload.people {
        let person_id = person.person_id.trim();
        if person_id.is_empty() {
            continue;
        }
        let stats = by_person.entry(person_id.to_string()).or_default();
        stats.label = stats.label.clone().or_else(|| person.label.clone());
        stats.face_count = stats.face_count.max(person.face_count);
        stats.confidence_total += person.confidence;
        stats.confidence_count += 1;
    }

    for (person_id, mut stats) in by_person {
        stats.frame_indices.sort_unstable();
        let confidence = average_confidence(stats.confidence_total, stats.confidence_count);
        let entry = people.entry(person_id).or_default();
        entry.label = entry.label.clone().or(stats.label);
        entry.face_count += stats.face_count;
        entry.confidence_total += stats.confidence_total;
        entry.confidence_count += stats.confidence_count;

        let location = entry
            .locations
            .entry(payload.id.clone())
            .or_insert_with(|| base_location(payload));
        location.occurrence_count += stats.face_count.max(1);
        location.confidence = location.confidence.max(confidence);
        for frame_index in stats.frame_indices {
            if !location.frame_indices.contains(&frame_index) {
                location.frame_indices.push(frame_index);
            }
        }
        location.frame_indices.sort_unstable();
    }
}

fn collect_speakers(payload: &ImagePayload, speakers: &mut BTreeMap<String, SpeakerAccumulator>) {
    let Some(analysis) = &payload.audio_analysis else {
        return;
    };

    for voice in &analysis.recognized_voices {
        let voice_id = voice.id.trim();
        if voice_id.is_empty() {
            continue;
        }

        let mut segment_count = 0;
        let mut start_seconds = None::<f64>;
        let mut end_seconds = None::<f64>;
        for segment in analysis.audio_segments.iter().filter(|segment| {
            segment
                .speaker_id
                .as_deref()
                .map(|speaker_id| speaker_id == voice_id)
                .unwrap_or(false)
        }) {
            segment_count += 1;
            start_seconds = Some(
                start_seconds
                    .map(|current| current.min(segment.start_seconds))
                    .unwrap_or(segment.start_seconds),
            );
            end_seconds = Some(
                end_seconds
                    .map(|current| current.max(segment.end_seconds))
                    .unwrap_or(segment.end_seconds),
            );
        }

        let entry = speakers.entry(voice_id.to_string()).or_default();
        entry.label = entry.label.clone().or_else(|| Some(voice.label.clone()));
        entry.segment_count += voice.segment_count;
        entry.total_seconds += voice.total_seconds;
        entry.confidence_total += voice.confidence;
        entry.confidence_count += 1;

        let location = entry
            .locations
            .entry(payload.id.clone())
            .or_insert_with(|| base_location(payload));
        location.occurrence_count += segment_count.max(voice.segment_count).max(1);
        location.start_seconds = min_optional(location.start_seconds, start_seconds);
        location.end_seconds = max_optional(location.end_seconds, end_seconds);
        location.confidence = location.confidence.max(voice.confidence);
    }
}

fn base_location(payload: &ImagePayload) -> InverseIndexLocation {
    InverseIndexLocation {
        media_id: payload.id.clone(),
        filename: payload.filename.clone(),
        relative_path: payload.relative_path.clone(),
        path: payload.path.clone(),
        media_kind: payload.media_kind.clone(),
        source_type: payload.source_type.clone(),
        source_uri: payload.source_uri.clone(),
        source_item_uri: payload.source_item_uri.clone(),
        thumbnail_url: payload
            .animated_thumbnail_url
            .clone()
            .or_else(|| payload.thumbnail_url.clone()),
        media_url: payload
            .full_audio_url
            .clone()
            .or_else(|| payload.full_video_url.clone())
            .or_else(|| payload.pdf_page_url.clone())
            .or_else(|| payload.full_pdf_url.clone()),
        scene_clip_url: payload.scene_clip_url.clone(),
        occurrence_count: 0,
        frame_indices: Vec::new(),
        start_seconds: payload.scene_start_seconds,
        end_seconds: payload.scene_end_seconds,
        page_number: payload.pdf_page_number,
        confidence: 0.0,
    }
}

fn sorted_locations(
    locations: BTreeMap<String, InverseIndexLocation>,
) -> Vec<InverseIndexLocation> {
    let mut locations: Vec<_> = locations.into_values().collect();
    locations.sort_by(|left, right| {
        left.relative_path
            .cmp(&right.relative_path)
            .then_with(|| left.media_id.cmp(&right.media_id))
    });
    locations
}

fn average_confidence(total: f32, count: u32) -> f32 {
    if count == 0 {
        0.0
    } else {
        total / count as f32
    }
}

fn min_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn max_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}
