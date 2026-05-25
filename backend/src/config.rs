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
    pub voice_registry_path: PathBuf,
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

impl Settings {
    pub fn server_settings(&self) -> ServerSettings {
        ServerSettings {
            bind_addr: self.bind_addr.clone(),
        }
    }

    pub fn storage_settings(&self) -> StorageSettings {
        StorageSettings {
            qdrant_url: self.qdrant_url.clone(),
            qdrant_collection: self.qdrant_collection.clone(),
            qdrant_request_timeout_ms: self.qdrant_request_timeout_ms,
            qdrant_connect_timeout_ms: self.qdrant_connect_timeout_ms,
            qdrant_retry_attempts: self.qdrant_retry_attempts,
            qdrant_retry_backoff_ms: self.qdrant_retry_backoff_ms,
            thumbnail_dir: self.thumbnail_dir.clone(),
            upload_dir: self.upload_dir.clone(),
        }
    }

    pub fn source_settings(&self) -> SourceSettings {
        SourceSettings {
            source_image_dir: self.source_image_dir.clone(),
            media_sources_file: self.media_sources_file.clone(),
            media_sources_seed_file: self.media_sources_seed_file.clone(),
            image_sources: self.image_sources.clone(),
            image_extensions: self.image_extensions.clone(),
            audio_extensions: self.audio_extensions.clone(),
            pdf_extensions: self.pdf_extensions.clone(),
            minio_endpoint: self.minio_endpoint.clone(),
            minio_access_key: self.minio_access_key.clone(),
            minio_secret_key: self.minio_secret_key.clone(),
            minio_secure: self.minio_secure,
            s3_endpoint: self.s3_endpoint.clone(),
            s3_access_key_id: self.s3_access_key_id.clone(),
            s3_secret_access_key: self.s3_secret_access_key.clone(),
            s3_region: self.s3_region.clone(),
            s3_allow_http: self.s3_allow_http,
        }
    }

    pub fn media_decode_settings(&self) -> MediaDecodeSettings {
        MediaDecodeSettings {
            gif_sample_frames: self.gif_sample_frames,
            gif_max_decode_frames: self.gif_max_decode_frames,
            gif_preview_frames: self.gif_preview_frames,
            gif_default_frame_delay_ms: self.gif_default_frame_delay_ms,
            gif_motion_weight: self.gif_motion_weight,
            video_frame_stride: self.video_frame_stride,
            video_max_frames: self.video_max_frames,
            camera_frame_stride: self.camera_frame_stride,
            camera_max_frames: self.camera_max_frames,
            pdf_render_dpi: self.pdf_render_dpi,
            pdf_max_pages: self.pdf_max_pages,
            pdf_summary_pages: self.pdf_summary_pages,
            max_upload_mb: self.max_upload_mb,
        }
    }

    pub fn visual_model_settings(&self) -> VisualModelSettings {
        VisualModelSettings {
            clip_model_name: self.clip_model_name.clone(),
            visual_embedding_enabled: self.visual_embedding_enabled,
            visual_embedding_backend: self.visual_embedding_backend.clone(),
            visual_embedding_model_path: self.visual_embedding_model_path.clone(),
            visual_embedding_preprocessor_path: self.visual_embedding_preprocessor_path.clone(),
            visual_embedding_vector_size: self.visual_embedding_vector_size,
            visual_embedding_batch_size: self.visual_embedding_batch_size,
            vector_size: self.vector_size,
            model_bundle_dir: self.model_bundle_dir.clone(),
            model_hf_cache_dir: self.model_hf_cache_dir.clone(),
            model_hf_token: self.model_hf_token.clone(),
        }
    }

    pub fn face_settings(&self) -> FaceSettings {
        FaceSettings {
            face_analysis_enabled: self.face_analysis_enabled,
            face_detection_model_path: self.face_detection_model_path.clone(),
            face_embedding_model_path: self.face_embedding_model_path.clone(),
            face_embedding_vector_size: self.face_embedding_vector_size,
            face_detection_min_confidence: self.face_detection_min_confidence,
            face_cluster_threshold: self.face_cluster_threshold,
            face_min_cluster_images: self.face_min_cluster_images,
            face_max_frames_per_media: self.face_max_frames_per_media,
        }
    }

    pub fn ocr_settings(&self) -> OcrSettings {
        OcrSettings {
            ocr_enabled: self.ocr_enabled,
            ocr_command: self.ocr_command.clone(),
            ocr_language: self.ocr_language.clone(),
            ocr_max_frames: self.ocr_max_frames,
        }
    }

