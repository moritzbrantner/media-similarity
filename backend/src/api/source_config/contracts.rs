use serde::{Deserialize, Serialize};

use crate::config::{parse_extensions, Settings};

#[derive(Debug, Serialize)]
pub struct SourceConfigResponse {
    pub media_sources_file: String,
    pub media_sources_seed_file: Option<String>,
    pub media_sources_writable: bool,
    pub default_source_dir: String,
    pub sources: Vec<SourceConfigSource>,
    pub supported_source_types: Vec<SupportedSourceType>,
    pub indexing: SourceIndexingConfig,
}

#[derive(Debug, Serialize)]
pub struct SourceConfigSource {
    pub spec: String,
    pub kind: String,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SupportedSourceType {
    pub kind: String,
    pub label: String,
    pub implemented: bool,
    pub example: String,
}

#[derive(Debug, Serialize)]
pub struct SourceIndexingConfig {
    pub collection: String,
    pub image_extensions: Vec<String>,
    pub audio_extensions: Vec<String>,
    pub pdf_extensions: Vec<String>,
    pub video_extensions: Vec<String>,
    pub visual_embedding_enabled: bool,
    pub visual_embedding_model: String,
    pub visual_embedding_vector_size: usize,
    pub face_analysis_enabled: bool,
    pub face_detection_min_confidence: f32,
    pub face_cluster_threshold: f32,
    pub face_min_cluster_images: u32,
    pub face_max_frames_per_media: usize,
    pub gif_sample_frames: usize,
    pub gif_max_decode_frames: usize,
    pub gif_preview_frames: usize,
    pub gif_default_frame_delay_ms: u32,
    pub gif_motion_weight: f32,
    pub video_frame_stride: u32,
    pub video_max_frames: Option<u32>,
    pub pdf_render_dpi: u32,
    pub pdf_max_pages: u32,
    pub pdf_summary_pages: usize,
    pub ocr_enabled: bool,
    pub ocr_max_frames: usize,
    pub audio_transcription_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSourceConfigRequest {
    pub sources: Option<Vec<String>>,
    pub indexing: Option<EditableIndexingConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EditableIndexingConfig {
    pub image_extensions: Vec<String>,
    pub audio_extensions: Vec<String>,
    pub pdf_extensions: Vec<String>,
    #[serde(default = "default_true")]
    pub visual_embedding_enabled: bool,
    pub face_analysis_enabled: bool,
    pub face_detection_min_confidence: f32,
    pub face_cluster_threshold: f32,
    pub face_min_cluster_images: u32,
    pub face_max_frames_per_media: usize,
    pub gif_sample_frames: usize,
    pub gif_max_decode_frames: usize,
    pub gif_preview_frames: usize,
    pub gif_default_frame_delay_ms: u32,
    pub gif_motion_weight: f32,
    pub video_frame_stride: u32,
    pub video_max_frames: Option<u32>,
    pub pdf_render_dpi: u32,
    pub pdf_max_pages: u32,
    pub pdf_summary_pages: usize,
    pub ocr_enabled: bool,
    pub ocr_max_frames: usize,
    pub audio_transcription_enabled: bool,
}

impl EditableIndexingConfig {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            image_extensions: settings.image_extensions.iter().cloned().collect(),
            audio_extensions: settings.audio_extensions.iter().cloned().collect(),
            pdf_extensions: settings.pdf_extensions.iter().cloned().collect(),
            visual_embedding_enabled: settings.visual_embedding_enabled,
            face_analysis_enabled: settings.face_analysis_enabled,
            face_detection_min_confidence: settings.face_detection_min_confidence,
            face_cluster_threshold: settings.face_cluster_threshold,
            face_min_cluster_images: settings.face_min_cluster_images,
            face_max_frames_per_media: settings.face_max_frames_per_media,
            gif_sample_frames: settings.gif_sample_frames,
            gif_max_decode_frames: settings.gif_max_decode_frames,
            gif_preview_frames: settings.gif_preview_frames,
            gif_default_frame_delay_ms: settings.gif_default_frame_delay_ms,
            gif_motion_weight: settings.gif_motion_weight,
            video_frame_stride: settings.video_frame_stride,
            video_max_frames: settings.video_max_frames,
            pdf_render_dpi: settings.pdf_render_dpi,
            pdf_max_pages: settings.pdf_max_pages,
            pdf_summary_pages: settings.pdf_summary_pages,
            ocr_enabled: settings.ocr_enabled,
            ocr_max_frames: settings.ocr_max_frames,
            audio_transcription_enabled: settings.audio_transcription_enabled,
        }
    }

    pub fn apply_to_settings(&self, settings: &mut Settings) {
        settings.image_extensions = parse_extensions(&self.image_extensions.join(","))
            .expect("validated indexing config contains image extensions");
        settings.audio_extensions = parse_extensions(&self.audio_extensions.join(","))
            .expect("validated indexing config contains audio extensions");
        settings.pdf_extensions = parse_extensions(&self.pdf_extensions.join(","))
            .expect("validated indexing config contains PDF extensions");
        settings.visual_embedding_enabled = self.visual_embedding_enabled;
        settings.face_analysis_enabled = self.face_analysis_enabled;
        settings.face_detection_min_confidence = self.face_detection_min_confidence;
        settings.face_cluster_threshold = self.face_cluster_threshold;
        settings.face_min_cluster_images = self.face_min_cluster_images;
        settings.face_max_frames_per_media = self.face_max_frames_per_media;
        settings.gif_sample_frames = self.gif_sample_frames;
        settings.gif_max_decode_frames = self.gif_max_decode_frames;
        settings.gif_preview_frames = self.gif_preview_frames;
        settings.gif_default_frame_delay_ms = self.gif_default_frame_delay_ms;
        settings.gif_motion_weight = self.gif_motion_weight;
        settings.video_frame_stride = self.video_frame_stride;
        settings.video_max_frames = self.video_max_frames;
        settings.pdf_render_dpi = self.pdf_render_dpi;
        settings.pdf_max_pages = self.pdf_max_pages;
        settings.pdf_summary_pages = self.pdf_summary_pages;
        settings.ocr_enabled = self.ocr_enabled;
        settings.ocr_max_frames = self.ocr_max_frames;
        settings.audio_transcription_enabled = self.audio_transcription_enabled;
    }
}

fn default_true() -> bool {
    true
}
