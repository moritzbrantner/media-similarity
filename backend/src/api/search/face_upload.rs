use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::extract::{Multipart, Query, State};
use axum::Json;
use bytes::Bytes;
use image_analysis_detection::FaceDetection as SharedFaceDetection;

use crate::api::{ApiError, AppState};
use crate::domain::models::{
    FaceDetectionPayload, FaceMediaMatch, FacePersonMatch, FacePointPayload, FaceSearchModelStatus,
    FaceSearchQueryPayload, FaceSearchResponse, ImagePayload, SearchResult,
};
use crate::storage::ScoredPoint;
use crate::workers::media::faces::{FaceBox, FaceDetector, FaceEmbedder};
use crate::workers::media::image_io::load_media_bytes;
use crate::workers::media::models::{model_status, ModelRole};
use crate::workers::search::SearchFilters;

use super::query::SearchQuery;

pub async fn search_face_upload(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
    mut multipart: Multipart,
) -> Result<Json<FaceSearchResponse>, ApiError> {
    let filters = query.search_filters()?;
    let settings = state.indexing_settings();
    let (raw, filename) = read_face_upload(&mut multipart, &settings).await?;
    let model_status = face_model_status(&settings);
    if model_status.degraded {
        return Err(ApiError::service_unavailable(
            model_status.detail.clone().unwrap_or_else(|| {
                "Face detection and face embedding models must be downloaded before face search"
                    .to_string()
            }),
        ));
    }

    let media = load_media_bytes(&raw, &settings).map_err(|_| {
        ApiError::bad_request(format!(
            "Could not decode face query{}",
            filename
                .as_deref()
                .map(|name| format!(" `{name}`"))
                .unwrap_or_default()
        ))
    })?;
    let detector = FaceDetector::new(&settings);
    let embedder = FaceEmbedder::new(&settings);
    let candidates = detect_query_faces(&detector, &media)?;
    let Some(selected) = select_query_face(&candidates) else {
        return Ok(Json(FaceSearchResponse {
            query: FaceSearchQueryPayload {
                detected_faces: Vec::new(),
                selected_face: None,
                model_status,
            },
            people: Vec::new(),
            results: Vec::new(),
        }));
    };
    let frame = media
        .sampled_frames
        .get(selected.frame_index)
        .ok_or_else(|| ApiError::internal("selected face frame is unavailable"))?;
    let query_vector = embedder
        .embed_face(&frame.image, &selected.detection)
        .map_err(ApiError::service_unavailable)?;
    let limit = query.limit.unwrap_or(settings.default_search_limit);
    let face_matches = state
        .store
        .search_faces(query_vector, limit.saturating_mul(20).max(limit).min(500))
        .await
        .map_err(ApiError::internal)?;

    let detected_faces = candidates
        .iter()
        .map(|candidate| candidate.payload.clone())
        .collect::<Vec<_>>();
    let selected_face = selected.payload.clone();
    let (people, results) =
        aggregate_face_matches(face_matches, state.store.as_ref(), &filters, limit as usize)
            .await?;

    Ok(Json(FaceSearchResponse {
        query: FaceSearchQueryPayload {
            detected_faces,
            selected_face: Some(selected_face),
            model_status,
        },
        people,
        results,
    }))
}

#[derive(Clone)]
struct QueryFaceCandidate {
    frame_index: usize,
    detection: SharedFaceDetection,
    payload: FaceDetectionPayload,
}

async fn read_face_upload(
    multipart: &mut Multipart,
    settings: &crate::config::Settings,
) -> Result<(Bytes, Option<String>), ApiError> {
    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        if field.name() != Some("file") {
            continue;
        }
        let content_type = field.content_type().unwrap_or_default().to_string();
        let filename = field.file_name().map(ToOwned::to_owned);
        let filename_extension = filename
            .as_deref()
            .and_then(|name| std::path::Path::new(name).extension())
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{}", extension.to_ascii_lowercase()));
        let is_image = content_type.starts_with("image/")
            || filename_extension
                .as_ref()
                .map(|extension| settings.image_extensions.contains(extension))
                .unwrap_or(false);
        if !is_image {
            return Err(ApiError::bad_request("Face search upload must be an image"));
        }
        return field
            .bytes()
            .await
            .map(|bytes| (bytes, filename))
            .map_err(multipart_error);
    }
    Err(ApiError::bad_request(
        "Face search requires an image upload",
    ))
}

