use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::sync::Arc;

use axum::extract::{Multipart, Path as AxumPath, Query, State};
use axum::Json;
use jobs_core::{
    JobArtifact, JobContext, JobError, JobEvent, JobId, JobProgress, JobSnapshot, JobSpec,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use text_transcripts::{WhisperCppModel, WhisperCppModelStore};
use uuid::Uuid;

use crate::config::{parse_extensions, parse_media_sources_file, Settings};
use crate::domain::models::{ImagePayload, IndexResponse, SearchResponse};
use crate::domain::models::{SearchResult, SearchSceneResponse};
use crate::storage::MediaVectorStore;
use crate::workers::deletion::{
    delete_indexed_media, delete_indexed_source, DeleteIndexResponse, DeleteIndexedSourceFilter,
};
use crate::workers::indexer::ImageIndexer;
use crate::workers::media::audio::{
    audio_upload_path, decode_audio_segments, is_audio_content_type, is_audio_extension,
    whisper_model_is_cached, write_audio_upload,
};
use crate::workers::media::image_io::load_media_bytes;
use crate::workers::media::models::{
    audio_transcription_model_store, download_role_bundle, model_statuses, parse_whisper_cpp_model,
    ModelRole, ModelRuntimeStatus,
};
use crate::workers::media::ocr::normalize_ocr_query;
use crate::workers::media::pdf::{
    decode_pdf, is_pdf_content_type, is_pdf_extension, pdf_upload_path, write_pdf_upload,
};
use crate::workers::media::video::{
    decode_video_scenes, is_video_content_type, is_video_extension, video_upload_path,
    write_video_upload,
};
use crate::workers::media::visual_embedding::VisualEmbeddingBackend;
use crate::workers::search::{
    ImageSearchService, NearDuplicateFilter, OrientationFilter, SearchFilters,
};

mod error;
mod health;
mod readiness;
mod state;

pub use error::ApiError;
pub use health::health;
pub use readiness::ready;
pub use state::AppState;

const MAX_MEDIA_TAGS: usize = 64;
const MAX_MEDIA_TAG_LENGTH: usize = 80;

#[derive(Deserialize)]
pub struct SearchQuery {
    limit: Option<u32>,
    ocr_text: Option<String>,
    person_id: Option<String>,
    source_type: Option<String>,
    media_kind: Option<String>,
    name_query: Option<String>,
    camera_query: Option<String>,
    keyword_query: Option<String>,
    has_gps: Option<String>,
    near_duplicate: Option<String>,
    orientation: Option<String>,
    min_width: Option<u32>,
    max_width: Option<u32>,
    min_height: Option<u32>,
    max_height: Option<u32>,
    min_size_bytes: Option<u64>,
    max_size_bytes: Option<u64>,
    modified_from: Option<f64>,
    modified_to: Option<f64>,
    captured_from: Option<f64>,
    captured_to: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct InverseIndexResponse {
    pub indexed_media: usize,
    pub people: Vec<InversePersonEntry>,
    pub speakers: Vec<InverseSpeakerEntry>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct InversePersonEntry {
    pub id: String,
    pub label: Option<String>,
    pub face_count: u32,
    pub media_count: usize,
    pub confidence: f32,
    pub locations: Vec<InverseIndexLocation>,
}

#[derive(Debug, Serialize)]
pub struct InverseSpeakerEntry {
    pub id: String,
    pub label: Option<String>,
    pub segment_count: u32,
    pub total_seconds: f64,
    pub media_count: usize,
    pub confidence: f32,
    pub locations: Vec<InverseIndexLocation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InverseIndexLocation {
    pub media_id: String,
    pub filename: String,
    pub relative_path: String,
    pub path: String,
    pub media_kind: String,
    pub source_type: String,
    pub source_uri: Option<String>,
    pub source_item_uri: Option<String>,
    pub thumbnail_url: Option<String>,
    pub media_url: Option<String>,
    pub scene_clip_url: Option<String>,
    pub occurrence_count: u32,
    pub frame_indices: Vec<usize>,
    pub start_seconds: Option<f64>,
    pub end_seconds: Option<f64>,
    pub page_number: Option<usize>,
    pub confidence: f32,
}

#[derive(Default)]
struct PersonAccumulator {
    label: Option<String>,
    face_count: u32,
    confidence_total: f32,
    confidence_count: u32,
    locations: BTreeMap<String, InverseIndexLocation>,
}

#[derive(Default)]
struct SpeakerAccumulator {
    label: Option<String>,
    segment_count: u32,
    total_seconds: f64,
    confidence_total: f32,
    confidence_count: u32,
    locations: BTreeMap<String, InverseIndexLocation>,
}

#[derive(Default)]
struct PersonMediaStats {
    label: Option<String>,
    face_count: u32,
    confidence_total: f32,
    confidence_count: u32,
    frame_indices: Vec<usize>,
}

pub async fn inverse_index(
    State(state): State<Arc<AppState>>,
) -> Result<Json<InverseIndexResponse>, ApiError> {
    let points = state
        .store
        .scroll_media_points()
        .await
        .map_err(ApiError::internal)?;
    let mut people = BTreeMap::<String, PersonAccumulator>::new();
    let mut speakers = BTreeMap::<String, SpeakerAccumulator>::new();
    let mut indexed_media = 0;
    let mut errors = Vec::new();

    for point in points {
        let Some(payload_value) = point.payload else {
            errors.push(format!("{}: missing payload", point.id));
            continue;
        };
        let payload = match serde_json::from_value::<ImagePayload>(payload_value) {
            Ok(payload) => payload,
            Err(error) => {
                errors.push(format!("{}: could not decode payload: {error}", point.id));
                continue;
            }
        };

        indexed_media += 1;
        collect_people(&payload, &mut people);
        collect_speakers(&payload, &mut speakers);
    }

    Ok(Json(InverseIndexResponse {
        indexed_media,
        people: people
            .into_iter()
            .map(|(id, entry)| InversePersonEntry {
                confidence: average_confidence(entry.confidence_total, entry.confidence_count),
                face_count: entry.face_count,
                id,
                label: entry.label,
                media_count: entry.locations.len(),
                locations: sorted_locations(entry.locations),
            })
            .collect(),
        speakers: speakers
            .into_iter()
            .map(|(id, entry)| InverseSpeakerEntry {
                confidence: average_confidence(entry.confidence_total, entry.confidence_count),
                id,
                label: entry.label,
                media_count: entry.locations.len(),
                segment_count: entry.segment_count,
                total_seconds: entry.total_seconds,
                locations: sorted_locations(entry.locations),
            })
            .collect(),
        errors,
    }))
}

fn collect_people(payload: &ImagePayload, people: &mut BTreeMap<String, PersonAccumulator>) {
    let mut by_person = BTreeMap::<String, PersonMediaStats>::new();

    for face in &payload.faces {
        let Some(person_id) = face.person_id.as_deref().filter(|id| !id.is_empty()) else {
            continue;
        };
        let stats = by_person.entry(person_id.to_string()).or_default();
        stats.label = stats.label.clone().or_else(|| face.person_label.clone());
        stats.face_count += 1;
        stats.confidence_total += face.confidence;
        stats.confidence_count += 1;
        if !stats.frame_indices.contains(&face.frame_index) {
            stats.frame_indices.push(face.frame_index);
        }
    }

    for person in &payload.people {
        let person_id = person.person_id.trim();
        if person_id.is_empty() {
            continue;
        }
        let stats = by_person.entry(person_id.to_string()).or_default();
        stats.label = stats.label.clone().or_else(|| person.label.clone());
        stats.face_count = stats.face_count.max(person.face_count);
        stats.confidence_total += person.confidence;
        stats.confidence_count += 1;
    }

    for (person_id, mut stats) in by_person {
        stats.frame_indices.sort_unstable();
        let confidence = average_confidence(stats.confidence_total, stats.confidence_count);
        let entry = people.entry(person_id).or_default();
        entry.label = entry.label.clone().or(stats.label);
        entry.face_count += stats.face_count;
        entry.confidence_total += stats.confidence_total;
        entry.confidence_count += stats.confidence_count;

        let location = entry
            .locations
            .entry(payload.id.clone())
            .or_insert_with(|| base_location(payload));
        location.occurrence_count += stats.face_count.max(1);
        location.confidence = location.confidence.max(confidence);
        for frame_index in stats.frame_indices {
            if !location.frame_indices.contains(&frame_index) {
                location.frame_indices.push(frame_index);
            }
        }
        location.frame_indices.sort_unstable();
    }
}

fn collect_speakers(payload: &ImagePayload, speakers: &mut BTreeMap<String, SpeakerAccumulator>) {
    let Some(analysis) = &payload.audio_analysis else {
        return;
    };

    for voice in &analysis.recognized_voices {
        let voice_id = voice.id.trim();
        if voice_id.is_empty() {
            continue;
        }

        let mut segment_count = 0;
        let mut start_seconds = None::<f64>;
        let mut end_seconds = None::<f64>;
        for segment in analysis.audio_segments.iter().filter(|segment| {
            segment
                .speaker_id
                .as_deref()
                .map(|speaker_id| speaker_id == voice_id)
                .unwrap_or(false)
        }) {
            segment_count += 1;
            start_seconds = Some(
                start_seconds
                    .map(|current| current.min(segment.start_seconds))
                    .unwrap_or(segment.start_seconds),
            );
            end_seconds = Some(
                end_seconds
                    .map(|current| current.max(segment.end_seconds))
                    .unwrap_or(segment.end_seconds),
            );
        }

        let entry = speakers.entry(voice_id.to_string()).or_default();
        entry.label = entry.label.clone().or_else(|| Some(voice.label.clone()));
        entry.segment_count += voice.segment_count;
        entry.total_seconds += voice.total_seconds;
        entry.confidence_total += voice.confidence;
        entry.confidence_count += 1;

        let location = entry
            .locations
            .entry(payload.id.clone())
            .or_insert_with(|| base_location(payload));
        location.occurrence_count += segment_count.max(voice.segment_count).max(1);
        location.start_seconds = min_optional(location.start_seconds, start_seconds);
        location.end_seconds = max_optional(location.end_seconds, end_seconds);
        location.confidence = location.confidence.max(voice.confidence);
    }
}

fn base_location(payload: &ImagePayload) -> InverseIndexLocation {
    InverseIndexLocation {
        media_id: payload.id.clone(),
        filename: payload.filename.clone(),
        relative_path: payload.relative_path.clone(),
        path: payload.path.clone(),
        media_kind: payload.media_kind.clone(),
        source_type: payload.source_type.clone(),
        source_uri: payload.source_uri.clone(),
        source_item_uri: payload.source_item_uri.clone(),
        thumbnail_url: payload
            .animated_thumbnail_url
            .clone()
            .or_else(|| payload.thumbnail_url.clone()),
        media_url: payload
            .full_audio_url
            .clone()
            .or_else(|| payload.full_video_url.clone())
            .or_else(|| payload.pdf_page_url.clone())
            .or_else(|| payload.full_pdf_url.clone()),
        scene_clip_url: payload.scene_clip_url.clone(),
        occurrence_count: 0,
        frame_indices: Vec::new(),
        start_seconds: payload.scene_start_seconds,
        end_seconds: payload.scene_end_seconds,
        page_number: payload.pdf_page_number,
        confidence: 0.0,
    }
}

fn sorted_locations(
    locations: BTreeMap<String, InverseIndexLocation>,
) -> Vec<InverseIndexLocation> {
    let mut locations: Vec<_> = locations.into_values().collect();
    locations.sort_by(|left, right| {
        left.relative_path
            .cmp(&right.relative_path)
            .then_with(|| left.media_id.cmp(&right.media_id))
    });
    locations
}

fn average_confidence(total: f32, count: u32) -> f32 {
    if count == 0 {
        0.0
    } else {
        total / count as f32
    }
}

fn min_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn max_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

pub async fn index_images(State(state): State<Arc<AppState>>) -> Json<IndexResponse> {
    let indexer = ImageIndexer::new(
        state.indexing_settings(),
        state.store.clone(),
        state.embedder.clone(),
    );
    Json(indexer.index_sources().await)
}

pub async fn spawn_index_job(
    State(state): State<Arc<AppState>>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let spec = JobSpec::new(
        format!("index.manual.{}", Uuid::new_v4()),
        "Index media sources",
    )
    .and_then(|spec| spec.with_kind("index.manual"))
    .and_then(|spec| spec.with_metadata("collection", state.settings.qdrant_collection.clone()))
    .map_err(ApiError::from_job)?;
    let jobs = state.jobs.clone();
    let settings = state.indexing_settings();
    let store = state.store.clone();
    let embedder = state.embedder.clone();

    jobs.spawn(spec, move |context| {
        run_index_job(context, settings, store, embedder)
    })
    .map(Json)
    .map_err(ApiError::from_job)
}

pub fn spawn_startup_index_job(state: Arc<AppState>) -> jobs_core::Result<JobSnapshot> {
    let spec = JobSpec::new(
        format!("index.startup.{}", Uuid::new_v4()),
        "Index missing media on startup",
    )?
    .with_kind("index.startup")?
    .with_metadata("collection", state.settings.qdrant_collection.clone())?;
    let jobs = state.jobs.clone();
    let settings = state.indexing_settings();
    let store = state.store.clone();
    let embedder = state.embedder.clone();

    jobs.spawn(spec, move |context| {
        run_index_job(context, settings, store, embedder)
    })
}

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
    fn from_settings(settings: &Settings) -> Self {
        Self {
            image_extensions: settings.image_extensions.iter().cloned().collect(),
            audio_extensions: settings.audio_extensions.iter().cloned().collect(),
            pdf_extensions: settings.pdf_extensions.iter().cloned().collect(),
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

    fn apply_to_settings(&self, settings: &mut Settings) {
        settings.image_extensions = parse_extensions(&self.image_extensions.join(","))
            .expect("validated indexing config contains image extensions");
        settings.audio_extensions = parse_extensions(&self.audio_extensions.join(","))
            .expect("validated indexing config contains audio extensions");
        settings.pdf_extensions = parse_extensions(&self.pdf_extensions.join(","))
            .expect("validated indexing config contains PDF extensions");
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

    fn validated(mut self) -> Result<Self, ApiError> {
        self.image_extensions = normalized_extensions("image_extensions", &self.image_extensions)?;
        self.audio_extensions = normalized_extensions("audio_extensions", &self.audio_extensions)?;
        self.pdf_extensions = normalized_extensions("pdf_extensions", &self.pdf_extensions)?;
        validate_range(
            "face_detection_min_confidence",
            self.face_detection_min_confidence,
            0.0,
            1.0,
        )?;
        validate_range(
            "face_cluster_threshold",
            self.face_cluster_threshold,
            0.0,
            2.0,
        )?;
        validate_min("face_min_cluster_images", self.face_min_cluster_images, 1)?;
        validate_min_usize(
            "face_max_frames_per_media",
            self.face_max_frames_per_media,
            1,
        )?;
        validate_min_usize("gif_sample_frames", self.gif_sample_frames, 1)?;
        validate_min_usize("gif_max_decode_frames", self.gif_max_decode_frames, 1)?;
        validate_min_usize("gif_preview_frames", self.gif_preview_frames, 1)?;
        validate_min(
            "gif_default_frame_delay_ms",
            self.gif_default_frame_delay_ms,
            1,
        )?;
        validate_range("gif_motion_weight", self.gif_motion_weight, 0.0, 1.0)?;
        validate_min("video_frame_stride", self.video_frame_stride, 1)?;
        if let Some(video_max_frames) = self.video_max_frames {
            validate_min("video_max_frames", video_max_frames, 1)?;
        }
        validate_range_u32("pdf_render_dpi", self.pdf_render_dpi, 72, 300)?;
        validate_range_u32("pdf_max_pages", self.pdf_max_pages, 1, 10_000)?;
        validate_range_usize("pdf_summary_pages", self.pdf_summary_pages, 1, 256)?;
        validate_range_usize("ocr_max_frames", self.ocr_max_frames, 1, 64)?;
        Ok(self)
    }
}

pub async fn get_source_config(State(state): State<Arc<AppState>>) -> Json<SourceConfigResponse> {
    Json(source_config_response(&state))
}

pub async fn update_source_config(
    State(state): State<Arc<AppState>>,
    Json(request): Json<UpdateSourceConfigRequest>,
) -> Result<Json<SourceConfigResponse>, ApiError> {
    if let Some(sources) = request.sources {
        let sources = normalize_source_specs(&sources)?;
        write_media_sources_file(&state.settings.media_sources_file, &sources)?;
        state.replace_source_specs(sources);
    }
    if let Some(indexing) = request.indexing {
        state.replace_indexing_config(indexing.validated()?);
    }
    Ok(Json(source_config_response(&state)))
}

fn source_config_response(state: &AppState) -> SourceConfigResponse {
    let settings = state.indexing_settings();
    SourceConfigResponse {
        media_sources_file: settings.media_sources_file.to_string_lossy().to_string(),
        media_sources_seed_file: settings
            .media_sources_seed_file
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        media_sources_writable: media_sources_file_is_writable(&settings.media_sources_file),
        default_source_dir: settings.source_image_dir.to_string_lossy().to_string(),
        sources: settings
            .source_specs()
            .into_iter()
            .map(|spec| source_config_source(spec, &settings))
            .collect(),
        supported_source_types: supported_source_types(),
        indexing: SourceIndexingConfig {
            collection: settings.qdrant_collection,
            image_extensions: settings.image_extensions.into_iter().collect(),
            audio_extensions: settings.audio_extensions.into_iter().collect(),
            pdf_extensions: settings.pdf_extensions.into_iter().collect(),
            video_extensions: video_source_extensions(),
            visual_embedding_enabled: settings.visual_embedding_enabled,
            visual_embedding_model: settings.clip_model_name.clone(),
            visual_embedding_vector_size: settings.visual_embedding_vector_size,
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
        },
    }
}

fn normalized_extensions(name: &str, values: &[String]) -> Result<Vec<String>, ApiError> {
    let normalized = parse_extensions(&values.join(",")).map_err(|error| {
        ApiError::bad_request(format!(
            "{name} must contain at least one extension: {error}"
        ))
    })?;
    Ok(normalized.into_iter().collect())
}

fn validate_range(name: &str, value: f32, min: f32, max: f32) -> Result<(), ApiError> {
    if value.is_finite() && value >= min && value <= max {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "{name} must be between {min} and {max}"
        )))
    }
}

fn validate_range_u32(name: &str, value: u32, min: u32, max: u32) -> Result<(), ApiError> {
    if value >= min && value <= max {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "{name} must be between {min} and {max}"
        )))
    }
}

