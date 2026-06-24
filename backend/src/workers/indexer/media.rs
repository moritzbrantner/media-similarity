struct PayloadBuildOptions<'a> {
    video_scene: Option<&'a SourceVideoScene>,
    audio_segment: Option<&'a SourceAudioSegment>,
    pdf_context: Option<&'a PdfPayloadContext>,
    ocr_override: Option<OcrAnalysis>,
    photo_metadata: Option<PhotoMetadataPayload>,
    face_analysis: &'a FaceAnalysis,
    animated_thumbnail_enabled: bool,
}

impl<'a> PayloadBuildOptions<'a> {
    fn new(face_analysis: &'a FaceAnalysis) -> Self {
        Self {
            video_scene: None,
            audio_segment: None,
            pdf_context: None,
            ocr_override: None,
            photo_metadata: None,
            face_analysis,
            animated_thumbnail_enabled: true,
        }
    }

    fn with_video_scene(mut self, scene: &'a SourceVideoScene) -> Self {
        self.video_scene = Some(scene);
        self
    }

    fn with_audio_segment(mut self, segment: &'a SourceAudioSegment) -> Self {
        self.audio_segment = Some(segment);
        self
    }

    fn with_pdf_context(mut self, context: &'a PdfPayloadContext) -> Self {
        self.pdf_context = Some(context);
        self
    }

    fn with_ocr(mut self, analysis: OcrAnalysis) -> Self {
        self.ocr_override = Some(analysis);
        self
    }

    fn with_photo_metadata(mut self, photo_metadata: Option<PhotoMetadataPayload>) -> Self {
        self.photo_metadata = photo_metadata;
        self
    }

    fn with_animated_thumbnail(mut self, enabled: bool) -> Self {
        self.animated_thumbnail_enabled = enabled;
        self
    }
}

struct PdfPayloadContext {
    id_base: String,
    relative_path: String,
    filename: String,
    path: String,
    full_pdf_url: Option<String>,
    pdf_page_url: Option<String>,
    pdf_document_id: Option<String>,
    pdf_page_index: Option<usize>,
    pdf_page_number: Option<usize>,
    pdf_page_count: Option<usize>,
}

#[derive(Default)]
struct IndexOneOutcome {
    indexed: usize,
    point_ids: BTreeSet<String>,
}

impl IndexOneOutcome {
    fn single(point_id: String) -> Self {
        let mut outcome = Self::default();
        outcome.insert(point_id);
        outcome
    }

    fn insert(&mut self, point_id: String) {
        self.indexed += 1;
        self.point_ids.insert(point_id);
    }
}

fn generated_artifacts(
    thumbnail_url: Option<&str>,
    animated_thumbnail_url: Option<&str>,
    full_video_url: Option<&str>,
    full_audio_url: Option<&str>,
    full_pdf_url: Option<&str>,
    pdf_page_url: Option<&str>,
    scene_clip_url: Option<&str>,
) -> Vec<GeneratedArtifactPayload> {
    [
        ("thumbnail", thumbnail_url),
        ("animated_thumbnail", animated_thumbnail_url),
        ("source_video", full_video_url),
        ("source_audio", full_audio_url),
        ("source_pdf", full_pdf_url),
        ("pdf_page", pdf_page_url),
        ("video_scene", scene_clip_url),
    ]
    .into_iter()
    .filter_map(|(kind, maybe_url)| {
        let raw = maybe_url?;
        let url = raw.split_once('#').map_or(raw, |(base, _)| base);
        (!url.is_empty()).then(|| GeneratedArtifactPayload {
            kind: kind.to_string(),
            url: url.to_string(),
        })
    })
    .collect()
}