    pub fn audio_transcription_settings(&self) -> AudioTranscriptionSettings {
        AudioTranscriptionSettings {
            voice_registry_path: self.voice_registry_path.clone(),
            audio_transcription_enabled: self.audio_transcription_enabled,
            audio_transcription_model: self.audio_transcription_model.clone(),
            audio_transcription_language: self.audio_transcription_language.clone(),
            audio_transcription_threads: self.audio_transcription_threads,
            audio_transcription_auto_download: self.audio_transcription_auto_download,
            audio_transcription_cache_dir: self.audio_transcription_cache_dir.clone(),
        }
    }

    pub fn search_settings(&self) -> SearchSettings {
        SearchSettings {
            default_search_limit: self.default_search_limit,
            duplicate_hash_distance: self.duplicate_hash_distance,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source_image_dir: PathBuf::from("/images"),
            qdrant_url: "http://qdrant:6333".to_string(),
            qdrant_collection: "image_similarity".to_string(),
            qdrant_request_timeout_ms: 30_000,
            qdrant_connect_timeout_ms: 2_000,
            qdrant_retry_attempts: 2,
            qdrant_retry_backoff_ms: 100,
            clip_model_name: "sentence-transformers/clip-ViT-B-32".to_string(),
            visual_embedding_enabled: true,
            visual_embedding_backend: "onnx".to_string(),
            visual_embedding_model_path: PathBuf::from(
                "data/models/visual/clip-vit-base-patch32/model.onnx",
            ),
            visual_embedding_preprocessor_path: PathBuf::from(
                "data/models/visual/clip-vit-base-patch32/preprocessor_config.json",
            ),
            visual_embedding_vector_size: 512,
            visual_embedding_batch_size: 8,
            thumbnail_dir: PathBuf::from("data/thumbnails"),
            upload_dir: PathBuf::from("data/uploads"),
            voice_registry_path: PathBuf::from("data/recognized-voices.json"),
            model_bundle_dir: PathBuf::from("data/models/bundles"),
            model_hf_cache_dir: None,
            model_hf_token: None,
            image_extensions: parse_extensions(".jpg,.jpeg,.png,.webp,.bmp,.tif,.tiff,.gif")
                .expect("default extensions are valid"),
            audio_extensions: parse_extensions(".mp3,.wav,.flac,.m4a,.aac,.ogg,.opus")
                .expect("default audio extensions are valid"),
            pdf_extensions: parse_extensions(".pdf").expect("default PDF extensions are valid"),
            pdf_render_dpi: 144,
            pdf_max_pages: 100,
            pdf_summary_pages: 8,
            audio_transcription_enabled: false,
            audio_transcription_model: "base.en".to_string(),
            audio_transcription_language: Some("en".to_string()),
            audio_transcription_threads: None,
            audio_transcription_auto_download: false,
            audio_transcription_cache_dir: None,
            media_sources_file: PathBuf::from("config/media-sources.txt"),
            media_sources_seed_file: None,
            image_sources: Vec::new(),
            source_watching_enabled: true,
            source_watching_debounce_ms: 1500,
            minio_endpoint: None,
            minio_access_key: None,
            minio_secret_key: None,
            minio_secure: true,
            s3_endpoint: None,
            s3_access_key_id: None,
            s3_secret_access_key: None,
            s3_region: "us-east-1".to_string(),
            s3_allow_http: false,
            video_frame_stride: 30,
            video_max_frames: None,
            camera_frame_stride: 30,
            camera_max_frames: 100,
            default_search_limit: 12,
            duplicate_hash_distance: 8,
            max_upload_mb: 20,
            vector_size: 512,
            gif_sample_frames: 16,
            gif_max_decode_frames: 512,
            gif_preview_frames: 16,
            gif_default_frame_delay_ms: 100,
            gif_motion_weight: 0.2,
            face_analysis_enabled: true,
            face_detection_model_path: PathBuf::from("data/models/faces/detector/model.onnx"),
            face_embedding_model_path: PathBuf::from("data/models/faces/recognizer/model.onnx"),
            face_embedding_vector_size: 512,
            face_detection_min_confidence: 0.75,
            face_cluster_threshold: 0.38,
            face_min_cluster_images: 2,
            face_max_frames_per_media: 8,
            ocr_enabled: true,
            ocr_command: "tesseract".to_string(),
            ocr_language: Some("eng".to_string()),
            ocr_max_frames: 4,
            bind_addr: "0.0.0.0:8000".to_string(),
        }
    }
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();
        let defaults = Self::default();
        let media_sources_file = path_var("MEDIA_SOURCES_FILE", defaults.media_sources_file);
        let media_sources_seed_file = optional_string_var("MEDIA_SOURCES_SEED_FILE")
            .map(PathBuf::from)
            .or(defaults.media_sources_seed_file);
        let media_sources_file_is_set = optional_string_var("MEDIA_SOURCES_FILE").is_some();
        let image_sources = match optional_string_var("IMAGE_SOURCES") {
            Some(value) => parse_image_sources(&value)?,
            None => read_media_sources_files(
                &media_sources_file,
                media_sources_seed_file.as_deref(),
                media_sources_file_is_set,
            )?,
        };
        let qdrant_request_timeout_ms = bounded_u64_var(
            "QDRANT_REQUEST_TIMEOUT_MS",
            defaults.qdrant_request_timeout_ms,
            1_000,
            600_000,
        )?;
        let qdrant_connect_timeout_ms = bounded_u64_var(
            "QDRANT_CONNECT_TIMEOUT_MS",
            defaults.qdrant_connect_timeout_ms,
            100,
            60_000,
        )?;
        if qdrant_connect_timeout_ms > qdrant_request_timeout_ms {
            return Err(
                "QDRANT_CONNECT_TIMEOUT_MS must be less than or equal to QDRANT_REQUEST_TIMEOUT_MS"
                    .to_string(),
            );
        }
        Ok(Self {
            source_image_dir: path_var("SOURCE_IMAGE_DIR", defaults.source_image_dir),
            qdrant_url: string_var("QDRANT_URL", defaults.qdrant_url),
            qdrant_collection: string_var("QDRANT_COLLECTION", defaults.qdrant_collection),
            qdrant_request_timeout_ms,
            qdrant_connect_timeout_ms,
            qdrant_retry_attempts: bounded_u32_var(
                "QDRANT_RETRY_ATTEMPTS",
                defaults.qdrant_retry_attempts,
                0,
                5,
            )?,
            qdrant_retry_backoff_ms: bounded_u64_var(
                "QDRANT_RETRY_BACKOFF_MS",
                defaults.qdrant_retry_backoff_ms,
                10,
                10_000,
            )?,
            clip_model_name: string_var("CLIP_MODEL_NAME", defaults.clip_model_name),
            visual_embedding_enabled: bool_var(
                "VISUAL_EMBEDDING_ENABLED",
                defaults.visual_embedding_enabled,
            ),
            visual_embedding_backend: string_var(
                "VISUAL_EMBEDDING_BACKEND",
                defaults.visual_embedding_backend,
            ),
            visual_embedding_model_path: path_var(
                "VISUAL_EMBEDDING_MODEL_PATH",
                defaults.visual_embedding_model_path,
            ),
            visual_embedding_preprocessor_path: path_var(
                "VISUAL_EMBEDDING_PREPROCESSOR_PATH",
                defaults.visual_embedding_preprocessor_path,
            ),
            visual_embedding_vector_size: bounded_usize_var(
                "VISUAL_EMBEDDING_VECTOR_SIZE",
                defaults.visual_embedding_vector_size,
                1,
                usize::MAX,
            )?,
            visual_embedding_batch_size: bounded_usize_var(
                "VISUAL_EMBEDDING_BATCH_SIZE",
                defaults.visual_embedding_batch_size,
                1,
                usize::MAX,
            )?,
            thumbnail_dir: path_var("THUMBNAIL_DIR", defaults.thumbnail_dir),
            upload_dir: path_var("UPLOAD_DIR", defaults.upload_dir),
            voice_registry_path: path_var("VOICE_REGISTRY_PATH", defaults.voice_registry_path),
            model_bundle_dir: path_var("MODEL_BUNDLE_DIR", defaults.model_bundle_dir),
            model_hf_cache_dir: optional_string_var("MODEL_HF_CACHE_DIR")
                .map(PathBuf::from)
                .or(defaults.model_hf_cache_dir),
            model_hf_token: optional_string_var("MODEL_HF_TOKEN").or(defaults.model_hf_token),
            image_extensions: match env::var("IMAGE_EXTENSIONS") {
                Ok(value) => parse_extensions(&value)?,
                Err(_) => defaults.image_extensions,
            },
            audio_extensions: match env::var("AUDIO_EXTENSIONS") {
                Ok(value) => parse_extensions(&value)?,
                Err(_) => defaults.audio_extensions,
            },
            pdf_extensions: match env::var("PDF_EXTENSIONS") {
                Ok(value) => parse_extensions(&value)?,
                Err(_) => defaults.pdf_extensions,
            },
            pdf_render_dpi: bounded_u32_var("PDF_RENDER_DPI", defaults.pdf_render_dpi, 72, 300)?,
            pdf_max_pages: bounded_u32_var("PDF_MAX_PAGES", defaults.pdf_max_pages, 1, 10_000)?,
            pdf_summary_pages: bounded_usize_var(
                "PDF_SUMMARY_PAGES",
                defaults.pdf_summary_pages,
                1,
                256,
            )?,
            audio_transcription_enabled: bool_var(
                "AUDIO_TRANSCRIPTION_ENABLED",
                defaults.audio_transcription_enabled,
            ),
            audio_transcription_model: string_var(
                "AUDIO_TRANSCRIPTION_MODEL",
                defaults.audio_transcription_model,
            ),
            audio_transcription_language: optional_string_var("AUDIO_TRANSCRIPTION_LANGUAGE")
                .or(defaults.audio_transcription_language),
            audio_transcription_threads: optional_bounded_usize_var(
                "AUDIO_TRANSCRIPTION_THREADS",
                1,
                usize::MAX,
            )?,
            audio_transcription_auto_download: bool_var(
                "AUDIO_TRANSCRIPTION_AUTO_DOWNLOAD",
                defaults.audio_transcription_auto_download,
            ),
            audio_transcription_cache_dir: optional_string_var("AUDIO_TRANSCRIPTION_CACHE_DIR")
                .map(PathBuf::from)
                .or(defaults.audio_transcription_cache_dir),
            media_sources_file,
            media_sources_seed_file,
            image_sources,
            source_watching_enabled: bool_var(
                "SOURCE_WATCHING_ENABLED",
                defaults.source_watching_enabled,
            ),
            source_watching_debounce_ms: bounded_u64_var(
                "SOURCE_WATCHING_DEBOUNCE_MS",
                defaults.source_watching_debounce_ms,
                100,
                600_000,
            )?,
            minio_endpoint: optional_string_var("MINIO_ENDPOINT"),
            minio_access_key: optional_string_var("MINIO_ACCESS_KEY"),
            minio_secret_key: optional_string_var("MINIO_SECRET_KEY"),
            minio_secure: bool_var("MINIO_SECURE", defaults.minio_secure),
            s3_endpoint: optional_string_var("S3_ENDPOINT"),
            s3_access_key_id: optional_string_var("S3_ACCESS_KEY_ID"),
            s3_secret_access_key: optional_string_var("S3_SECRET_ACCESS_KEY"),
            s3_region: string_var("S3_REGION", defaults.s3_region),
            s3_allow_http: bool_var("S3_ALLOW_HTTP", defaults.s3_allow_http),
            video_frame_stride: bounded_u32_var(
                "VIDEO_FRAME_STRIDE",
                defaults.video_frame_stride,
                1,
                u32::MAX,
            )?,
            video_max_frames: optional_bounded_u32_var("VIDEO_MAX_FRAMES", 1, u32::MAX)?,
            camera_frame_stride: bounded_u32_var(
                "CAMERA_FRAME_STRIDE",
                defaults.camera_frame_stride,
                1,
                u32::MAX,
            )?,
            camera_max_frames: bounded_u32_var(
                "CAMERA_MAX_FRAMES",
                defaults.camera_max_frames,
                1,
                u32::MAX,
            )?,
            default_search_limit: bounded_u32_var(
                "DEFAULT_SEARCH_LIMIT",
                defaults.default_search_limit,
                1,
                100,
            )?,
            duplicate_hash_distance: bounded_u32_var(
                "DUPLICATE_HASH_DISTANCE",
                defaults.duplicate_hash_distance,
                0,
                64,
            )?,
            max_upload_mb: bounded_u32_var("MAX_UPLOAD_MB", defaults.max_upload_mb, 1, 200)?,
            vector_size: bounded_usize_var("VECTOR_SIZE", defaults.vector_size, 1, usize::MAX)?,
            gif_sample_frames: bounded_usize_var(
                "GIF_SAMPLE_FRAMES",
                defaults.gif_sample_frames,
                1,
                usize::MAX,
            )?,
            gif_max_decode_frames: bounded_usize_var(
                "GIF_MAX_DECODE_FRAMES",
                defaults.gif_max_decode_frames,
                1,
                usize::MAX,
            )?,
            gif_preview_frames: bounded_usize_var(
                "GIF_PREVIEW_FRAMES",
                defaults.gif_preview_frames,
                1,
                usize::MAX,
            )?,
            gif_default_frame_delay_ms: bounded_u32_var(
                "GIF_DEFAULT_FRAME_DELAY_MS",
                defaults.gif_default_frame_delay_ms,
                1,
                u32::MAX,
            )?,
            gif_motion_weight: bounded_f32_var(
                "GIF_MOTION_WEIGHT",
                defaults.gif_motion_weight,
                0.0,
                1.0,
            )?,
            face_analysis_enabled: bool_var(
                "FACE_ANALYSIS_ENABLED",
                defaults.face_analysis_enabled,
            ),
            face_detection_model_path: path_var(
                "FACE_DETECTION_MODEL_PATH",
                defaults.face_detection_model_path,
            ),
            face_embedding_model_path: path_var(
                "FACE_EMBEDDING_MODEL_PATH",
                defaults.face_embedding_model_path,
            ),
            face_embedding_vector_size: bounded_usize_var(
                "FACE_EMBEDDING_VECTOR_SIZE",
                defaults.face_embedding_vector_size,
                1,
                usize::MAX,
            )?,
            face_detection_min_confidence: bounded_f32_var(
                "FACE_DETECTION_MIN_CONFIDENCE",
                defaults.face_detection_min_confidence,
                0.0,
                1.0,
            )?,
            face_cluster_threshold: bounded_f32_var(
                "FACE_CLUSTER_THRESHOLD",
                defaults.face_cluster_threshold,
                0.0,
                2.0,
            )?,
            face_min_cluster_images: bounded_u32_var(
                "FACE_MIN_CLUSTER_IMAGES",
                defaults.face_min_cluster_images,
                1,
                u32::MAX,
            )?,
            face_max_frames_per_media: bounded_usize_var(
                "FACE_MAX_FRAMES_PER_MEDIA",
                defaults.face_max_frames_per_media,
                1,
                usize::MAX,
            )?,
            ocr_enabled: bool_var("OCR_ENABLED", defaults.ocr_enabled),
            ocr_command: string_var("OCR_COMMAND", defaults.ocr_command),
            ocr_language: optional_string_var("OCR_LANGUAGE").or(defaults.ocr_language),
            ocr_max_frames: bounded_usize_var("OCR_MAX_FRAMES", defaults.ocr_max_frames, 1, 64)?,
            bind_addr: string_var("BIND_ADDR", defaults.bind_addr),
        })
    }

    pub fn source_specs(&self) -> Vec<String> {
        if self.image_sources.is_empty() {
            vec![self.source_image_dir.to_string_lossy().to_string()]
        } else {
            self.image_sources.clone()
        }
    }
}