fn validate_range_usize(name: &str, value: usize, min: usize, max: usize) -> Result<(), ApiError> {
    if value >= min && value <= max {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "{name} must be between {min} and {max}"
        )))
    }
}

fn validate_min(name: &str, value: u32, min: u32) -> Result<(), ApiError> {
    if value >= min {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "{name} must be at least {min}"
        )))
    }
}

fn validate_min_usize(name: &str, value: usize, min: usize) -> Result<(), ApiError> {
    if value >= min {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "{name} must be at least {min}"
        )))
    }
}

fn normalize_source_specs(sources: &[String]) -> Result<Vec<String>, ApiError> {
    let input = sources
        .iter()
        .map(|source| source.trim())
        .filter(|source| !source.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let parsed = parse_media_sources_file(&input).map_err(ApiError::bad_request)?;
    if parsed.is_empty() {
        return Err(ApiError::bad_request(
            "At least one media source must be configured",
        ));
    }
    Ok(parsed)
}

fn write_media_sources_file(path: &std::path::Path, sources: &[String]) -> Result<(), ApiError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ApiError::internal(format!(
                "Could not create media source config directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let mut content = "# Managed by image-similarity-service.\n".to_string();
    content.push_str("# One source per line. Supported now: local paths, file://, local://.\n");
    content.push_str(
        "# Planned source specs can be kept here, but unsupported types will be skipped.\n",
    );
    for source in sources {
        content.push_str(source);
        content.push('\n');
    }
    fs::write(path, content).map_err(|error| {
        ApiError::internal(format!(
            "Could not write media source config file {}: {error}",
            path.display()
        ))
    })
}

fn media_sources_file_is_writable(path: &std::path::Path) -> bool {
    if path.is_file() {
        return fs::OpenOptions::new().append(true).open(path).is_ok();
    }

    let Some(parent) = path.parent() else {
        return false;
    };
    if fs::create_dir_all(parent).is_err() {
        return false;
    }
    let probe = parent.join(format!(
        ".media-sources-writable-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

pub(super) fn source_config_source(spec: String, settings: &Settings) -> SourceConfigSource {
    let kind = source_kind(&spec);
    let (status, detail) = match kind.as_str() {
        "local" => {
            let path = local_source_path(&spec);
            if path.is_dir() {
                ("ready".to_string(), None)
            } else {
                (
                    "unavailable".to_string(),
                    Some(format!("Directory does not exist: {}", path.display())),
                )
            }
        }
        "minio" | "s3" => object_source_config_status(&spec, &kind, settings),
        "video" => (
            "not_implemented".to_string(),
            Some(
                "Video source specs are not implemented; local folders can include video files"
                    .to_string(),
            ),
        ),
        "camera" => (
            "not_implemented".to_string(),
            Some("Camera sources are not implemented in the native Rust service yet".to_string()),
        ),
        _ => (
            "unsupported".to_string(),
            Some(format!("Unsupported media source: {spec}")),
        ),
    };

    SourceConfigSource {
        spec,
        kind,
        status,
        detail,
    }
}

fn object_source_config_status(
    spec: &str,
    kind: &str,
    settings: &Settings,
) -> (String, Option<String>) {
    let Ok(url) = url::Url::parse(spec) else {
        return (
            "unavailable".to_string(),
            Some(format!("Invalid object-store source URI: {spec}")),
        );
    };
    if url.host_str().filter(|bucket| !bucket.is_empty()).is_none() {
        return (
            "unavailable".to_string(),
            Some(format!("Missing bucket in object-store source URI: {spec}")),
        );
    }

    let endpoint = match kind {
        "minio" => settings
            .minio_endpoint
            .clone()
            .or_else(|| settings.s3_endpoint.clone()),
        "s3" => settings
            .s3_endpoint
            .clone()
            .or_else(|| settings.minio_endpoint.clone()),
        _ => None,
    };
    let access_key = match kind {
        "minio" => settings
            .minio_access_key
            .clone()
            .or_else(|| settings.s3_access_key_id.clone()),
        "s3" => settings
            .s3_access_key_id
            .clone()
            .or_else(|| settings.minio_access_key.clone()),
        _ => None,
    };
    let secret_key = match kind {
        "minio" => settings
            .minio_secret_key
            .clone()
            .or_else(|| settings.s3_secret_access_key.clone()),
        "s3" => settings
            .s3_secret_access_key
            .clone()
            .or_else(|| settings.minio_secret_key.clone()),
        _ => None,
    };

    if kind == "minio" && endpoint.is_none() {
        return (
            "unavailable".to_string(),
            Some("MINIO_ENDPOINT or S3_ENDPOINT is required for MinIO sources".to_string()),
        );
    }
    if endpoint.is_some() && (access_key.is_none() || secret_key.is_none()) {
        return (
            "unavailable".to_string(),
            Some(format!(
                "{} object-store credentials are incomplete",
                kind.to_ascii_uppercase()
            )),
        );
    }

    ("ready".to_string(), None)
}

fn source_kind(spec: &str) -> String {
    if let Some((scheme, _)) = spec.split_once(':') {
        if scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        }) {
            return match scheme {
                "file" | "local" => "local".to_string(),
                other => other.to_string(),
            };
        }
    }
    "local".to_string()
}

fn local_source_path(spec: &str) -> std::path::PathBuf {
    match url::Url::parse(spec) {
        Ok(url) if url.scheme() == "file" => {
            url.to_file_path().unwrap_or_else(|_| url.path().into())
        }
        Ok(url) if url.scheme() == "local" => {
            let mut path = String::new();
            if let Some(host) = url.host_str() {
                path.push('/');
                path.push_str(host);
            }
            path.push_str(url.path());
            path.into()
        }
        _ => spec.into(),
    }
}

fn supported_source_types() -> Vec<SupportedSourceType> {
    vec![
        SupportedSourceType {
            kind: "local".to_string(),
            label: "Local folder".to_string(),
            implemented: true,
            example: "/images or local:///images".to_string(),
        },
        SupportedSourceType {
            kind: "minio".to_string(),
            label: "MinIO bucket".to_string(),
            implemented: true,
            example: "minio://bucket/prefix".to_string(),
        },
        SupportedSourceType {
            kind: "s3".to_string(),
            label: "S3 bucket".to_string(),
            implemented: true,
            example: "s3://bucket/prefix".to_string(),
        },
        SupportedSourceType {
            kind: "video".to_string(),
            label: "Video stream".to_string(),
            implemented: false,
            example: "video:///clips/demo.mp4".to_string(),
        },
        SupportedSourceType {
            kind: "camera".to_string(),
            label: "Camera".to_string(),
            implemented: false,
            example: "camera://front-door".to_string(),
        },
    ]
}

fn video_source_extensions() -> Vec<String> {
    [".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<JobSnapshot>>, ApiError> {
    state.jobs.snapshots().map(Json).map_err(ApiError::from_job)
}

pub async fn get_job(
    State(state): State<Arc<AppState>>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let job_id = parse_job_id(job_id)?;
    state
        .jobs
        .snapshot(&job_id)
        .map_err(ApiError::from_job)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("Unknown job `{job_id}`")))
}

pub async fn get_job_events(
    State(state): State<Arc<AppState>>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<Vec<JobEvent>>, ApiError> {
    let job_id = parse_job_id(job_id)?;
    if state
        .jobs
        .snapshot(&job_id)
        .map_err(ApiError::from_job)?
        .is_none()
    {
        return Err(ApiError::not_found(format!("Unknown job `{job_id}`")));
    }
    state
        .jobs
        .events(&job_id)
        .map(Json)
        .map_err(ApiError::from_job)
}

pub async fn cancel_job(
    State(state): State<Arc<AppState>>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let job_id = parse_job_id(job_id)?;
    state
        .jobs
        .request_cancel(&job_id)
        .map_err(ApiError::from_job)?;
    state
        .jobs
        .snapshot(&job_id)
        .map_err(ApiError::from_job)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("Unknown job `{job_id}`")))
}

pub async fn delete_indexed_media_route(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<DeleteIndexResponse>, ApiError> {
    let response =
        delete_indexed_media(&state.indexing_settings(), state.store.as_ref(), &id).await;
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
pub struct UpdateMediaTagsRequest {
    tags: Vec<String>,
}

pub async fn update_indexed_media_tags_route(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(request): Json<UpdateMediaTagsRequest>,
) -> Result<Json<ImagePayload>, ApiError> {
    let tags = normalize_media_tags(request.tags)?;
    let point = state
        .store
        .scroll_media_points_by_filter(Some(&id), None, None)
        .await
        .map_err(ApiError::internal)?
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::not_found(format!("Unknown indexed media `{id}`")))?;
    let payload_value = point
        .payload
        .ok_or_else(|| ApiError::internal(format!("Indexed media `{id}` has no payload")))?;
    let mut payload = serde_json::from_value::<ImagePayload>(payload_value)
        .map_err(|error| ApiError::internal(format!("could not decode media payload: {error}")))?;

    payload.tags = tags;
    state
        .store
        .set_media_payload(&payload)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(payload))
}

pub async fn delete_indexed_sources_route(
    State(state): State<Arc<AppState>>,
    Query(filter): Query<DeleteIndexedSourceFilter>,
) -> Result<Json<DeleteIndexResponse>, ApiError> {
    if filter.source_uri.is_none() && filter.source_item_uri.is_none() {
        return Err(ApiError::bad_request(
            "source_uri or source_item_uri is required",
        ));
    }
    let response =
        delete_indexed_source(&state.indexing_settings(), state.store.as_ref(), filter).await;
    Ok(Json(response))
}

fn normalize_media_tags(tags: Vec<String>) -> Result<Vec<String>, ApiError> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();

    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        if tag.chars().any(char::is_control) {
            return Err(ApiError::bad_request(
                "Tags cannot contain control characters",
            ));
        }
        if tag.chars().count() > MAX_MEDIA_TAG_LENGTH {
            return Err(ApiError::bad_request(format!(
                "Tags must be {MAX_MEDIA_TAG_LENGTH} characters or fewer"
            )));
        }
        if seen.insert(tag.to_lowercase()) {
            normalized.push(tag.to_string());
        }
    }

    if normalized.len() > MAX_MEDIA_TAGS {
        return Err(ApiError::bad_request(format!(
            "Media can have at most {MAX_MEDIA_TAGS} tags"
        )));
    }

    Ok(normalized)
}

