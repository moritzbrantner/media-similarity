use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::config::Settings;
use crate::domain::models::{AudioRecognizedVoice, FacePointPayload, ImagePayload, PersonSummary};
use crate::storage::{MediaVectorStore, StoredPoint};
use crate::workers::media::persons::summarize_people;
use crate::workers::media::voice::VoiceRegistry;

const MAX_LABEL_CHARS: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IdentityKind {
    Person,
    Speaker,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IdentityMutationResponse {
    pub kind: IdentityKind,
    pub target_id: String,
    pub target_label: Option<String>,
    pub source_ids: Vec<String>,
    pub updated_media: usize,
    pub updated_faces: usize,
    pub registry_updated: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum IdentityMutationError {
    NotFound(String),
    Internal(String),
}

pub fn normalize_label(label: Option<String>) -> Result<String, String> {
    let Some(label) = label else {
        return Err("label is required".to_string());
    };
    let label = label.trim().to_string();
    if label.is_empty() {
        return Err("label must not be empty".to_string());
    }
    if label.chars().any(char::is_control) {
        return Err("label must not contain control characters".to_string());
    }
    if label.chars().count() > MAX_LABEL_CHARS {
        return Err(format!(
            "label must be at most {MAX_LABEL_CHARS} characters"
        ));
    }
    Ok(label)
}

pub fn normalize_source_ids(
    target_id: &str,
    source_ids: Option<Vec<String>>,
) -> Result<Vec<String>, String> {
    let Some(source_ids) = source_ids else {
        return Err("source_ids is required".to_string());
    };
    if source_ids.is_empty() {
        return Err("source_ids must not be empty".to_string());
    }

    let target_id = target_id.trim();
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for source_id in source_ids {
        let source_id = source_id.trim();
        if source_id.is_empty() {
            return Err("source_ids must not contain empty identities".to_string());
        }
        if source_id == target_id {
            return Err("source_ids must not contain the target identity".to_string());
        }
        if seen.insert(source_id.to_string()) {
            normalized.push(source_id.to_string());
        }
    }

    Ok(normalized)
}

pub async fn rename_person_identity(
    store: &dyn MediaVectorStore,
    target_id: &str,
    label: &str,
) -> Result<IdentityMutationResponse, IdentityMutationError> {
    let media_points = decode_media_points(store.scroll_media_points().await?)?;
    let face_points = decode_face_points(store.scroll_face_points().await?)?;
    let mut target_found = false;
    let mut updated_media = 0;
    let mut updated_faces = 0;

    for mut payload in media_points {
        let mut changed = false;
        for face in &mut payload.faces {
            if face.person_id.as_deref() == Some(target_id) {
                target_found = true;
                if face.person_label.as_deref() != Some(label) {
                    face.person_label = Some(label.to_string());
                    changed = true;
                }
            }
        }
        for person in &mut payload.people {
            if person.person_id == target_id {
                target_found = true;
                if person.label.as_deref() != Some(label) {
                    person.label = Some(label.to_string());
                    changed = true;
                }
            }
        }
        if changed {
            store.set_media_payload(&payload).await?;
            updated_media += 1;
        }
    }

    for mut payload in face_points {
        if payload.person_id == target_id {
            target_found = true;
            if payload.person_label.as_deref() != Some(label) {
                payload.person_label = Some(label.to_string());
                store.set_face_payload(&payload).await?;
                updated_faces += 1;
            }
        }
    }

    if !target_found {
        return Err(IdentityMutationError::NotFound(format!(
            "person identity `{target_id}` was not found"
        )));
    }

    Ok(IdentityMutationResponse {
        kind: IdentityKind::Person,
        target_id: target_id.to_string(),
        target_label: Some(label.to_string()),
        source_ids: Vec::new(),
        updated_media,
        updated_faces,
        registry_updated: false,
        warnings: Vec::new(),
    })
}

pub async fn merge_person_identities(
    store: &dyn MediaVectorStore,
    target_id: &str,
    source_ids: &[String],
) -> Result<IdentityMutationResponse, IdentityMutationError> {
    let mut media_points = decode_media_points(store.scroll_media_points().await?)?;
    let mut face_points = decode_face_points(store.scroll_face_points().await?)?;
    let source_set = source_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut target_found = false;
    let target_label = resolve_person_target_label(target_id, &media_points, &face_points);
    let mut updated_media = 0;
    let mut updated_faces = 0;

    for payload in &media_points {
        if media_payload_has_person(payload, target_id) {
            target_found = true;
            break;
        }
    }
    if !target_found {
        target_found = face_points
            .iter()
            .any(|payload| payload.person_id == target_id);
    }
    if !target_found {
        return Err(IdentityMutationError::NotFound(format!(
            "person identity `{target_id}` was not found"
        )));
    }

    for payload in &mut media_points {
        let original_people = payload.people.clone();
        let mut changed = false;
        for face in &mut payload.faces {
            if face
                .person_id
                .as_ref()
                .is_some_and(|person_id| source_set.contains(person_id))
            {
                face.person_id = Some(target_id.to_string());
                face.person_label = target_label.clone();
                changed = true;
            }
        }

        let next_people = rebuild_people_after_merge(
            target_id,
            &target_label,
            &source_set,
            &payload.faces,
            &original_people,
        );
        if next_people != payload.people {
            payload.people = next_people;
            changed = true;
        }

        if changed {
            store.set_media_payload(payload).await?;
            updated_media += 1;
        }
    }

    for payload in &mut face_points {
        if source_set.contains(&payload.person_id) {
            payload.person_id = target_id.to_string();
            payload.person_label = target_label.clone();
            store.set_face_payload(payload).await?;
            updated_faces += 1;
        }
    }

    Ok(IdentityMutationResponse {
        kind: IdentityKind::Person,
        target_id: target_id.to_string(),
        target_label,
        source_ids: source_ids.to_vec(),
        updated_media,
        updated_faces,
        registry_updated: false,
        warnings: Vec::new(),
    })
}

pub async fn rename_speaker_identity(
    store: &dyn MediaVectorStore,
    settings: &Settings,
    target_id: &str,
    label: &str,
) -> Result<IdentityMutationResponse, IdentityMutationError> {
    let media_points = decode_media_points(store.scroll_media_points().await?)?;
    let mut target_found = false;
    let mut updated_media = 0;
    let mut warnings = Vec::new();

    for mut payload in media_points {
        let Some(analysis) = &mut payload.audio_analysis else {
            continue;
        };
        let mut changed = false;
        for voice in &mut analysis.recognized_voices {
            if voice.id == target_id {
                target_found = true;
                if voice.label != label {
                    voice.label = label.to_string();
                    changed = true;
                }
            }
        }
        for segment in &mut analysis.audio_segments {
            if segment.speaker_id.as_deref() == Some(target_id) {
                target_found = true;
                if segment.speaker_label.as_deref() != Some(label) {
                    segment.speaker_label = Some(label.to_string());
                    changed = true;
                }
            }
        }
        if changed {
            store.set_media_payload(&payload).await?;
            updated_media += 1;
        }
    }

    let mut registry = VoiceRegistry::load(settings).map_err(IdentityMutationError::Internal)?;
    let registry_had_file = registry.loaded_from_file();
    let registry_has_target = registry.label(target_id)?.is_some();
    target_found |= registry_has_target;
    if !target_found {
        return Err(IdentityMutationError::NotFound(format!(
            "speaker identity `{target_id}` was not found"
        )));
    }

    let registry_updated = registry.rename(target_id, label)?;
    if registry_updated {
        registry
            .save_if_changed()
            .map_err(IdentityMutationError::Internal)?;
    } else if !registry_had_file {
        warnings.push("voice registry file was not found".to_string());
    } else if !registry_has_target {
        warnings.push(format!(
            "voice registry profile `{target_id}` was not found"
        ));
    }

    Ok(IdentityMutationResponse {
        kind: IdentityKind::Speaker,
        target_id: target_id.to_string(),
        target_label: Some(label.to_string()),
        source_ids: Vec::new(),
        updated_media,
        updated_faces: 0,
        registry_updated,
        warnings,
    })
}

pub async fn merge_speaker_identities(
    store: &dyn MediaVectorStore,
    settings: &Settings,
    target_id: &str,
    source_ids: &[String],
) -> Result<IdentityMutationResponse, IdentityMutationError> {
    let mut media_points = decode_media_points(store.scroll_media_points().await?)?;
    let source_set = source_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut registry = VoiceRegistry::load(settings).map_err(IdentityMutationError::Internal)?;
    let registry_label = registry.label(target_id)?;
    let target_label =
        resolve_speaker_target_label(target_id, &media_points).or(registry_label.clone());
    let target_found = target_label.is_some()
        || media_points
            .iter()
            .any(|payload| media_payload_has_speaker(payload, target_id));
    if !target_found {
        return Err(IdentityMutationError::NotFound(format!(
            "speaker identity `{target_id}` was not found"
        )));
    }
    let target_label = target_label.unwrap_or_else(|| target_id.to_string());
    let mut updated_media = 0;

    for payload in &mut media_points {
        let Some(analysis) = &mut payload.audio_analysis else {
            continue;
        };
        let original_voices = analysis.recognized_voices.clone();
        let mut changed = false;
        for segment in &mut analysis.audio_segments {
            if segment
                .speaker_id
                .as_ref()
                .is_some_and(|speaker_id| source_set.contains(speaker_id))
            {
                segment.speaker_id = Some(target_id.to_string());
                segment.speaker_label = Some(target_label.clone());
                changed = true;
            }
        }

        let merged_voices =
            merge_recognized_voices(target_id, &target_label, &source_set, &original_voices);
        if merged_voices != analysis.recognized_voices {
            analysis.recognized_voices = merged_voices;
            changed = true;
        }

        if changed {
            store.set_media_payload(payload).await?;
            updated_media += 1;
        }
    }

    let registry_outcome = registry.merge(target_id, source_ids, &target_label)?;
    let mut warnings = registry_outcome.warnings;
    if registry_outcome.changed {
        registry
            .save_if_changed()
            .map_err(IdentityMutationError::Internal)?;
    } else if !registry.loaded_from_file() {
        warnings.push("voice registry file was not found".to_string());
    }

    Ok(IdentityMutationResponse {
        kind: IdentityKind::Speaker,
        target_id: target_id.to_string(),
        target_label: Some(target_label),
        source_ids: source_ids.to_vec(),
        updated_media,
        updated_faces: 0,
        registry_updated: registry_outcome.changed,
        warnings,
    })
}

fn decode_media_points(
    points: Vec<StoredPoint>,
) -> Result<Vec<ImagePayload>, IdentityMutationError> {
    points
        .into_iter()
        .map(|point| {
            let payload = point.payload.ok_or_else(|| {
                IdentityMutationError::Internal(format!("{}: missing payload", point.id))
            })?;
            serde_json::from_value::<ImagePayload>(payload).map_err(|error| {
                IdentityMutationError::Internal(format!(
                    "{}: could not decode media payload: {error}",
                    point.id
                ))
            })
        })
        .collect()
}

fn decode_face_points(
    points: Vec<StoredPoint>,
) -> Result<Vec<FacePointPayload>, IdentityMutationError> {
    points
        .into_iter()
        .map(|point| {
            let payload = point.payload.ok_or_else(|| {
                IdentityMutationError::Internal(format!("{}: missing payload", point.id))
            })?;
            serde_json::from_value::<FacePointPayload>(payload).map_err(|error| {
                IdentityMutationError::Internal(format!(
                    "{}: could not decode face payload: {error}",
                    point.id
                ))
            })
        })
        .collect()
}

fn media_payload_has_person(payload: &ImagePayload, target_id: &str) -> bool {
    payload
        .faces
        .iter()
        .any(|face| face.person_id.as_deref() == Some(target_id))
        || payload
            .people
            .iter()
            .any(|person| person.person_id == target_id)
}

fn media_payload_has_speaker(payload: &ImagePayload, target_id: &str) -> bool {
    payload.audio_analysis.as_ref().is_some_and(|analysis| {
        analysis
            .recognized_voices
            .iter()
            .any(|voice| voice.id == target_id)
            || analysis
                .audio_segments
                .iter()
                .any(|segment| segment.speaker_id.as_deref() == Some(target_id))
    })
}

fn resolve_person_target_label(
    target_id: &str,
    media_points: &[ImagePayload],
    face_points: &[FacePointPayload],
) -> Option<String> {
    media_points
        .iter()
        .flat_map(|payload| &payload.people)
        .find_map(|person| {
            (person.person_id == target_id)
                .then(|| non_empty_string(person.label.as_deref()))
                .flatten()
        })
        .or_else(|| {
            media_points
                .iter()
                .flat_map(|payload| &payload.faces)
                .find_map(|face| {
                    (face.person_id.as_deref() == Some(target_id))
                        .then(|| non_empty_string(face.person_label.as_deref()))
                        .flatten()
                })
        })
        .or_else(|| {
            face_points.iter().find_map(|face| {
                (face.person_id == target_id)
                    .then(|| non_empty_string(face.person_label.as_deref()))
                    .flatten()
            })
        })
}

fn resolve_speaker_target_label(target_id: &str, media_points: &[ImagePayload]) -> Option<String> {
    media_points
        .iter()
        .filter_map(|payload| payload.audio_analysis.as_ref())
        .flat_map(|analysis| &analysis.recognized_voices)
        .find_map(|voice| {
            (voice.id == target_id)
                .then(|| non_empty_string(Some(&voice.label)))
                .flatten()
        })
        .or_else(|| {
            media_points
                .iter()
                .filter_map(|payload| payload.audio_analysis.as_ref())
                .flat_map(|analysis| &analysis.audio_segments)
                .find_map(|segment| {
                    (segment.speaker_id.as_deref() == Some(target_id))
                        .then(|| non_empty_string(segment.speaker_label.as_deref()))
                        .flatten()
                })
        })
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn rebuild_people_after_merge(
    target_id: &str,
    target_label: &Option<String>,
    source_set: &BTreeSet<String>,
    faces: &[crate::domain::models::FaceDetectionPayload],
    original_people: &[PersonSummary],
) -> Vec<PersonSummary> {
    let mut people = summarize_people(faces);
    let mut existing_ids = people
        .iter()
        .map(|person| person.person_id.clone())
        .collect::<BTreeSet<_>>();

    for person in original_people {
        let next_id = if source_set.contains(&person.person_id) {
            target_id
        } else {
            person.person_id.as_str()
        };
        if existing_ids.contains(next_id) {
            continue;
        }
        let mut next = person.clone();
        next.person_id = next_id.to_string();
        if next.person_id == target_id {
            next.label = target_label.clone();
        }
        people.push(next);
        existing_ids.insert(next_id.to_string());
    }

    people
}

fn merge_recognized_voices(
    target_id: &str,
    target_label: &str,
    source_set: &BTreeSet<String>,
    voices: &[AudioRecognizedVoice],
) -> Vec<AudioRecognizedVoice> {
    let mut accumulators = BTreeMap::<String, VoiceAccumulator>::new();
    for voice in voices {
        let id = if source_set.contains(&voice.id) {
            target_id
        } else {
            voice.id.as_str()
        };
        let entry = accumulators.entry(id.to_string()).or_default();
        entry.label = if id == target_id {
            Some(target_label.to_string())
        } else {
            entry.label.clone().or_else(|| Some(voice.label.clone()))
        };
        entry.segment_count += voice.segment_count;
        entry.total_seconds += voice.total_seconds;
        if voice.segment_count == 0 {
            entry.confidence_total += voice.confidence;
            entry.confidence_weight += 1;
        } else {
            entry.confidence_total += voice.confidence * voice.segment_count as f32;
            entry.confidence_weight += voice.segment_count;
        }
    }

    accumulators
        .into_iter()
        .map(|(id, entry)| AudioRecognizedVoice {
            id,
            label: entry.label.unwrap_or_else(|| target_label.to_string()),
            segment_count: entry.segment_count,
            total_seconds: round_millis(entry.total_seconds),
            confidence: if entry.confidence_weight == 0 {
                0.0
            } else {
                entry.confidence_total / entry.confidence_weight as f32
            },
        })
        .collect()
}

fn round_millis(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[derive(Default)]
struct VoiceAccumulator {
    label: Option<String>,
    segment_count: u32,
    total_seconds: f64,
    confidence_total: f32,
    confidence_weight: u32,
}

impl From<String> for IdentityMutationError {
    fn from(error: String) -> Self {
        Self::Internal(error)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use serde_json::to_value;

    use super::*;
    use crate::domain::models::{
        AudioAnalysis, AudioSegmentGuess, FaceBoxPayload, FaceDetectionPayload,
    };
    use crate::storage::{MediaSearchFilter, ScoredPoint};

    #[test]
    fn label_normalization_trims_and_rejects_invalid_labels() {
        assert_eq!(
            normalize_label(Some("  Ada Lovelace  ".to_string())).unwrap(),
            "Ada Lovelace"
        );
        assert!(normalize_label(Some("   ".to_string())).is_err());
        assert!(normalize_label(Some("Ada\nLovelace".to_string())).is_err());
        assert!(normalize_label(Some("a".repeat(81))).is_err());
        assert!(normalize_label(None).is_err());
    }

    #[test]
    fn merge_validation_rejects_target_id_in_sources() {
        assert_eq!(
            normalize_source_ids(
                "person-a",
                Some(vec![
                    " person-b ".to_string(),
                    "person-b".to_string(),
                    "person-c".to_string()
                ])
            )
            .unwrap(),
            vec!["person-b".to_string(), "person-c".to_string()]
        );
        assert!(normalize_source_ids("person-a", Some(vec![])).is_err());
        assert!(normalize_source_ids("person-a", Some(vec!["person-a".to_string()])).is_err());
    }

    #[tokio::test]
    async fn person_rename_updates_media_faces_people_and_face_points() {
        let store = TestStore::new(
            vec![person_media_payload()],
            vec![test_face_point("face-a", "person-a", Some("Ada"))],
        );

        let response = rename_person_identity(&store, "person-a", "Ada Lovelace")
            .await
            .unwrap();

        assert_eq!(response.updated_media, 1);
        assert_eq!(response.updated_faces, 1);
        let media = store.media.lock().unwrap();
        assert_eq!(
            media[0].faces[0].person_label.as_deref(),
            Some("Ada Lovelace")
        );
        assert_eq!(media[0].people[0].label.as_deref(), Some("Ada Lovelace"));
        let faces = store.faces.lock().unwrap();
        assert_eq!(faces[0].person_label.as_deref(), Some("Ada Lovelace"));
    }

    #[tokio::test]
    async fn person_merge_rewrites_sources_and_rebuilds_summaries() {
        let store = TestStore::new(
            vec![person_media_payload()],
            vec![
                test_face_point("face-a", "person-a", Some("Ada")),
                test_face_point("face-b", "person-b", Some("Grace")),
            ],
        );

        let response = merge_person_identities(&store, "person-a", &["person-b".to_string()])
            .await
            .unwrap();

        assert_eq!(response.updated_media, 1);
        assert_eq!(response.updated_faces, 1);
        let media = store.media.lock().unwrap();
        assert!(media[0]
            .faces
            .iter()
            .all(|face| face.person_id.as_deref() == Some("person-a")));
        assert_eq!(media[0].people.len(), 1);
        assert_eq!(media[0].people[0].person_id, "person-a");
        assert_eq!(media[0].people[0].face_count, 2);
    }

    #[tokio::test]
    async fn speaker_rename_updates_recognized_voices_and_segments() {
        let store = TestStore::new(vec![speaker_media_payload()], Vec::new());
        let settings = Settings {
            voice_registry_path: temp_path("missing-voices.json"),
            ..Settings::default()
        };

        let response = rename_speaker_identity(&store, &settings, "voice-0001", "Alice")
            .await
            .unwrap();

        assert_eq!(response.updated_media, 1);
        assert!(!response.registry_updated);
        let media = store.media.lock().unwrap();
        let analysis = media[0].audio_analysis.as_ref().unwrap();
        assert_eq!(analysis.recognized_voices[0].label, "Alice");
        assert_eq!(
            analysis.audio_segments[0].speaker_label.as_deref(),
            Some("Alice")
        );
    }

    #[tokio::test]
    async fn speaker_merge_combines_voice_stats_and_rewrites_segments() {
        let store = TestStore::new(vec![speaker_media_payload()], Vec::new());
        let settings = Settings {
            voice_registry_path: temp_path("missing-voices.json"),
            ..Settings::default()
        };

        let response =
            merge_speaker_identities(&store, &settings, "voice-0001", &["voice-0002".to_string()])
                .await
                .unwrap();

        assert_eq!(response.updated_media, 1);
        let media = store.media.lock().unwrap();
        let analysis = media[0].audio_analysis.as_ref().unwrap();
        assert!(analysis
            .audio_segments
            .iter()
            .all(|segment| segment.speaker_id.as_deref() == Some("voice-0001")));
        assert_eq!(analysis.recognized_voices.len(), 1);
        assert_eq!(analysis.recognized_voices[0].segment_count, 3);
        assert_eq!(analysis.recognized_voices[0].total_seconds, 4.5);
        assert!((analysis.recognized_voices[0].confidence - 0.8).abs() < 0.001);
    }

    struct TestStore {
        media: Mutex<Vec<ImagePayload>>,
        faces: Mutex<Vec<FacePointPayload>>,
    }

    impl TestStore {
        fn new(media: Vec<ImagePayload>, faces: Vec<FacePointPayload>) -> Self {
            Self {
                media: Mutex::new(media),
                faces: Mutex::new(faces),
            }
        }
    }

    #[async_trait]
    impl MediaVectorStore for TestStore {
        async fn ensure_collection(&self) -> Result<(), String> {
            Ok(())
        }

        async fn upsert_media(
            &self,
            _payload: &ImagePayload,
            _vector: Vec<f32>,
        ) -> Result<(), String> {
            Ok(())
        }

        async fn upsert_face(
            &self,
            _payload: &FacePointPayload,
            _vector: Vec<f32>,
        ) -> Result<(), String> {
            Ok(())
        }

        async fn set_media_payload(&self, payload: &ImagePayload) -> Result<(), String> {
            let mut media = self.media.lock().unwrap();
            let index = media.iter().position(|item| item.id == payload.id).unwrap();
            media[index] = payload.clone();
            Ok(())
        }

        async fn set_face_payload(&self, payload: &FacePointPayload) -> Result<(), String> {
            let mut faces = self.faces.lock().unwrap();
            let index = faces
                .iter()
                .position(|item| item.face_id == payload.face_id)
                .unwrap();
            faces[index] = payload.clone();
            Ok(())
        }

        async fn delete_points(&self, _ids: &[String]) -> Result<(), String> {
            Ok(())
        }

        async fn search_visual(
            &self,
            _vector: Vec<f32>,
            _limit: u32,
        ) -> Result<Vec<ScoredPoint>, String> {
            Ok(Vec::new())
        }

        async fn search_visual_filtered(
            &self,
            _vector: Vec<f32>,
            _limit: u32,
            _filter: Option<MediaSearchFilter>,
        ) -> Result<Vec<ScoredPoint>, String> {
            Ok(Vec::new())
        }

        async fn search_faces(
            &self,
            _vector: Vec<f32>,
            _limit: u32,
        ) -> Result<Vec<ScoredPoint>, String> {
            Ok(Vec::new())
        }

        async fn scroll_media_points(&self) -> Result<Vec<StoredPoint>, String> {
            Ok(self
                .media
                .lock()
                .unwrap()
                .iter()
                .map(|payload| StoredPoint {
                    id: payload.id.clone(),
                    payload: Some(to_value(payload).unwrap()),
                })
                .collect())
        }

        async fn scroll_face_points(&self) -> Result<Vec<StoredPoint>, String> {
            Ok(self
                .faces
                .lock()
                .unwrap()
                .iter()
                .map(|payload| StoredPoint {
                    id: payload.face_id.clone(),
                    payload: Some(to_value(payload).unwrap()),
                })
                .collect())
        }

        async fn scroll_media_points_by_filter(
            &self,
            _id: Option<&str>,
            _source_uri: Option<&str>,
            _source_item_uri: Option<&str>,
        ) -> Result<Vec<StoredPoint>, String> {
            Ok(Vec::new())
        }

        async fn scroll_face_points_by_media_ids(
            &self,
            _media_ids: &[String],
        ) -> Result<Vec<StoredPoint>, String> {
            Ok(Vec::new())
        }
    }

    fn person_media_payload() -> ImagePayload {
        let mut payload = base_media_payload("media-1");
        payload.faces = vec![
            test_face_detection("face-a", "person-a", Some("Ada"), 0.9),
            test_face_detection("face-b", "person-b", Some("Grace"), 0.7),
        ];
        payload.people = summarize_people(&payload.faces);
        payload
    }

    fn speaker_media_payload() -> ImagePayload {
        let mut payload = base_media_payload("audio-1");
        payload.media_kind = "audio".to_string();
        payload.audio_analysis = Some(AudioAnalysis {
            speech_detected: true,
            speech_ratio: 1.0,
            speech_segments: Vec::new(),
            audio_segments: vec![
                AudioSegmentGuess {
                    segment_index: 0,
                    kind: "speech".to_string(),
                    start_seconds: 0.0,
                    end_seconds: 1.0,
                    confidence: 0.8,
                    speaker_id: Some("voice-0001".to_string()),
                    speaker_label: Some("Voice 1".to_string()),
                },
                AudioSegmentGuess {
                    segment_index: 1,
                    kind: "speech".to_string(),
                    start_seconds: 1.0,
                    end_seconds: 3.5,
                    confidence: 0.7,
                    speaker_id: Some("voice-0002".to_string()),
                    speaker_label: Some("Voice 2".to_string()),
                },
            ],
            recognized_voices: vec![
                AudioRecognizedVoice {
                    id: "voice-0001".to_string(),
                    label: "Voice 1".to_string(),
                    segment_count: 1,
                    total_seconds: 1.0,
                    confidence: 0.9,
                },
                AudioRecognizedVoice {
                    id: "voice-0002".to_string(),
                    label: "Voice 2".to_string(),
                    segment_count: 2,
                    total_seconds: 3.5,
                    confidence: 0.75,
                },
            ],
            transcript_text: String::new(),
            transcript_language: None,
            transcript_segments: Vec::new(),
            tempo_bpm: None,
            tempo_confidence: 0.0,
            tempo_onset_count: 0,
        });
        payload
    }

    fn base_media_payload(id: &str) -> ImagePayload {
        ImagePayload {
            id: id.to_string(),
            path: format!("/images/{id}.jpg"),
            relative_path: format!("{id}.jpg"),
            filename: format!("{id}.jpg"),
            width: 100,
            height: 100,
            size_bytes: 1000,
            modified_at: 1.0,
            phash: "0000000000000000".to_string(),
            thumbnail_url: None,
            animated_thumbnail_url: None,
            media_kind: "static_image".to_string(),
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

    fn test_face_detection(
        face_id: &str,
        person_id: &str,
        label: Option<&str>,
        confidence: f32,
    ) -> FaceDetectionPayload {
        FaceDetectionPayload {
            face_id: face_id.to_string(),
            media_id: "media-1".to_string(),
            frame_index: 0,
            bbox: FaceBoxPayload::default(),
            confidence,
            person_id: Some(person_id.to_string()),
            person_label: label.map(ToOwned::to_owned),
        }
    }

    fn test_face_point(face_id: &str, person_id: &str, label: Option<&str>) -> FacePointPayload {
        FacePointPayload {
            face_id: face_id.to_string(),
            media_id: "media-1".to_string(),
            frame_index: 0,
            bbox: FaceBoxPayload::default(),
            confidence: 0.9,
            person_id: person_id.to_string(),
            person_label: label.map(ToOwned::to_owned),
            source_uri: None,
            source_item_uri: None,
        }
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("{name}-{}", uuid::Uuid::new_v4()))
    }
}
