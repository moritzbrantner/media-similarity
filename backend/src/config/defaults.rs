impl Settings {
    pub fn server_settings(&self) -> ServerSettings {
        ServerSettings {
            bind_addr: self.bind_addr.clone(),
            frontend_serving_enabled: self.frontend_serving_enabled,
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
            audio_transcription_provider: self.audio_transcription_provider.clone(),
            audio_transcription_model: self.audio_transcription_model.clone(),
            audio_transcription_language: self.audio_transcription_language.clone(),
            audio_transcription_device: self.audio_transcription_device.clone(),
            audio_transcription_compute_type: self.audio_transcription_compute_type.clone(),
            audio_transcription_threads: self.audio_transcription_threads,
            audio_transcription_auto_download: self.audio_transcription_auto_download,
            audio_transcription_cache_dir: self.audio_transcription_cache_dir.clone(),
            audio_transcription_batch_chunks: self.audio_transcription_batch_chunks,
            audio_transcription_max_batch_size: self.audio_transcription_max_batch_size,
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
            indexing_ledger_file: PathBuf::from("data/indexing-ledger.json"),
            processing_workflows_file: PathBuf::from("data/processing-workflows.json"),
            processing_workflows_hash: None,
            voice_registry_path: PathBuf::from("data/recognized-voices.json"),
            smart_albums_file: PathBuf::from("data/smart-albums.json"),
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
            audio_transcription_provider: "candle-whisper".to_string(),
            audio_transcription_model: "openai/whisper-large-v3-turbo".to_string(),
            audio_transcription_language: Some("en".to_string()),
            audio_transcription_device: "auto".to_string(),
            audio_transcription_compute_type: "automatic".to_string(),
            audio_transcription_threads: None,
            audio_transcription_auto_download: false,
            audio_transcription_cache_dir: None,
            audio_transcription_batch_chunks: true,
            audio_transcription_max_batch_size: Some(4),
            media_sources_file: PathBuf::from("config/media-sources.txt"),
            media_sources_seed_file: None,
            image_sources: Vec::new(),
            startup_indexing_enabled: false,
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
            bind_addr: "127.0.0.1:8000".to_string(),
            frontend_serving_enabled: true,
        }
    }
}