#[derive(Debug, Serialize)]
pub struct AudioTranscriptionModelsResponse {
    pub enabled: bool,
    pub configured_model: String,
    pub auto_download: bool,
    pub cache_dir: Option<String>,
    pub models: Vec<AudioTranscriptionModelResponse>,
}

#[derive(Debug, Serialize)]
pub struct AudioTranscriptionModelResponse {
    pub id: String,
    pub cached: bool,
    pub configured: bool,
}

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub models: Vec<ModelRuntimeStatus>,
}

pub async fn get_models(State(state): State<Arc<AppState>>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        models: model_statuses(&state.indexing_settings()),
    })
}

pub async fn audio_transcription_models(
    State(state): State<Arc<AppState>>,
) -> Json<AudioTranscriptionModelsResponse> {
    let store = audio_transcription_model_store(&state.settings);
    let configured_model = state.settings.audio_transcription_model.clone();
    let models = store
        .catalog()
        .models
        .into_iter()
        .map(|status| {
            let id = status.model.id().to_string();
            AudioTranscriptionModelResponse {
                cached: status.cached,
                configured: id.eq_ignore_ascii_case(&configured_model),
                id,
            }
        })
        .collect();

    Json(AudioTranscriptionModelsResponse {
        enabled: state.settings.audio_transcription_enabled,
        configured_model,
        auto_download: state.settings.audio_transcription_auto_download,
        cache_dir: state
            .settings
            .audio_transcription_cache_dir
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        models,
    })
}