pub fn parse_extensions(value: &str) -> Result<BTreeSet<String>, String> {
    let extensions = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            if lower.starts_with('.') {
                lower
            } else {
                format!(".{lower}")
            }
        })
        .collect::<BTreeSet<_>>();
    if extensions.is_empty() {
        Err("At least one image extension is required".to_string())
    } else {
        Ok(extensions)
    }
}

pub fn parse_image_sources(value: &str) -> Result<Vec<String>, String> {
    let stripped = value.trim();
    if stripped.is_empty() {
        return Ok(Vec::new());
    }
    if stripped.starts_with('[') {
        let parsed: Vec<String> = serde_json::from_str(stripped)
            .map_err(|error| format!("IMAGE_SOURCES must be a JSON string array: {error}"))?;
        return Ok(parsed
            .into_iter()
            .map(|part| expand_local_source_spec(part.trim()))
            .filter(|part| !part.is_empty())
            .collect());
    }
    for separator in ['\n', ';', ','] {
        if stripped.contains(separator) {
            return Ok(stripped
                .split(separator)
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(expand_local_source_spec)
                .collect());
        }
    }
    Ok(vec![expand_local_source_spec(stripped)])
}

pub fn parse_media_sources_file(value: &str) -> Result<Vec<String>, String> {
    Ok(value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(expand_local_source_spec)
        .filter(|line| !line.is_empty())
        .collect())
}

