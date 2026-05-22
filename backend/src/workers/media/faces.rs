use image::RgbImage;
use image_analysis_core::{ImagePixelFormat, ImageView};
use image_analysis_models::{
    FaceDetection as SharedFaceDetection, FaceDetectorBackend, FaceEmbedderBackend,
};
use image_analysis_onnx::{
    NativeOnnxRunner, OnnxFaceDetectionOptions, OnnxFaceDetector, OnnxFaceEmbedder,
};

use crate::config::Settings;
use crate::domain::models::{FaceBoxPayload, FaceDetectionPayload, PersonSummary};
use crate::storage::qdrant::QdrantImageStore;
use crate::workers::media::media::DecodedMedia;
use crate::workers::media::models::{load_role_bundle, ModelRole};
use crate::workers::media::persons::{assign_person, face_point_payload, summarize_people};

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
    source_uri: Option<String>,
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
        let point = face_point_payload(
            &payload,
            &assignment,
            source_uri.clone(),
            source_item_uri.clone(),
        );
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
            for (face_index, detection) in
                self.detector.detect(&frame.image)?.into_iter().enumerate()
            {
                let bbox = FaceBox::from_shared(&detection.bbox);
                let embedding = self.embedder.embed_face(&frame.image, &detection)?;
                let candidate = DetectedFace {
                    face_id: format!("{media_id}#face={frame_index}-{face_index}"),
                    frame_index,
                    bbox,
                    confidence: detection.confidence,
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
    settings: Settings,
    min_confidence: f32,
    max_frames: usize,
    cluster_threshold: f32,
    runner:
        std::sync::OnceLock<std::sync::Mutex<Result<OnnxFaceDetector<NativeOnnxRunner>, String>>>,
}

impl FaceDetector {
    pub fn new(settings: &Settings) -> Self {
        Self {
            model_path: settings.face_detection_model_path.clone(),
            settings: settings.clone(),
            min_confidence: settings.face_detection_min_confidence,
            max_frames: settings.face_max_frames_per_media,
            cluster_threshold: settings.face_cluster_threshold,
            runner: std::sync::OnceLock::new(),
        }
    }

    pub fn detect(&self, image: &RgbImage) -> Result<Vec<SharedFaceDetection>, String> {
        let bundle = load_role_bundle(ModelRole::FaceDetection, &self.settings).ok();
        if bundle.is_none() && !self.model_path.is_file() {
            return Err(format!(
                "face detection model is not available at {}",
                self.model_path.display()
            ));
        }

        let model_path = self.model_path.clone();
        let min_confidence = self.min_confidence;
        let settings = self.settings.clone();
        let mut runner = self
            .runner
            .get_or_init(|| {
                let options = OnnxFaceDetectionOptions {
                    score_threshold: min_confidence,
                    ..OnnxFaceDetectionOptions::default()
                };
                let runner = load_role_bundle(ModelRole::FaceDetection, &settings)
                    .and_then(|bundle| {
                        OnnxFaceDetector::<NativeOnnxRunner>::from_bundle(bundle)
                            .map_err(|error| error.to_string())
                    })
                    .or_else(|bundle_error| {
                        if model_path.is_file() {
                            NativeOnnxRunner::new(model_path)
                                .and_then(|runner| OnnxFaceDetector::with_options(options, runner))
                                .map_err(|error| error.to_string())
                        } else {
                            Err(bundle_error)
                        }
                    });
                std::sync::Mutex::new(runner)
            })
            .lock()
            .map_err(|_| "face detection runner mutex was poisoned".to_string())?;
        let runner = runner.as_mut().map_err(|error| error.clone())?;
        let view = rgb_image_view(image)?;
        runner
            .detect_faces(&view)
            .map_err(|error| error.to_string())
    }
}

pub struct FaceEmbedder {
    model_path: std::path::PathBuf,
    settings: Settings,
    vector_size: usize,
    runner:
        std::sync::OnceLock<std::sync::Mutex<Result<OnnxFaceEmbedder<NativeOnnxRunner>, String>>>,
}

impl FaceEmbedder {
    pub fn new(settings: &Settings) -> Self {
        Self {
            model_path: settings.face_embedding_model_path.clone(),
            settings: settings.clone(),
            vector_size: settings.face_embedding_vector_size,
            runner: std::sync::OnceLock::new(),
        }
    }

    pub fn embed_face(
        &self,
        image: &RgbImage,
        detection: &SharedFaceDetection,
    ) -> Result<Vec<f32>, String> {
        let bundle = load_role_bundle(ModelRole::FaceEmbedding, &self.settings).ok();
        if bundle.is_none() && !self.model_path.is_file() {
            return Err(format!(
                "face embedding model is not available at {}",
                self.model_path.display()
            ));
        }

        let model_path = self.model_path.clone();
        let vector_size = self.vector_size;
        let settings = self.settings.clone();
        let mut runner = self
            .runner
            .get_or_init(|| {
                let runner = load_role_bundle(ModelRole::FaceEmbedding, &settings)
                    .and_then(|bundle| {
                        OnnxFaceEmbedder::<NativeOnnxRunner>::from_bundle(bundle)
                            .map_err(|error| error.to_string())
                    })
                    .or_else(|bundle_error| {
                        if model_path.is_file() {
                            OnnxFaceEmbedder::from_model_path(model_path, Some(vector_size))
                                .map_err(|error| error.to_string())
                        } else {
                            Err(bundle_error)
                        }
                    });
                std::sync::Mutex::new(runner)
            })
            .lock()
            .map_err(|_| "face embedding runner mutex was poisoned".to_string())?;
        let runner = runner.as_mut().map_err(|error| error.clone())?;
        let view = rgb_image_view(image)?;
        runner
            .embed_face(&view, Some(detection))
            .map(|embedding| embedding.vector)
            .map_err(|error| error.to_string())
    }
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

impl FaceBox {
    fn from_shared(value: &image_analysis_models::FaceBox) -> Self {
        Self {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }
}

fn rgb_image_view(image: &RgbImage) -> Result<ImageView<'_>, String> {
    ImageView::packed(
        image.width(),
        image.height(),
        ImagePixelFormat::Rgb24,
        image.as_raw(),
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::{FaceBox, FaceDetector};
    use crate::config::Settings;

    #[test]
    fn face_detector_reports_missing_model_without_panicking() {
        let settings = Settings {
            face_detection_model_path: std::env::temp_dir().join("missing-yunet-model.onnx"),
            ..Settings::default()
        };
        let detector = FaceDetector::new(&settings);
        let image = ImageBuffer::from_pixel(8, 8, Rgb([0, 0, 0]));

        let error = detector.detect(&image).unwrap_err();

        assert!(error.contains("face detection model is not available"));
    }

    #[test]
    fn shared_face_box_maps_to_service_payload_box() {
        let shared = image_analysis_models::FaceBox::new(0.1, 0.2, 0.3, 0.4).unwrap();

        let service = FaceBox::from_shared(&shared);

        assert_eq!(service.x, 0.1);
        assert_eq!(service.y, 0.2);
        assert_eq!(service.width, 0.3);
        assert_eq!(service.height, 0.4);
    }
}