#[derive(Debug, Deserialize)]
pub struct AudioTranscriptionModelJobRequest {
    pub model: Option<String>,
}

pub async fn download_model(
    State(state): State<Arc<AppState>>,
    AxumPath(role): AxumPath<String>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let role = role.parse::<ModelRole>().map_err(ApiError::bad_request)?;
    if role == ModelRole::AudioTranscription {
        return spawn_audio_transcription_download(state, request.model).map(Json);
    }

    let settings = state.indexing_settings();
    let spec = crate::workers::media::models::role_spec(role).map_err(ApiError::bad_request)?;
    if let Some(requested) = request.model.as_deref() {
        if requested != spec.name {
            return Err(ApiError::bad_request(format!(
                "Model role `{}` supports configured model `{}`",
                role.as_str(),
                spec.name
            )));
        }
    }
    let spec_for_job = spec.clone();
    let model_name = spec.name.clone();
    let spec = JobSpec::new(
        format!("model.download.{}.{}", role.as_str(), Uuid::new_v4()),
        format!("Download {}", role.label()),
    )
    .and_then(|job_spec| job_spec.with_kind("model.download"))
    .and_then(|job_spec| job_spec.with_metadata("role", role.as_str()))
    .and_then(|job_spec| job_spec.with_metadata("model", model_name))
    .map_err(ApiError::from_job)?;
    state
        .jobs
        .spawn(spec, move |context| {
            context.info(format!("downloading model bundle `{}`", spec_for_job.name))?;
            context.progress(
                JobProgress::new(0, Some(1))?
                    .unit("steps")?
                    .message("downloading model bundle"),
            )?;
            let bundle = download_role_bundle(role, &settings).map_err(job_failed)?;
            context.artifact(
                JobArtifact::new("manifest", format!("model bundle {}", bundle.manifest.name))
                    .kind("model-bundle")
                    .path(bundle.manifest_path()),
            )?;
            context.progress(
                JobProgress::new(1, Some(1))?
                    .unit("steps")?
                    .message("model bundle ready"),
            )?;
            Ok(())
        })
        .map(Json)
        .map_err(ApiError::from_job)
}