fn read_media_sources_files(
    target_path: &Path,
    seed_path: Option<&Path>,
    target_required: bool,
) -> Result<Vec<String>, String> {
    match read_media_sources_file(target_path) {
        Ok(sources) => return Ok(sources),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Could not read MEDIA_SOURCES_FILE {}: {error}",
                target_path.display()
            ));
        }
    }

    if let Some(seed_path) = seed_path {
        return read_media_sources_file(seed_path).map_err(|error| {
            format!(
                "Could not read MEDIA_SOURCES_SEED_FILE {}: {error}",
                seed_path.display()
            )
        });
    }

    if target_required {
        return Err(format!(
            "Could not read MEDIA_SOURCES_FILE {}: file does not exist",
            target_path.display()
        ));
    }

    Ok(Vec::new())
}

fn read_media_sources_file(path: &Path) -> std::io::Result<Vec<String>> {
    fs::read_to_string(path).and_then(|value| {
        parse_media_sources_file(&value)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })
}

fn expand_local_source_spec(value: &str) -> String {
    if has_uri_scheme(value) {
        return value.to_string();
    }

    let expanded = expand_env_vars(value);
    if let Some(home) = home_dir() {
        if expanded == "~" {
            return home;
        }
        if let Some(rest) = expanded.strip_prefix("~/") {
            return format!("{home}/{rest}");
        }
    }
    expanded
}