fn face_model_status(settings: &crate::config::Settings) -> FaceSearchModelStatus {
    let detection = model_status(ModelRole::FaceDetection, settings);
    let embedding = model_status(ModelRole::FaceEmbedding, settings);
    let degraded = !detection.active || !embedding.active;
    let face_detection_active = detection.active;
    let face_embedding_active = embedding.active;
    let detail = degraded.then(|| {
        [detection, embedding]
            .into_iter()
            .filter(|status| !status.active)
            .map(|status| {
                status.detail.unwrap_or_else(|| {
                    format!(
                        "{} model `{}` is not active",
                        status.label, status.configured
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("; ")
    });
    FaceSearchModelStatus {
        face_detection_active,
        face_embedding_active,
        degraded,
        detail,
    }
}

fn detect_query_faces(
    detector: &FaceDetector,
    media: &crate::workers::media::media::DecodedMedia,
) -> Result<Vec<QueryFaceCandidate>, ApiError> {
    let mut candidates = Vec::new();
    for (frame_index, frame) in media.sampled_frames.iter().enumerate() {
        let detections = detector
            .detect(&frame.image)
            .map_err(ApiError::service_unavailable)?;
        for (face_index, detection) in detections.into_iter().enumerate() {
            candidates.push(QueryFaceCandidate {
                frame_index,
                payload: FaceDetectionPayload {
                    face_id: format!("query#face={frame_index}-{face_index}"),
                    media_id: "query".to_string(),
                    frame_index,
                    bbox: FaceBox::from_shared(&detection.bbox).into(),
                    confidence: detection.confidence,
                    person_id: None,
                    person_label: None,
                },
                detection,
            });
        }
    }
    Ok(candidates)
}

fn select_query_face(candidates: &[QueryFaceCandidate]) -> Option<&QueryFaceCandidate> {
    candidates.iter().max_by(|left, right| {
        face_selection_score(left)
            .total_cmp(&face_selection_score(right))
            .then_with(|| left.payload.face_id.cmp(&right.payload.face_id).reverse())
    })
}

fn face_selection_score(candidate: &QueryFaceCandidate) -> f32 {
    candidate.payload.bbox.width * candidate.payload.bbox.height * candidate.payload.confidence
}

async fn aggregate_face_matches(
    matches: Vec<ScoredPoint>,
    store: &dyn crate::storage::MediaVectorStore,
    filters: &SearchFilters,
    limit: usize,
) -> Result<(Vec<FacePersonMatch>, Vec<FaceMediaMatch>), ApiError> {
    let mut face_matches = Vec::new();
    let mut media_ids = BTreeSet::new();
    for point in matches {
        let Some(payload) = point.payload else {
            continue;
        };
        let face: FacePointPayload = serde_json::from_value(payload)
            .map_err(|error| ApiError::internal(error.to_string()))?;
        media_ids.insert(face.media_id.clone());
        face_matches.push((face, point.score));
    }

    let media_points = store
        .scroll_media_points_filtered(Some(filters.qdrant_filter()))
        .await
        .map_err(ApiError::internal)?;
    let media_by_id = media_points
        .into_iter()
        .filter(|point| media_ids.contains(&point.id))
        .filter_map(|point| {
            let payload = point.payload?;
            serde_json::from_value::<ImagePayload>(payload).ok()
        })
        .filter(|payload| filters.matches_without_hash(payload))
        .map(|payload| (payload.id.clone(), payload))
        .collect::<BTreeMap<_, _>>();

    let mut people = BTreeMap::<String, PersonAccumulator>::new();
    let mut media = BTreeMap::<String, MediaAccumulator>::new();
    for (face, score) in face_matches {
        if !media_by_id.contains_key(&face.media_id) {
            continue;
        }
        let person = people.entry(face.person_id.clone()).or_default();
        person.label = person.label.clone().or(face.person_label.clone());
        person.score = person.score.max(score);
        person.face_ids.insert(face.face_id.clone());
        person.media_ids.insert(face.media_id.clone());

        let media_entry = media.entry(face.media_id.clone()).or_default();
        media_entry.person_id = face.person_id;
        media_entry.score = media_entry.score.max(score);
        media_entry.face_ids.insert(face.face_id);
    }

    let mut people = people
        .into_iter()
        .map(|(person_id, person)| FacePersonMatch {
            person_id,
            label: person.label,
            score: person.score,
            face_count: person.face_ids.len() as u32,
            media_count: person.media_ids.len() as u32,
            matched_face_ids: person.face_ids.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    people.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.person_id.cmp(&right.person_id))
    });
    people.truncate(limit);

    let mut results = media
        .into_iter()
        .filter_map(|(media_id, match_data)| {
            let image = media_by_id.get(&media_id)?.clone();
            Some(FaceMediaMatch {
                result: SearchResult {
                    image,
                    vector_score: match_data.score,
                    relevance_score: Some(match_data.score),
                    hash_distance: None,
                    ocr_score: None,
                    near_duplicate: false,
                    query_scene_index: None,
                },
                face_score: match_data.score,
                matched_person_id: match_data.person_id,
                matched_face_ids: match_data.face_ids.into_iter().collect(),
            })
        })
        .collect::<Vec<_>>();
    results.sort_by(|left, right| {
        right
            .face_score
            .total_cmp(&left.face_score)
            .then_with(|| left.result.image.filename.cmp(&right.result.image.filename))
    });
    results.truncate(limit);

    Ok((people, results))
}

#[derive(Default)]
struct PersonAccumulator {
    label: Option<String>,
    score: f32,
    face_ids: BTreeSet<String>,
    media_ids: BTreeSet<String>,
}

#[derive(Default)]
struct MediaAccumulator {
    person_id: String,
    score: f32,
    face_ids: BTreeSet<String>,
}

fn multipart_error(error: axum::extract::multipart::MultipartError) -> ApiError {
    match error.status() {
        axum::http::StatusCode::PAYLOAD_TOO_LARGE => ApiError::payload_too_large(error.body_text()),
        _ => ApiError::bad_request(error.body_text()),
    }
}

#[cfg(test)]
mod tests {
    use image_analysis_detection::{FaceBox as SharedFaceBox, FaceDetection};

    use super::{face_selection_score, select_query_face, QueryFaceCandidate};
    use crate::domain::models::{FaceBoxPayload, FaceDetectionPayload};

    #[test]
    fn face_query_selects_largest_confident_face() {
        let small = candidate("small", 0.9, 0.1, 0.1);
        let large = candidate("large", 0.7, 0.5, 0.5);

        let candidates = [small, large];
        let selected = select_query_face(&candidates).expect("selected face");

        assert_eq!(selected.payload.face_id, "large");
        assert!(face_selection_score(selected) > 0.0);
    }

    fn candidate(id: &str, confidence: f32, width: f32, height: f32) -> QueryFaceCandidate {
        QueryFaceCandidate {
            frame_index: 0,
            detection: FaceDetection::new(
                SharedFaceBox::new(0.0, 0.0, width, height).unwrap(),
                confidence,
            )
            .unwrap(),
            payload: FaceDetectionPayload {
                face_id: id.to_string(),
                media_id: "query".to_string(),
                frame_index: 0,
                bbox: FaceBoxPayload {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height,
                },
                confidence,
                person_id: None,
                person_label: None,
            },
        }
    }
}