fn indexing_profile(settings: &Settings) -> String {
    let profile = IndexingProfile {
        version: 6,
        processing_workflows_hash: settings.processing_workflows_hash.as_deref(),
        photo_metadata_version: "photo-metadata-v1",
        clip_model_name: &settings.clip_model_name,
        vector_size: settings.vector_size,
        visual_embedding_enabled: settings.visual_embedding_enabled,
        visual_embedding_backend: &settings.visual_embedding_backend,
        visual_embedding_model_path: settings.visual_embedding_model_path.to_string_lossy(),
        visual_embedding_preprocessor_path: settings
            .visual_embedding_preprocessor_path
            .to_string_lossy(),
        visual_embedding_vector_size: settings.visual_embedding_vector_size,
        visual_embedding_batch_size: settings.visual_embedding_batch_size,
        face_analysis_enabled: settings.face_analysis_enabled,
        face_detection_model_path: settings.face_detection_model_path.to_string_lossy(),
        face_embedding_model_path: settings.face_embedding_model_path.to_string_lossy(),
        face_embedding_vector_size: settings.face_embedding_vector_size,
        face_detection_min_confidence_bits: settings.face_detection_min_confidence.to_bits(),
        face_cluster_threshold_bits: settings.face_cluster_threshold.to_bits(),
        face_min_cluster_images: settings.face_min_cluster_images,
        face_max_frames_per_media: settings.face_max_frames_per_media,
        gif_sample_frames: settings.gif_sample_frames,
        gif_max_decode_frames: settings.gif_max_decode_frames,
        gif_preview_frames: settings.gif_preview_frames,
        gif_default_frame_delay_ms: settings.gif_default_frame_delay_ms,
        gif_motion_weight_bits: settings.gif_motion_weight.to_bits(),
        video_frame_stride: settings.video_frame_stride,
        video_max_frames: settings.video_max_frames,
        pdf_render_dpi: settings.pdf_render_dpi,
        pdf_max_pages: settings.pdf_max_pages,
        pdf_summary_pages: settings.pdf_summary_pages,
        audio_transcription_enabled: settings.audio_transcription_enabled,
        audio_transcription_model: &settings.audio_transcription_model,
        audio_transcription_language: settings.audio_transcription_language.as_deref(),
        audio_transcription_threads: settings.audio_transcription_threads,
        ocr_enabled: settings.ocr_enabled,
        ocr_command: &settings.ocr_command,
        ocr_language: settings.ocr_language.as_deref(),
        ocr_max_frames: settings.ocr_max_frames,
    };
    let encoded = serde_json::to_vec(&profile).unwrap_or_default();
    let digest = Sha256::digest(encoded);
    format!("v{}:{digest:x}", profile.version)
}

#[derive(Serialize)]
struct IndexingProfile<'a> {
    version: u32,
    processing_workflows_hash: Option<&'a str>,
    photo_metadata_version: &'a str,
    clip_model_name: &'a str,
    vector_size: usize,
    visual_embedding_enabled: bool,
    visual_embedding_backend: &'a str,
    visual_embedding_model_path: std::borrow::Cow<'a, str>,
    visual_embedding_preprocessor_path: std::borrow::Cow<'a, str>,
    visual_embedding_vector_size: usize,
    visual_embedding_batch_size: usize,
    face_analysis_enabled: bool,
    face_detection_model_path: std::borrow::Cow<'a, str>,
    face_embedding_model_path: std::borrow::Cow<'a, str>,
    face_embedding_vector_size: usize,
    face_detection_min_confidence_bits: u32,
    face_cluster_threshold_bits: u32,
    face_min_cluster_images: u32,
    face_max_frames_per_media: usize,
    gif_sample_frames: usize,
    gif_max_decode_frames: usize,
    gif_preview_frames: usize,
    gif_default_frame_delay_ms: u32,
    gif_motion_weight_bits: u32,
    video_frame_stride: u32,
    video_max_frames: Option<u32>,
    pdf_render_dpi: u32,
    pdf_max_pages: u32,
    pdf_summary_pages: usize,
    audio_transcription_enabled: bool,
    audio_transcription_model: &'a str,
    audio_transcription_language: Option<&'a str>,
    audio_transcription_threads: Option<usize>,
    ocr_enabled: bool,
    ocr_command: &'a str,
    ocr_language: Option<&'a str>,
    ocr_max_frames: usize,
}

fn index_progress(
    completed: u64,
    total: u64,
    message: impl Into<String>,
) -> jobs_core::Result<JobProgress> {
    let total = (total > 0).then_some(total);
    let progress = JobProgress::new(completed, total)?
        .unit("files")?
        .message(message);
    progress.validate()?;
    Ok(progress)
}
