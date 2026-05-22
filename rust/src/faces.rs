use image::{imageops, RgbImage};

use crate::config::Settings;
use crate::media::DecodedMedia;
use crate::models::{FaceBoxPayload, FaceDetectionPayload, PersonSummary};
use crate::persons::{assign_person, face_point_payload, summarize_people};
use crate::qdrant::QdrantImageStore;
use crate::visual_embedding::{LegacyColorEmbedder, VisualEmbeddingBackend};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FaceAnalysis {
    pub faces: Vec<FaceDetectionPayload>,
    pub person_clusters: Vec<PersonSummary>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DetectedFace {
    pub face_id: String,
    pub frame_index: usize,
    pub bbox: FaceBox,
    pub confidence: f32,
    pub embedding: Vec<f32>,
    pub person_id: Option<String>,
    pub person_label: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FaceBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub async fn analyze_faces_for_media(
    settings: &Settings,
    store: &QdrantImageStore,
    media: &DecodedMedia,
    media_id: &str,
    source_item_uri: Option<String>,
) -> FaceAnalysis {
    if !settings.face_analysis_enabled || media.kind.as_str() == "audio" {
        return FaceAnalysis::default();
    }

    let detector = FaceDetector::new(settings);
    let embedder = FaceEmbedder::new(settings);
    let mut detected = match FaceAnalyzer::new(detector, embedder).analyze(media, media_id) {
        Ok(faces) => faces,
        Err(error) => {
            tracing::warn!(%error, %media_id, "face analysis skipped");
            return FaceAnalysis::default();
        }
    };

    let mut payload_faces = Vec::new();
    for face in &mut detected {
        let assignment =
            assign_person(store, settings, &face.face_id, face.embedding.clone()).await;
        face.person_id = Some(assignment.person_id.clone());
        face.person_label = assignment.person_label.clone();

        let payload = FaceDetectionPayload {
            face_id: face.face_id.clone(),
            media_id: media_id.to_string(),
            frame_index: face.frame_index,
            bbox: face.bbox.clone().into(),
            confidence: face.confidence,
            person_id: face.person_id.clone(),
            person_label: face.person_label.clone(),
        };
        let point = face_point_payload(&payload, &assignment, source_item_uri.clone());
        if let Err(error) = store.upsert_face(&point, face.embedding.clone()).await {
            tracing::warn!(%error, face_id = %face.face_id, "could not upsert face point");
        }
        payload_faces.push(payload);
    }

    let person_clusters = summarize_people(&payload_faces);
    FaceAnalysis {
        faces: payload_faces,
        person_clusters,
    }
}

pub struct FaceAnalyzer {
    detector: FaceDetector,
    embedder: FaceEmbedder,
}

impl FaceAnalyzer {
    pub fn new(detector: FaceDetector, embedder: FaceEmbedder) -> Self {
        Self { detector, embedder }
    }

    pub fn analyze(
        &self,
        media: &DecodedMedia,
        media_id: &str,
    ) -> Result<Vec<DetectedFace>, String> {
        let mut faces = Vec::new();
        for (frame_index, frame) in media
            .sampled_frames
            .iter()
            .take(self.detector.max_frames)
            .enumerate()
        {
            for (face_index, bbox) in self.detector.detect(&frame.image)?.into_iter().enumerate() {
                let embedding = self.embedder.embed_face(&frame.image, &bbox)?;
                let candidate = DetectedFace {
                    face_id: format!("{media_id}#face={frame_index}-{face_index}"),
                    frame_index,
                    bbox,
                    confidence: 1.0,
                    embedding,
                    person_id: None,
                    person_label: None,
                };
                if !is_duplicate_face(&candidate, &faces, self.detector.cluster_threshold) {
                    faces.push(candidate);
                }
            }
        }
        Ok(faces)
    }
}

pub struct FaceDetector {
    model_path: std::path::PathBuf,
    min_confidence: f32,
    max_frames: usize,
    cluster_threshold: f32,
}

impl FaceDetector {
    pub fn new(settings: &Settings) -> Self {
        Self {
            model_path: settings.face_detection_model_path.clone(),
            min_confidence: settings.face_detection_min_confidence,
            max_frames: settings.face_max_frames_per_media,
            cluster_threshold: settings.face_cluster_threshold,
        }
    }

    pub fn detect(&self, _image: &RgbImage) -> Result<Vec<FaceBox>, String> {
        if !self.model_path.is_file() {
            return Err(format!(
                "face detection model is not available at {}",
                self.model_path.display()
            ));
        }

        let _ = self.min_confidence;
        Ok(Vec::new())
    }
}

pub struct FaceEmbedder {
    model_path: std::path::PathBuf,
    fallback: LegacyColorEmbedder,
}

impl FaceEmbedder {
    pub fn new(settings: &Settings) -> Self {
        Self {
            model_path: settings.face_embedding_model_path.clone(),
            fallback: LegacyColorEmbedder::new(
                "face-legacy-fallback",
                settings.face_embedding_vector_size,
            ),
        }
    }

    pub fn embed_face(&self, image: &RgbImage, bbox: &FaceBox) -> Result<Vec<f32>, String> {
        if !self.model_path.is_file() {
            return Err(format!(
                "face embedding model is not available at {}",
                self.model_path.display()
            ));
        }

        let crop = crop_face(image, bbox);
        self.fallback.embed_image(&crop)
    }
}

fn crop_face(image: &RgbImage, bbox: &FaceBox) -> RgbImage {
    let width = image.width().max(1);
    let height = image.height().max(1);
    let x = (bbox.x.clamp(0.0, 1.0) * width as f32).floor() as u32;
    let y = (bbox.y.clamp(0.0, 1.0) * height as f32).floor() as u32;
    let crop_width = (bbox.width.clamp(0.0, 1.0) * width as f32).ceil() as u32;
    let crop_height = (bbox.height.clamp(0.0, 1.0) * height as f32).ceil() as u32;
    let crop_width = crop_width.max(1).min(width.saturating_sub(x).max(1));
    let crop_height = crop_height.max(1).min(height.saturating_sub(y).max(1));
    imageops::crop_imm(image, x, y, crop_width, crop_height).to_image()
}

fn is_duplicate_face(candidate: &DetectedFace, faces: &[DetectedFace], threshold: f32) -> bool {
    faces.iter().any(|existing| {
        let similarity = cosine_similarity(&candidate.embedding, &existing.embedding);
        1.0 - similarity <= threshold
    })
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (left, right) in left.iter().zip(right) {
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

impl From<FaceBox> for FaceBoxPayload {
    fn from(value: FaceBox) -> Self {
        Self {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }
}
