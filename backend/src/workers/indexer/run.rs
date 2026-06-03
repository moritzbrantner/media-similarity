use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use jobs_core::{JobContext, JobProgress};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::config::Settings;
use crate::domain::models::{
    GeneratedArtifactPayload, ImagePayload, IndexResponse, OcrAnalysis, PhotoMetadataPayload,
};
use crate::storage::MediaVectorStore;
use crate::workers::deletion::delete_indexed_media;
use crate::workers::indexing::planner::{
    committed_records_are_current, legacy_source_item_uri, payload_analysis_complete,
    record_is_current, source_is_current, IndexedSourceRecord, PendingSource, SourceIndexPlan,
};
use crate::workers::media::audio::{
    decode_source_audio_segments, expose_source_audio, SourceAudioSegment,
};
use crate::workers::media::faces::{analyze_faces_for_media, FaceAnalysis};
use crate::workers::media::hashing::phash_image;
use crate::workers::media::image_io::{dimensions, image_id_for_uri};
use crate::workers::media::media::{DecodedMedia, MediaKind};
use crate::workers::media::ocr::extract_media_ocr;
use crate::workers::media::pdf::{decode_pdf, expose_source_pdf, merge_pdf_text};
use crate::workers::media::photo_metadata::extract_photo_metadata;
use crate::workers::media::thumbnails::{ensure_animated_thumbnail, ensure_thumbnail};
use crate::workers::media::video::{decode_source_video_scenes, SourceVideoScene};
use crate::workers::media::visual_embedding::VisualEmbeddingBackend;
use crate::workers::sources::{build_image_sources, SourceImage, SourceUnavailable};
use crate::workers::workflows::{
    compile_media_workflow, default_media_workflow_library, load_media_workflow_library,
    validate_media_workflow_library, CompiledMediaWorkflow, MediaFileKind, WorkflowMode,
};

#[derive(Clone)]
pub struct ImageIndexer {
    settings: Settings,
    store: Arc<dyn MediaVectorStore>,
    embedder: Arc<dyn VisualEmbeddingBackend>,
}

impl ImageIndexer {
    fn workflow_settings(
        &self,
        kind: MediaFileKind,
    ) -> Result<(Settings, CompiledMediaWorkflow), String> {
        let library = load_media_workflow_library(&self.settings.processing_workflows_file)
            .ok()
            .filter(|library| validate_media_workflow_library(library).is_empty())
            .unwrap_or_else(|| default_media_workflow_library(&self.settings));
        let workflow = compile_media_workflow(kind, WorkflowMode::Index, &library)?;
        let mut settings = self.settings.clone();
        workflow.apply_to_settings(&mut settings);
        Ok((settings, workflow))
    }
}

fn source_file_kind(source_image: &SourceImage) -> MediaFileKind {
    if source_image.is_video() {
        return MediaFileKind::Video;
    }
    if source_image.is_audio() {
        return MediaFileKind::Audio;
    }
    if source_image.is_pdf() {
        return MediaFileKind::Pdf;
    }
    if source_image
        .filename
        .rsplit_once('.')
        .map(|(_, extension)| extension.eq_ignore_ascii_case("gif"))
        .unwrap_or(false)
    {
        MediaFileKind::AnimatedGif
    } else {
        MediaFileKind::StaticImage
    }
}
