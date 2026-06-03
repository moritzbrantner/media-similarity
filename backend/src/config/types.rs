use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub source_image_dir: PathBuf,
    pub qdrant_url: String,
    pub qdrant_collection: String,
    pub qdrant_request_timeout_ms: u64,
    pub qdrant_connect_timeout_ms: u64,
    pub qdrant_retry_attempts: u32,
    pub qdrant_retry_backoff_ms: u64,
    pub clip_model_name: String,
    pub visual_embedding_enabled: bool,
    pub visual_embedding_backend: String,
    pub visual_embedding_model_path: PathBuf,
    pub visual_embedding_preprocessor_path: PathBuf,
    pub visual_embedding_vector_size: usize,
    pub visual_embedding_batch_size: usize,
    pub thumbnail_dir: PathBuf,
    pub upload_dir: PathBuf,
    pub indexing_ledger_file: PathBuf,
    pub processing_workflows_file: PathBuf,
    #[serde(default)]
    pub processing_workflows_hash: Option<String>,
    pub voice_registry_path: PathBuf,
    pub smart_albums_file: PathBuf,
    pub model_bundle_dir: PathBuf,
    pub model_hf_cache_dir: Option<PathBuf>,
    pub model_hf_token: Option<String>,
    pub image_extensions: BTreeSet<String>,
    pub audio_extensions: BTreeSet<String>,
    pub pdf_extensions: BTreeSet<String>,
    pub pdf_render_dpi: u32,
    pub pdf_max_pages: u32,
    pub pdf_summary_pages: usize,
    pub audio_transcription_enabled: bool,
    pub audio_transcription_model: String,
    pub audio_transcription_language: Option<String>,
    pub audio_transcription_threads: Option<usize>,
    pub audio_transcription_auto_download: bool,
    pub audio_transcription_cache_dir: Option<PathBuf>,
    pub media_sources_file: PathBuf,
    pub media_sources_seed_file: Option<PathBuf>,
    pub image_sources: Vec<String>,
    pub startup_indexing_enabled: bool,
    pub source_watching_enabled: bool,
    pub source_watching_debounce_ms: u64,
    pub minio_endpoint: Option<String>,
    pub minio_access_key: Option<String>,
    pub minio_secret_key: Option<String>,
    pub minio_secure: bool,
    pub s3_endpoint: Option<String>,
    pub s3_access_key_id: Option<String>,
    pub s3_secret_access_key: Option<String>,
    pub s3_region: String,
    pub s3_allow_http: bool,
    pub video_frame_stride: u32,
    pub video_max_frames: Option<u32>,
    pub camera_frame_stride: u32,
    pub camera_max_frames: u32,
    pub default_search_limit: u32,
    pub duplicate_hash_distance: u32,
    pub max_upload_mb: u32,
    pub vector_size: usize,
    pub gif_sample_frames: usize,
    pub gif_max_decode_frames: usize,
    pub gif_preview_frames: usize,
    pub gif_default_frame_delay_ms: u32,
    pub gif_motion_weight: f32,
    pub face_analysis_enabled: bool,
    pub face_detection_model_path: PathBuf,
    pub face_embedding_model_path: PathBuf,
    pub face_embedding_vector_size: usize,
    pub face_detection_min_confidence: f32,
    pub face_cluster_threshold: f32,
    pub face_min_cluster_images: u32,
    pub face_max_frames_per_media: usize,
    pub ocr_enabled: bool,
    pub ocr_command: String,
    pub ocr_language: Option<String>,
    pub ocr_max_frames: usize,
    pub bind_addr: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerSettings {
    pub bind_addr: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StorageSettings {
    pub qdrant_url: String,
    pub qdrant_collection: String,
    pub qdrant_request_timeout_ms: u64,
    pub qdrant_connect_timeout_ms: u64,
    pub qdrant_retry_attempts: u32,
    pub qdrant_retry_backoff_ms: u64,
    pub thumbnail_dir: PathBuf,
    pub upload_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceSettings {
    pub source_image_dir: PathBuf,
    pub media_sources_file: PathBuf,
    pub media_sources_seed_file: Option<PathBuf>,
    pub image_sources: Vec<String>,
    pub image_extensions: BTreeSet<String>,
    pub audio_extensions: BTreeSet<String>,
    pub pdf_extensions: BTreeSet<String>,
    pub minio_endpoint: Option<String>,
    pub minio_access_key: Option<String>,
    pub minio_secret_key: Option<String>,
    pub minio_secure: bool,
    pub s3_endpoint: Option<String>,
    pub s3_access_key_id: Option<String>,
    pub s3_secret_access_key: Option<String>,
    pub s3_region: String,
    pub s3_allow_http: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaDecodeSettings {
    pub gif_sample_frames: usize,
    pub gif_max_decode_frames: usize,
    pub gif_preview_frames: usize,
    pub gif_default_frame_delay_ms: u32,
    pub gif_motion_weight: f32,
    pub video_frame_stride: u32,
    pub video_max_frames: Option<u32>,
    pub camera_frame_stride: u32,
    pub camera_max_frames: u32,
    pub pdf_render_dpi: u32,
    pub pdf_max_pages: u32,
    pub pdf_summary_pages: usize,
    pub max_upload_mb: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VisualModelSettings {
    pub clip_model_name: String,
    pub visual_embedding_enabled: bool,
    pub visual_embedding_backend: String,
    pub visual_embedding_model_path: PathBuf,
    pub visual_embedding_preprocessor_path: PathBuf,
    pub visual_embedding_vector_size: usize,
    pub visual_embedding_batch_size: usize,
    pub vector_size: usize,
    pub model_bundle_dir: PathBuf,
    pub model_hf_cache_dir: Option<PathBuf>,
    pub model_hf_token: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaceSettings {
    pub face_analysis_enabled: bool,
    pub face_detection_model_path: PathBuf,
    pub face_embedding_model_path: PathBuf,
    pub face_embedding_vector_size: usize,
    pub face_detection_min_confidence: f32,
    pub face_cluster_threshold: f32,
    pub face_min_cluster_images: u32,
    pub face_max_frames_per_media: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OcrSettings {
    pub ocr_enabled: bool,
    pub ocr_command: String,
    pub ocr_language: Option<String>,
    pub ocr_max_frames: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioTranscriptionSettings {
    pub voice_registry_path: PathBuf,
    pub audio_transcription_enabled: bool,
    pub audio_transcription_model: String,
    pub audio_transcription_language: Option<String>,
    pub audio_transcription_threads: Option<usize>,
    pub audio_transcription_auto_download: bool,
    pub audio_transcription_cache_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchSettings {
    pub default_search_limit: u32,
    pub duplicate_hash_distance: u32,
}
