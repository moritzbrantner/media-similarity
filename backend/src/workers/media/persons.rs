use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use uuid::Uuid;

use crate::config::Settings;
use crate::domain::models::{FaceDetectionPayload, FacePointPayload, PersonSummary};
use crate::storage::{MediaVectorStore, ScoredPoint};

const PERSON_NAMESPACE: Uuid = Uuid::from_u128(0x2f26cf00_2f65_4df1_8a46_9b0d5fc7e4d3);

#[derive(Clone, Debug, PartialEq)]
pub struct PersonAssignment {
    pub person_id: String,
    pub person_label: Option<String>,
    pub confidence: f32,
}

pub async fn assign_person(
    store: &dyn MediaVectorStore,
    settings: &Settings,
    face_id: &str,
    embedding: Vec<f32>,
) -> PersonAssignment {
    let matches = store
        .search_faces(embedding, 20)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "face vector search failed; creating a new person cluster");
            Vec::new()
        });
    assign_person_from_matches(settings.face_cluster_threshold, face_id, &matches)
}

pub fn assign_person_from_matches(
    threshold: f32,
    face_id: &str,
    matches: &[ScoredPoint],
) -> PersonAssignment {
    let mut counts = BTreeMap::<String, (u32, Option<String>, f32)>::new();
    for point in matches {
        let distance = 1.0 - point.score;
        if distance > threshold {
            continue;
        }
        let Some(payload) = &point.payload else {
            continue;
        };
        let Some(person_id) = payload.get("person_id").and_then(Value::as_str) else {
            continue;
        };
        let person_label = payload
            .get("person_label")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let entry =
            counts
                .entry(person_id.to_string())
                .or_insert((0, person_label.clone(), point.score));
        entry.0 += 1;
        if entry.1.is_none() {
            entry.1 = person_label;
        }
        entry.2 = entry.2.max(point.score);
    }

    if let Some((person_id, (count, person_label, score))) =
        counts.into_iter().max_by(|left, right| {
            left.1
                 .0
                .cmp(&right.1 .0)
                .then_with(|| left.1 .2.total_cmp(&right.1 .2))
        })
    {
        return PersonAssignment {
            person_id,
            person_label,
            confidence: (score * count as f32).min(1.0),
        };
    }

    PersonAssignment {
        person_id: format!(
            "person-{}",
            Uuid::new_v5(&PERSON_NAMESPACE, face_id.as_bytes())
        ),
        person_label: None,
        confidence: 1.0,
    }
}

pub fn summarize_people(faces: &[FaceDetectionPayload]) -> Vec<PersonSummary> {
    let mut summaries = BTreeMap::<String, PersonAccumulator>::new();
    for face in faces {
        let Some(person_id) = &face.person_id else {
            continue;
        };
        let entry = summaries.entry(person_id.clone()).or_default();
        entry.label = entry.label.clone().or_else(|| face.person_label.clone());
        entry.face_count += 1;
        entry.media_ids.insert(face.media_id.clone());
        entry.confidence = entry.confidence.max(face.confidence);
    }

    summaries
        .into_iter()
        .map(|(person_id, summary)| PersonSummary {
            person_id,
            label: summary.label,
            face_count: summary.face_count,
            media_count: summary.media_ids.len() as u32,
            confidence: summary.confidence,
        })
        .collect()
}

pub fn face_point_payload(
    face: &FaceDetectionPayload,
    person: &PersonAssignment,
    source_uri: Option<String>,
    source_item_uri: Option<String>,
) -> FacePointPayload {
    FacePointPayload {
        face_id: face.face_id.clone(),
        media_id: face.media_id.clone(),
        frame_index: face.frame_index,
        bbox: face.bbox.clone(),
        confidence: face.confidence,
        person_id: person.person_id.clone(),
        person_label: person.person_label.clone(),
        source_uri,
        source_item_uri,
    }
}

#[derive(Default)]
struct PersonAccumulator {
    label: Option<String>,
    face_count: u32,
    media_ids: BTreeSet<String>,
    confidence: f32,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::assign_person_from_matches;
    use crate::storage::ScoredPoint;

    #[test]
    fn face_threshold_assignment_reuses_close_person() {
        let matches = vec![ScoredPoint {
            payload: Some(json!({ "person_id": "person-a", "person_label": "Ada" })),
            score: 0.9,
        }];

        let assignment = assign_person_from_matches(0.38, "face-1", &matches);

        assert_eq!(assignment.person_id, "person-a");
        assert_eq!(assignment.person_label.as_deref(), Some("Ada"));
    }

    #[test]
    fn face_threshold_assignment_creates_person_without_close_match() {
        let matches = vec![ScoredPoint {
            payload: Some(json!({ "person_id": "person-a" })),
            score: 0.1,
        }];

        let assignment = assign_person_from_matches(0.38, "face-1", &matches);

        assert!(assignment.person_id.starts_with("person-"));
        assert_ne!(assignment.person_id, "person-a");
    }
}