fn has_uri_scheme(value: &str) -> bool {
    value
        .find(':')
        .map(|index| {
            value[..index].chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
        })
        .unwrap_or(false)
}

fn expand_env_vars(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '$' {
            output.push(character);
            continue;
        }

        if chars.peek() == Some(&'{') {
            chars.next();
            let mut name = String::new();
            for next in chars.by_ref() {
                if next == '}' {
                    break;
                }
                name.push(next);
            }
            output.push_str(&env::var(name).unwrap_or_default());
            continue;
        }

        let mut name = String::new();
        while let Some(next) = chars.peek() {
            if next.is_ascii_alphanumeric() || *next == '_' {
                name.push(*next);
                chars.next();
            } else {
                break;
            }
        }
        if name.is_empty() {
            output.push('$');
        } else {
            output.push_str(&env::var(name).unwrap_or_default());
        }
    }

    output
}

fn home_dir() -> Option<String> {
    optional_string_var("HOME")
}

fn string_var(name: &str, default: String) -> String {
    optional_string_var(name).unwrap_or(default)
}

fn optional_string_var(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn path_var(name: &str, default: PathBuf) -> PathBuf {
    optional_string_var(name)
        .map(PathBuf::from)
        .unwrap_or(default)
}

fn bool_var(name: &str, default: bool) -> bool {
    optional_string_var(name)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn bounded_u32_var(name: &str, default: u32, min: u32, max: u32) -> Result<u32, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<u32>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(parsed)
            }
        }
        None => Ok(default),
    }
}