pub async fn enable_model(
    State(state): State<Arc<AppState>>,
    AxumPath(role): AxumPath<String>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let role = role.parse::<ModelRole>().map_err(ApiError::bad_request)?;
    if role == ModelRole::AudioTranscription {
        return spawn_audio_transcription_enable(state, request.model).map(Json);
    }

    let settings = state.indexing_settings();
    let status = crate::workers::media::models::model_status(role, &settings);
    if !status.cached {
        return Err(ApiError::bad_request(format!(
            "model `{}` is not cached; download it before enabling",
            status.configured
        )));
    }
    let spec = JobSpec::new(
        format!("model.enable.{}.{}", role.as_str(), Uuid::new_v4()),
        format!("Enable {}", role.label()),
    )
    .and_then(|spec| spec.with_kind("model.enable"))
    .and_then(|spec| spec.with_metadata("role", role.as_str()))
    .and_then(|spec| spec.with_metadata("model", status.configured.clone()))
    .map_err(ApiError::from_job)?;
    state
        .jobs
        .spawn(spec, move |context| {
            context.info(format!("model role `{}` is ready", role.as_str()))?;
            context.metadata("enabled", "true")?;
            context.progress(
                JobProgress::new(1, Some(1))?
                    .unit("steps")?
                    .message("model enable check complete"),
            )?;
            Ok(())
        })
        .map(Json)
        .map_err(ApiError::from_job)
}

