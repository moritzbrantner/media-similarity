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
            indexing_ledger_file: path_var("INDEXING_LEDGER_FILE", defaults.indexing_ledger_file),
            processing_workflows_file: path_var(
                "PROCESSING_WORKFLOWS_FILE",
                defaults.processing_workflows_file,
            ),
            processing_workflows_hash: None,
            voice_registry_path: path_var("VOICE_REGISTRY_PATH", defaults.voice_registry_path),
            smart_albums_file: path_var("SMART_ALBUMS_FILE", defaults.smart_albums_file),
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
            startup_indexing_enabled: bool_var(
                "STARTUP_INDEXING_ENABLED",
                defaults.startup_indexing_enabled,
            ),
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
            frontend_serving_enabled: bool_var(
                "FRONTEND_SERVING_ENABLED",
                defaults.frontend_serving_enabled,
            ),
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