fn bounded_u64_var(name: &str, default: u64, min: u64, max: u64) -> Result<u64, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<u64>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(parsed)
            }
        }
        None => Ok(default),
    }
}

fn optional_bounded_u32_var(name: &str, min: u32, max: u32) -> Result<Option<u32>, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<u32>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(Some(parsed))
            }
        }
        None => Ok(None),
    }
}

fn optional_bounded_usize_var(name: &str, min: usize, max: usize) -> Result<Option<usize>, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<usize>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(Some(parsed))
            }
        }
        None => Ok(None),
    }
}

fn bounded_usize_var(name: &str, default: usize, min: usize, max: usize) -> Result<usize, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<usize>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(parsed)
            }
        }
        None => Ok(default),
    }
}

fn bounded_f32_var(name: &str, default: f32, min: f32, max: f32) -> Result<f32, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<f32>()
                .map_err(|_| format!("{name} must be a number"))?;
            if !parsed.is_finite() || parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(parsed)
            }
        }
        None => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{parse_extensions, parse_image_sources, parse_media_sources_file, Settings};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn extensions_are_normalized() {
        let parsed = parse_extensions("jpg, .PNG, webp").unwrap();
        assert_eq!(
            parsed.into_iter().collect::<Vec<_>>(),
            vec![".jpg", ".png", ".webp"]
        );
    }

    #[test]
    fn image_sources_accept_delimited_strings_and_json() {
        assert_eq!(
            parse_image_sources("local:///images; minio://bucket/prefix; s3://archive/photos")
                .unwrap(),
            vec![
                "local:///images",
                "minio://bucket/prefix",
                "s3://archive/photos"
            ]
        );
        assert_eq!(
            parse_image_sources(r#"["/images", "video:///clips/demo.mp4"]"#).unwrap(),
            vec!["/images", "video:///clips/demo.mp4"]
        );
    }

    #[test]
    fn media_sources_file_accepts_ignore_style_comments_and_expands_paths() {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut input = "# Default user media folders.\n/srv/audio\n".to_string();
        let mut expected = vec!["/srv/audio".to_string()];
        if !home.is_empty() {
            input.push_str("~/Pictures\n$HOME/Videos\n");
            expected.push(format!("{home}/Pictures"));
            expected.push(format!("{home}/Videos"));
        }
        assert_eq!(parse_media_sources_file(&input).unwrap(), expected);
    }

    #[test]
    fn empty_extensions_are_rejected() {
        assert!(parse_extensions(" , ").is_err());
    }

    #[test]
    fn default_extensions_include_gif() {
        assert!(Settings::default().image_extensions.contains(".gif"));
    }

    #[test]
    fn default_audio_extensions_include_mp3() {
        assert!(Settings::default().audio_extensions.contains(".mp3"));
    }

    #[test]
    fn default_pdf_extensions_include_pdf() {
        assert!(Settings::default().pdf_extensions.contains(".pdf"));
    }

    #[test]
    fn default_qdrant_http_settings_are_bounded() {
        let settings = Settings::default();

        assert_eq!(settings.qdrant_request_timeout_ms, 30_000);
        assert_eq!(settings.qdrant_connect_timeout_ms, 2_000);
        assert_eq!(settings.qdrant_retry_attempts, 2);
        assert_eq!(settings.qdrant_retry_backoff_ms, 100);
    }

    #[test]
    fn qdrant_http_settings_are_loaded_from_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set([
            ("QDRANT_REQUEST_TIMEOUT_MS", Some("45000")),
            ("QDRANT_CONNECT_TIMEOUT_MS", Some("3000")),
            ("QDRANT_RETRY_ATTEMPTS", Some("4")),
            ("QDRANT_RETRY_BACKOFF_MS", Some("250")),
            ("IMAGE_SOURCES", Some("/images")),
            ("MEDIA_SOURCES_FILE", None),
            ("MEDIA_SOURCES_SEED_FILE", None),
        ]);

        let settings = Settings::from_env().unwrap();

        assert_eq!(settings.qdrant_request_timeout_ms, 45_000);
        assert_eq!(settings.qdrant_connect_timeout_ms, 3_000);
        assert_eq!(settings.qdrant_retry_attempts, 4);
        assert_eq!(settings.qdrant_retry_backoff_ms, 250);
    }

    #[test]
    fn invalid_qdrant_http_settings_are_rejected() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set([
            ("QDRANT_REQUEST_TIMEOUT_MS", Some("999")),
            ("QDRANT_CONNECT_TIMEOUT_MS", None),
            ("QDRANT_RETRY_ATTEMPTS", None),
            ("QDRANT_RETRY_BACKOFF_MS", None),
            ("IMAGE_SOURCES", Some("/images")),
            ("MEDIA_SOURCES_FILE", None),
            ("MEDIA_SOURCES_SEED_FILE", None),
        ]);

        let error = Settings::from_env().unwrap_err();

        assert!(error.contains("QDRANT_REQUEST_TIMEOUT_MS"));
        assert!(error.contains("between 1000 and 600000"));
    }

    #[test]
    fn qdrant_connect_timeout_must_not_exceed_request_timeout() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set([
            ("QDRANT_REQUEST_TIMEOUT_MS", Some("1000")),
            ("QDRANT_CONNECT_TIMEOUT_MS", Some("2000")),
            ("QDRANT_RETRY_ATTEMPTS", None),
            ("QDRANT_RETRY_BACKOFF_MS", None),
            ("IMAGE_SOURCES", Some("/images")),
            ("MEDIA_SOURCES_FILE", None),
            ("MEDIA_SOURCES_SEED_FILE", None),
        ]);

        let error = Settings::from_env().unwrap_err();

        assert!(error.contains("QDRANT_CONNECT_TIMEOUT_MS"));
        assert!(error.contains("QDRANT_REQUEST_TIMEOUT_MS"));
    }

    #[test]
    fn image_sources_env_wins_over_media_source_files() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = TestTempDir::new();
        let target = temp.path().join("target-sources.txt");
        let seed = temp.path().join("seed-sources.txt");
        fs::write(&target, "/target\n").unwrap();
        fs::write(&seed, "/seed\n").unwrap();
        let _env = EnvGuard::set([
            ("IMAGE_SOURCES", Some("/env-a,/env-b")),
            ("MEDIA_SOURCES_FILE", Some(target.to_str().unwrap())),
            ("MEDIA_SOURCES_SEED_FILE", Some(seed.to_str().unwrap())),
        ]);

        let settings = Settings::from_env().unwrap();

        assert_eq!(settings.image_sources, vec!["/env-a", "/env-b"]);
        assert_eq!(settings.media_sources_file, target);
        assert_eq!(settings.media_sources_seed_file, Some(seed));
    }

    #[test]
    fn missing_media_source_target_loads_seed_file() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = TestTempDir::new();
        let target = temp.path().join("target-sources.txt");
        let seed = temp.path().join("seed-sources.txt");
        fs::write(&seed, "/seed\n").unwrap();
        let _env = EnvGuard::set([
            ("IMAGE_SOURCES", None),
            ("MEDIA_SOURCES_FILE", Some(target.to_str().unwrap())),
            ("MEDIA_SOURCES_SEED_FILE", Some(seed.to_str().unwrap())),
        ]);

        let settings = Settings::from_env().unwrap();

        assert_eq!(settings.image_sources, vec!["/seed"]);
        assert_eq!(settings.media_sources_file, target);
        assert_eq!(settings.media_sources_seed_file, Some(seed));
    }

    #[test]
    fn existing_media_source_target_wins_over_seed_file() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = TestTempDir::new();
        let target = temp.path().join("target-sources.txt");
        let seed = temp.path().join("seed-sources.txt");
        fs::write(&target, "/target\n").unwrap();
        fs::write(&seed, "/seed\n").unwrap();
        let _env = EnvGuard::set([
            ("IMAGE_SOURCES", None),
            ("MEDIA_SOURCES_FILE", Some(target.to_str().unwrap())),
            ("MEDIA_SOURCES_SEED_FILE", Some(seed.to_str().unwrap())),
        ]);

        let settings = Settings::from_env().unwrap();

        assert_eq!(settings.image_sources, vec!["/target"]);
    }

    #[test]
    fn explicit_missing_media_source_target_without_seed_errors() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = TestTempDir::new();
        let target = temp.path().join("missing-sources.txt");
        let _env = EnvGuard::set([
            ("IMAGE_SOURCES", None),
            ("MEDIA_SOURCES_FILE", Some(target.to_str().unwrap())),
            ("MEDIA_SOURCES_SEED_FILE", None),
        ]);

        let error = Settings::from_env().unwrap_err();

        assert!(error.contains("MEDIA_SOURCES_FILE"));
        assert!(error.contains("file does not exist"));
    }

    #[test]
    fn implicit_missing_media_source_target_without_seed_falls_back_to_default_source_dir() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = TestTempDir::new();
        let source_dir = temp.path().join("sources");
        let _dir = CurrentDirGuard::set(temp.path());
        let _env = EnvGuard::set([
            ("IMAGE_SOURCES", None),
            ("MEDIA_SOURCES_FILE", None),
            ("MEDIA_SOURCES_SEED_FILE", None),
            ("SOURCE_IMAGE_DIR", Some(source_dir.to_str().unwrap())),
        ]);

        let settings = Settings::from_env().unwrap();

        assert!(settings.image_sources.is_empty());
        assert_eq!(
            settings.source_specs(),
            vec![source_dir.to_string_lossy().to_string()]
        );
    }

    struct EnvGuard {
        previous: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn set<const N: usize>(values: [(&'static str, Option<&str>); N]) -> Self {
            let previous = values
                .iter()
                .map(|(name, _)| (*name, std::env::var(name).ok()))
                .collect::<Vec<_>>();
            for (name, value) in values {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
            Self { previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.previous {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "image-sim-config-test-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    struct CurrentDirGuard {
        previous: PathBuf,
    }

    impl CurrentDirGuard {
        fn set(path: &Path) -> Self {
            let previous = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { previous }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.previous);
        }
    }
}