pub async fn download_audio_transcription_model(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<JobSnapshot>, ApiError> {
    spawn_audio_transcription_download(state, request.model).map(Json)
}

fn spawn_audio_transcription_download(
    state: Arc<AppState>,
    requested: Option<String>,
) -> Result<JobSnapshot, ApiError> {
    let model = requested_audio_transcription_model(&state.settings, requested.as_deref())?;
    let store = audio_transcription_model_store(&state.settings);
    let spec = model_job_spec("model.download", "Download whisper.cpp model", model)?;
    state
        .jobs
        .spawn(spec, move |context| {
            download_whisper_cpp_model(context, store, model)?;
            Ok(())
        })
        .map_err(ApiError::from_job)
}

pub async fn enable_audio_transcription_model(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<JobSnapshot>, ApiError> {
    spawn_audio_transcription_enable(state, request.model).map(Json)
}

fn spawn_audio_transcription_enable(
    state: Arc<AppState>,
    requested: Option<String>,
) -> Result<JobSnapshot, ApiError> {
    let model = requested_audio_transcription_model(&state.settings, requested.as_deref())?;
    let store = audio_transcription_model_store(&state.settings);
    let settings = state.settings.clone();
    let spec = model_job_spec("model.enable", "Enable whisper.cpp model", model)?;
    state
        .jobs
        .spawn(spec, move |context| {
            enable_whisper_cpp_model(context, &settings, &store, model)?;
            Ok(())
        })
        .map_err(ApiError::from_job)
}

pub async fn search_upload(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
    mut multipart: Multipart,
) -> Result<Json<SearchResponse>, ApiError> {
    let filters = query.search_filters()?;
    let mut uploaded = None;
    let mut upload_kind = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let content_type = field.content_type().unwrap_or_default().to_string();
        let filename = field.file_name().map(ToOwned::to_owned);
        let filename_extension = filename
            .as_deref()
            .and_then(|name| std::path::Path::new(name).extension())
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{}", extension.to_ascii_lowercase()));
        let is_image = content_type.starts_with("image/")
            || filename_extension
                .as_ref()
                .map(|extension| state.settings.image_extensions.contains(extension))
                .unwrap_or(false);
        let is_video = is_video_content_type(&content_type)
            || filename_extension
                .as_deref()
                .map(is_video_extension)
                .unwrap_or(false);
        let is_audio = is_audio_content_type(&content_type)
            || filename_extension
                .as_deref()
                .map(is_audio_extension)
                .unwrap_or(false);
        let is_pdf = is_pdf_content_type(&content_type)
            || filename_extension
                .as_deref()
                .map(is_pdf_extension)
                .unwrap_or(false);
        if !is_image && !is_video && !is_audio && !is_pdf {
            return Err(ApiError::bad_request(
                "Upload must be an image, video, audio, or PDF file",
            ));
        }
        let raw = field
            .bytes()
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        uploaded = Some(raw);
        upload_kind = Some(UploadedFileKind {
            is_video,
            is_audio,
            is_pdf,
            filename,
        });
        break;
    }

    let raw = uploaded.ok_or_else(|| {
        ApiError::bad_request("Upload must be an image, video, audio, or PDF file")
    })?;
    let upload_kind = upload_kind.ok_or_else(|| {
        ApiError::bad_request("Upload must be an image, video, audio, or PDF file")
    })?;
    let max_bytes = state.settings.max_upload_mb as usize * 1024 * 1024;
    if raw.len() > max_bytes {
        return Err(ApiError::payload_too_large(format!(
            "Upload is larger than {} MB",
            state.settings.max_upload_mb
        )));
    }

    if upload_kind.is_video {
        return search_video_upload(
            state,
            query.limit,
            query.ocr_text.as_deref(),
            filters,
            &raw,
            upload_kind.filename.as_deref(),
        )
        .await
        .map(Json);
    }

    if upload_kind.is_audio {
        return search_audio_upload(
            state,
            query.limit,
            query.ocr_text.as_deref(),
            filters,
            &raw,
            upload_kind.filename.as_deref(),
        )
        .await
        .map(Json);
    }

    if upload_kind.is_pdf {
        return search_pdf_upload(
            state,
            query.limit,
            query.ocr_text.as_deref(),
            filters,
            &raw,
            upload_kind.filename.as_deref(),
        )
        .await
        .map(Json);
    }

    let media = load_media_bytes(&raw, &state.settings)
        .map_err(|_| ApiError::bad_request("Could not decode image"))?;
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    service
        .search_media_filtered(&media, query.limit, query.ocr_text.as_deref(), filters)
        .await
        .map(Json)
        .map_err(search_error)
}

impl SearchQuery {
    fn search_filters(&self) -> Result<SearchFilters, ApiError> {
        Ok(SearchFilters {
            source_type: normalized_filter(self.source_type.as_deref())
                .filter(|value| value != "all"),
            media_kind: normalized_filter(self.media_kind.as_deref())
                .filter(|value| value != "all")
                .map(validate_media_kind)
                .transpose()?,
            name_query: normalized_filter(self.name_query.as_deref()),
            camera_query: normalized_filter(self.camera_query.as_deref()),
            keyword_query: normalized_filter(self.keyword_query.as_deref()),
            has_gps: parse_has_gps(self.has_gps.as_deref())?,
            near_duplicate: parse_near_duplicate(self.near_duplicate.as_deref())?,
            orientation: parse_orientation(self.orientation.as_deref())?,
            min_width: self.min_width,
            max_width: self.max_width,
            min_height: self.min_height,
            max_height: self.max_height,
            min_size_bytes: self.min_size_bytes,
            max_size_bytes: self.max_size_bytes,
            modified_from: validate_optional_seconds("modified_from", self.modified_from)?,
            modified_to: validate_optional_seconds("modified_to", self.modified_to)?,
            captured_from: validate_optional_seconds("captured_from", self.captured_from)?,
            captured_to: validate_optional_seconds("captured_to", self.captured_to)?,
            person_id: normalized_filter(self.person_id.as_deref()),
        })
    }
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn validate_media_kind(value: String) -> Result<String, ApiError> {
    match value.as_str() {
        "static_image" | "animated_gif" | "video_scene" | "audio" | "pdf_page"
        | "pdf_document" => Ok(value),
        _ => Err(ApiError::bad_request(
            "media_kind must be one of all, static_image, animated_gif, video_scene, audio, pdf_page, pdf_document",
        )),
    }
}

fn parse_has_gps(value: Option<&str>) -> Result<Option<bool>, ApiError> {
    match normalized_filter(value).as_deref() {
        None | Some("all") => Ok(None),
        Some("yes") => Ok(Some(true)),
        Some("no") => Ok(Some(false)),
        Some(_) => Err(ApiError::bad_request("has_gps must be one of all, yes, no")),
    }
}

fn parse_near_duplicate(value: Option<&str>) -> Result<Option<NearDuplicateFilter>, ApiError> {
    match normalized_filter(value).as_deref() {
        None | Some("all") => Ok(None),
        Some("only") => Ok(Some(NearDuplicateFilter::Only)),
        Some("exclude") => Ok(Some(NearDuplicateFilter::Exclude)),
        Some(_) => Err(ApiError::bad_request(
            "near_duplicate must be one of all, only, exclude",
        )),
    }
}

fn parse_orientation(value: Option<&str>) -> Result<Option<OrientationFilter>, ApiError> {
    match normalized_filter(value).as_deref() {
        None | Some("all") => Ok(None),
        Some("landscape") => Ok(Some(OrientationFilter::Landscape)),
        Some("portrait") => Ok(Some(OrientationFilter::Portrait)),
        Some("square") => Ok(Some(OrientationFilter::Square)),
        Some(_) => Err(ApiError::bad_request(
            "orientation must be one of all, landscape, portrait, square",
        )),
    }
}

fn validate_optional_seconds(name: &str, value: Option<f64>) -> Result<Option<f64>, ApiError> {
    match value {
        Some(value) if !value.is_finite() || value < 0.0 => Err(ApiError::bad_request(format!(
            "{name} must be a non-negative Unix timestamp in seconds"
        ))),
        _ => Ok(value),
    }
}

struct UploadedFileKind {
    is_video: bool,
    is_audio: bool,
    is_pdf: bool,
    filename: Option<String>,
}

async fn search_pdf_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    filters: SearchFilters,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let upload_path = pdf_upload_path(&state.settings.upload_dir, filename);
    write_pdf_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let pdf = match decode_pdf(&upload_path, &state.settings) {
        Ok(pdf) => pdf,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process PDF: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    let mut scene_responses = Vec::new();
    let mut flattened = Vec::new();

    for page in &pdf.pages {
        let mut response = service
            .search_media_filtered(&page.media, limit, ocr_text, filters.clone())
            .await
            .map_err(search_error)?;
        for result in &mut response.results {
            result.query_scene_index = Some(page.page_index);
        }
        flattened.extend(response.results.clone());
        scene_responses.push(SearchSceneResponse {
            scene_index: page.page_index,
            scene_kind: "pdf_page".to_string(),
            start_frame: page.page_number as u64,
            end_frame: page.page_number as u64,
            start_seconds: 0.0,
            end_seconds: 0.0,
            clip_url: None,
            page_index: Some(page.page_index),
            page_number: Some(page.page_number),
            page_label: Some(format!("Page {}", page.page_number)),
            speaker_id: None,
            speaker_label: None,
            query_phash: response.query_phash,
            count: response.count,
            results: response.results,
        });
    }

    let results = deduplicate_flat_results(flattened);
    Ok(SearchResponse {
        query_phash: scene_responses
            .first()
            .map(|scene| scene.query_phash.clone())
            .unwrap_or_default(),
        count: results.len(),
        results,
        query_media_kind: "pdf".to_string(),
        scenes: scene_responses,
        query_audio_analysis: None,
        query_ocr_text: normalize_ocr_query(ocr_text),
    })
}

async fn search_audio_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    filters: SearchFilters,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let upload_path = audio_upload_path(&state.settings.upload_dir, filename);
    write_audio_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let segments = match decode_audio_segments(&upload_path, &state.settings) {
        Ok(segments) => segments,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process audio: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    let mut scene_responses = Vec::new();
    let mut flattened = Vec::new();

    for segment in &segments {
        let mut response = service
            .search_media_filtered(&segment.media, limit, ocr_text, filters.clone())
            .await
            .map_err(search_error)?;
        for result in &mut response.results {
            result.query_scene_index = Some(segment.scene_index);
        }
        flattened.extend(response.results.clone());
        scene_responses.push(SearchSceneResponse {
            scene_index: segment.scene_index,
            scene_kind: "audio_bit".to_string(),
            start_frame: (segment.start_seconds * 1000.0).round() as u64,
            end_frame: (segment.end_seconds * 1000.0).round() as u64,
            start_seconds: segment.start_seconds,
            end_seconds: segment.end_seconds,
            clip_url: None,
            page_index: None,
            page_number: None,
            page_label: None,
            speaker_id: segment.speaker_id.clone(),
            speaker_label: segment.speaker_label.clone(),
            query_phash: response.query_phash,
            count: response.count,
            results: response.results,
        });
    }

    let results = deduplicate_flat_results(flattened);
    Ok(SearchResponse {
        query_phash: scene_responses
            .first()
            .map(|scene| scene.query_phash.clone())
            .unwrap_or_default(),
        count: results.len(),
        results,
        query_media_kind: "audio".to_string(),
        scenes: scene_responses,
        query_audio_analysis: segments
            .first()
            .and_then(|segment| segment.media.audio_analysis.clone()),
        query_ocr_text: normalize_ocr_query(ocr_text),
    })
}

async fn search_video_upload(
    state: Arc<AppState>,
    limit: Option<u32>,
    ocr_text: Option<&str>,
    filters: SearchFilters,
    raw: &[u8],
    filename: Option<&str>,
) -> Result<SearchResponse, ApiError> {
    let upload_path = video_upload_path(&state.settings.upload_dir, filename);
    write_video_upload(&upload_path, raw).map_err(ApiError::internal)?;
    let scenes = match decode_video_scenes(&upload_path, &state.settings) {
        Ok(scenes) => scenes,
        Err(error) => {
            let _ = std::fs::remove_file(&upload_path);
            return Err(ApiError::bad_request(format!(
                "Could not process video: {error}"
            )));
        }
    };
    let _ = std::fs::remove_file(&upload_path);
    let service = ImageSearchService::new(
        state.settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    let mut scene_responses = Vec::new();
    let mut flattened = Vec::new();

    for scene in &scenes {
        let mut response = service
            .search_media_filtered(&scene.media, limit, ocr_text, filters.clone())
            .await
            .map_err(search_error)?;
        for result in &mut response.results {
            result.query_scene_index = Some(scene.scene_index);
        }
        flattened.extend(response.results.clone());
        scene_responses.push(SearchSceneResponse {
            scene_index: scene.scene_index,
            scene_kind: "scene".to_string(),
            start_frame: scene.start.frame_index,
            end_frame: scene.end.frame_index,
            start_seconds: scene.start.timestamp.seconds(),
            end_seconds: scene.end.timestamp.seconds(),
            clip_url: scene.clip_url.clone(),
            page_index: None,
            page_number: None,
            page_label: None,
            speaker_id: None,
            speaker_label: None,
            query_phash: response.query_phash,
            count: response.count,
            results: response.results,
        });
    }

    let results = deduplicate_flat_results(flattened);
    Ok(SearchResponse {
        query_phash: scene_responses
            .first()
            .map(|scene| scene.query_phash.clone())
            .unwrap_or_default(),
        count: results.len(),
        results,
        query_media_kind: "video".to_string(),
        scenes: scene_responses,
        query_audio_analysis: None,
        query_ocr_text: normalize_ocr_query(ocr_text),
    })
}

fn deduplicate_flat_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut by_image_id = BTreeMap::<String, SearchResult>::new();
    for result in results {
        by_image_id
            .entry(result.image.id.clone())
            .and_modify(|existing| {
                if result.vector_score > existing.vector_score {
                    *existing = result.clone();
                }
            })
            .or_insert(result);
    }
    let mut deduped = by_image_id.into_values().collect::<Vec<_>>();
    deduped.sort_by(|left, right| right.vector_score.total_cmp(&left.vector_score));
    deduped
}

fn parse_job_id(value: String) -> Result<JobId, ApiError> {
    JobId::new(value).map_err(|error| ApiError::bad_request(error.to_string()))
}

fn requested_audio_transcription_model(
    settings: &Settings,
    requested: Option<&str>,
) -> Result<WhisperCppModel, ApiError> {
    parse_whisper_cpp_model(requested.unwrap_or(&settings.audio_transcription_model))
        .map_err(ApiError::bad_request)
}

fn model_job_spec(kind: &str, name: &str, model: WhisperCppModel) -> Result<JobSpec, ApiError> {
    JobSpec::new(
        format!("{kind}.whisper.{}.{}", model.id(), Uuid::new_v4()),
        name,
    )
    .and_then(|spec| spec.with_kind(kind))
    .and_then(|spec| spec.with_metadata("provider", "whisper.cpp"))
    .and_then(|spec| spec.with_metadata("model", model.id()))
    .map_err(ApiError::from_job)
}

fn run_index_job(
    context: JobContext,
    settings: Settings,
    store: Arc<dyn MediaVectorStore>,
    embedder: Arc<dyn VisualEmbeddingBackend>,
) -> jobs_core::Result<()> {
    context.info("checking indexed media sources")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(job_failed)?;
    let indexer = ImageIndexer::new(settings, store, embedder);
    let response = runtime.block_on(indexer.index_missing_sources(Some(&context)));

    for error in &response.errors {
        context.warn(error.clone())?;
    }
    if context.is_cancelled() {
        return Err(JobError::Cancelled);
    }
    if response.failed > 0 {
        return Err(JobError::Failed(format!(
            "indexing finished with {} failed source file(s)",
            response.failed
        )));
    }
    Ok(())
}

fn download_whisper_cpp_model(
    context: JobContext,
    store: WhisperCppModelStore,
    model: WhisperCppModel,
) -> jobs_core::Result<()> {
    context.info(format!("checking whisper.cpp model `{}`", model.id()))?;
    context.progress(
        JobProgress::new(0, Some(1))?
            .unit("steps")?
            .message("checking model cache"),
    )?;
    context.check_cancelled()?;

    let model_path = store.model_path(model);
    if model_path.is_file() {
        context.info(format!(
            "whisper.cpp model `{}` is already cached",
            model.id()
        ))?;
        context.progress(
            JobProgress::new(1, Some(1))?
                .unit("steps")?
                .message("model already cached"),
        )?;
        context.artifact(
            JobArtifact::new("model", format!("whisper.cpp model {}", model.id()))
                .kind("model")
                .path(model_path),
        )?;
        return Ok(());
    }

    fs::create_dir_all(store.models_dir()).map_err(job_failed)?;
    let temp_path = model_path.with_extension(format!("{}.part", context.id()));
    if temp_path.exists() {
        fs::remove_file(&temp_path).map_err(job_failed)?;
    }

    context.info(format!("downloading whisper.cpp model `{}`", model.id()))?;
    let mut response = reqwest::blocking::get(model.download_url())
        .and_then(|response| response.error_for_status())
        .map_err(job_failed)?;
    let total_bytes = response.content_length().filter(|total| *total > 0);
    let mut output = File::create(&temp_path).map_err(job_failed)?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        context.check_cancelled()?;
        let read = response.read(&mut buffer).map_err(job_failed)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read]).map_err(job_failed)?;
        hasher.update(&buffer[..read]);
        downloaded += read as u64;
        context.progress(bytes_progress(
            downloaded,
            total_bytes,
            format!("downloaded {} bytes", downloaded),
        )?)?;
    }
    output.flush().map_err(job_failed)?;

    let checksum = format!("{:x}", hasher.finalize());
    if checksum != model.checksum_sha256() {
        let _ = fs::remove_file(&temp_path);
        return Err(JobError::Failed(format!(
            "downloaded model `{}` failed checksum verification",
            model.id()
        )));
    }

    fs::rename(&temp_path, &model_path).map_err(job_failed)?;
    context.info(format!("downloaded whisper.cpp model `{}`", model.id()))?;
    context.artifact(
        JobArtifact::new("model", format!("whisper.cpp model {}", model.id()))
            .kind("model")
            .path(model_path),
    )?;
    Ok(())
}

