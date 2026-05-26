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
    legacy_source_item_uri, payload_analysis_complete, record_is_current, source_is_current,
    IndexedSourceRecord, PendingSource, SourceIndexPlan,
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

#[derive(Clone)]
pub struct ImageIndexer {
    settings: Settings,
    store: Arc<dyn MediaVectorStore>,
    embedder: Arc<dyn VisualEmbeddingBackend>,
}