fn enable_whisper_cpp_model(
    context: JobContext,
    settings: &Settings,
    store: &WhisperCppModelStore,
    model: WhisperCppModel,
) -> jobs_core::Result<()> {
    context.info(format!("checking whisper.cpp model `{}`", model.id()))?;
    context.progress(
        JobProgress::new(0, Some(1))?
            .unit("steps")?
            .message("checking model settings"),
    )?;
    context.check_cancelled()?;

    if !whisper_model_is_cached(store, model) {
        return Err(JobError::Failed(format!(
            "whisper.cpp model `{}` is not cached; download it before enabling",
            model.id()
        )));
    }

    if !settings.audio_transcription_enabled {
        context
            .warn("AUDIO_TRANSCRIPTION_ENABLED is false; set it to true to use transcription")?;
    }
    if !settings
        .audio_transcription_model
        .eq_ignore_ascii_case(model.id())
    {
        context.warn(format!(
            "AUDIO_TRANSCRIPTION_MODEL is `{}`; set it to `{}` to make this the active model",
            settings.audio_transcription_model,
            model.id()
        ))?;
    }
    context.metadata("enabled", settings.audio_transcription_enabled.to_string())?;
    context.metadata(
        "configured_model",
        settings.audio_transcription_model.clone(),
    )?;
    context.progress(
        JobProgress::new(1, Some(1))?
            .unit("steps")?
            .message("model enable check complete"),
    )?;
    context.info(format!("finished enable check for `{}`", model.id()))?;
    Ok(())
}

fn bytes_progress(
    completed: u64,
    total: Option<u64>,
    message: impl Into<String>,
) -> jobs_core::Result<JobProgress> {
    let total = total.filter(|total| *total >= completed && *total > 0);
    let progress = JobProgress::new(completed, total)?
        .unit("bytes")?
        .message(message);
    progress.validate()?;
    Ok(progress)
}

fn job_failed(error: impl std::fmt::Display) -> JobError {
    JobError::Failed(error.to_string())
}

fn search_error(error: String) -> ApiError {
    if error.contains("model is not available") || error.contains("model unavailable") {
        ApiError::service_unavailable(error)
    } else {
        ApiError::internal(error)
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::normalize_media_tags;

    #[test]
    fn media_tags_are_trimmed_and_deduplicated() {
        assert_eq!(
            normalize_media_tags(vec![
                " travel ".to_string(),
                "Travel".to_string(),
                "".to_string(),
                "archive".to_string(),
            ])
            .unwrap(),
            vec!["travel", "archive"]
        );
    }

    #[test]
    fn media_tags_reject_control_characters() {
        let error = normalize_media_tags(vec!["bad\ntag".to_string()]).unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
    }
}
