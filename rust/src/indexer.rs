use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use jobs_core::{JobContext, JobProgress};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::audio::{decode_source_audio_segments, expose_source_audio, SourceAudioSegment};
use crate::config::Settings;
use crate::faces::{analyze_faces_for_media, FaceAnalysis};
use crate::hashing::phash_image;
use crate::image_io::{dimensions, image_id_for_uri};
use crate::media::{DecodedMedia, MediaKind};
use crate::models::{ImagePayload, IndexResponse};
use crate::ocr::extract_media_ocr;
use crate::qdrant::QdrantImageStore;
use crate::sources::{build_image_sources, SourceImage, SourceUnavailable};
use crate::thumbnails::{ensure_animated_thumbnail, ensure_thumbnail};
use crate::video::{decode_source_video_scenes, SourceVideoScene};
use crate::visual_embedding::VisualEmbeddingBackend;

#[derive(Clone)]
pub struct ImageIndexer {
    settings: Settings,
    store: QdrantImageStore,
    embedder: Arc<dyn VisualEmbeddingBackend>,
}

impl ImageIndexer {
    pub fn new(
        settings: Settings,
        store: QdrantImageStore,
        embedder: Arc<dyn VisualEmbeddingBackend>,
    ) -> Self {
        Self {
            settings,
            store,
            embedder,
        }
    }

    pub async fn index_sources(&self) -> IndexResponse {
        self.index_missing_sources(None).await
    }

    pub async fn index_missing_sources(&self, context: Option<&JobContext>) -> IndexResponse {
        let plan = match self.plan_sources().await {
            Ok(plan) => plan,
            Err(error) => {
                return IndexResponse {
                    indexed: 0,
                    skipped: 0,
                    failed: 1,
                    pruned: 0,
                    collection: self.settings.qdrant_collection.clone(),
                    source_dir: self.settings.source_image_dir.to_string_lossy().to_string(),
                    sources: build_image_sources(&self.settings)
                        .iter()
                        .map(|source| source.uri())
                        .collect(),
                    errors: vec![format!("Could not prepare indexing plan: {error}")],
                };
            }
        };

        if let Some(context) = context {
            let _ = context.info(format!(
                "{} source file(s) already indexed; {} source file(s) need indexing",
                plan.already_indexed,
                plan.pending.len()
            ));
            let _ = context.metadata("already_indexed", plan.already_indexed.to_string());
            let _ = context.metadata("needs_indexing", plan.pending.len().to_string());
        }

        let mut indexed = 0;
        let mut pruned = 0;
        let skipped = plan.skipped + plan.already_indexed;
        let mut failed = 0;
        let mut errors = plan.errors;
        if !plan.prune_point_ids.is_empty() {
            let prune_count = plan.prune_point_ids.len();
            if let Some(context) = context {
                let _ = context.info(format!(
                    "pruning {prune_count} stale Qdrant record(s) before indexing"
                ));
            }
            match self.store.delete_points(&plan.prune_point_ids).await {
                Ok(()) => {
                    pruned += prune_count;
                    if let Some(context) = context {
                        let _ = context.metadata("pruned", pruned.to_string());
                    }
                }
                Err(error) => {
                    failed += 1;
                    errors.push(format!("Could not prune stale Qdrant records: {error}"));
                    if let Some(context) = context {
                        let _ =
                            context.warn(format!("could not prune stale Qdrant records: {error}"));
                    }
                }
            }
        }
        let total = plan.pending.len() as u64;
        if let Some(context) = context {
            if let Ok(progress) = index_progress(0, total, "indexing pending sources") {
                let _ = context.progress(progress);
            }
        }
        for (index, pending_source) in plan.pending.iter().enumerate() {
            let source_image = &pending_source.source_image;
            if let Some(context) = context {
                if let Err(error) = context.check_cancelled() {
                    errors.truncate(50);
                    let _ = context.metadata("indexed", indexed.to_string());
                    let _ = context.metadata("failed", failed.to_string());
                    let _ = context.metadata("skipped", skipped.to_string());
                    let _ = context.metadata("pruned", pruned.to_string());
                    let _ = context.warn(format!(
                        "indexing cancelled before {}",
                        source_image.display_path
                    ));
                    return IndexResponse {
                        indexed,
                        skipped,
                        failed,
                        pruned,
                        collection: self.settings.qdrant_collection.clone(),
                        source_dir: self.settings.source_image_dir.to_string_lossy().to_string(),
                        sources: plan.source_uris,
                        errors: {
                            errors.push(error.to_string());
                            errors
                        },
                    };
                }
                let _ = context.info(format!("indexing {}", source_image.display_path));
            }

            match self.index_one(source_image).await {
                Ok(outcome) => {
                    indexed += outcome.indexed;
                    let stale_point_ids = pending_source
                        .indexed_point_ids
                        .iter()
                        .filter(|id| !outcome.point_ids.contains(*id))
                        .cloned()
                        .collect::<Vec<_>>();
                    if !stale_point_ids.is_empty() {
                        match self.store.delete_points(&stale_point_ids).await {
                            Ok(()) => {
                                pruned += stale_point_ids.len();
                                if let Some(context) = context {
                                    let _ = context.info(format!(
                                        "pruned {} stale record(s) for {}",
                                        stale_point_ids.len(),
                                        source_image.display_path
                                    ));
                                }
                            }
                            Err(error) => {
                                failed += 1;
                                errors.push(format!(
                                    "{}: could not prune stale Qdrant records: {error}",
                                    source_image.display_path
                                ));
                                if let Some(context) = context {
                                    let _ = context.warn(format!(
                                        "{}: could not prune stale Qdrant records: {error}",
                                        source_image.display_path
                                    ));
                                }
                            }
                        }
                    }
                }
                Err(error) => {
                    failed += 1;
                    errors.push(format!("{}: {error}", source_image.display_path));
                    if let Some(context) = context {
                        let _ = context.warn(format!("{}: {error}", source_image.display_path));
                    }
                }
            }

            if let Some(context) = context {
                let completed = index as u64 + 1;
                if let Ok(progress) = index_progress(
                    completed,
                    total,
                    format!("indexed {completed}/{total} pending source files"),
                ) {
                    let _ = context.progress(progress);
                }
            }
        }

        if let Some(context) = context {
            let _ = context.metadata("indexed", indexed.to_string());
            let _ = context.metadata("failed", failed.to_string());
            let _ = context.metadata("skipped", skipped.to_string());
            let _ = context.metadata("pruned", pruned.to_string());
            let _ = context.info(format!(
                "indexing complete: {indexed} media item(s), {skipped} skipped, {pruned} pruned, {failed} failed"
            ));
        }

        errors.truncate(50);
        IndexResponse {
            indexed,
            skipped,
            failed,
            pruned,
            collection: self.settings.qdrant_collection.clone(),
            source_dir: self.settings.source_image_dir.to_string_lossy().to_string(),
            sources: plan.source_uris,
            errors,
        }
    }

    async fn plan_sources(&self) -> Result<SourceIndexPlan, String> {
        let sources = build_image_sources(&self.settings);
        let source_uris = sources
            .iter()
            .map(|source| source.uri())
            .collect::<Vec<_>>();

        self.store.ensure_collection().await?;
        let indexing_profile = indexing_profile(&self.settings);
        let indexed_sources = self.indexed_source_records().await?;

        let mut pending = Vec::new();
        let mut already_indexed = 0;
        let mut skipped = 0;
        let mut errors = Vec::new();
        let mut scanned_source_items = BTreeSet::new();
        let mut prune_point_ids = Vec::new();
        for source in &sources {
            match source.iter_images() {
                Ok(images) => {
                    for source_image in images {
                        scanned_source_items.insert(source_image.item_uri.clone());
                        let indexed_records = indexed_sources
                            .get(&source_image.item_uri)
                            .cloned()
                            .unwrap_or_default();
                        if source_is_current(&indexed_records, &source_image, &indexing_profile) {
                            already_indexed += 1;
                            prune_point_ids.extend(
                                indexed_records
                                    .iter()
                                    .filter(|record| {
                                        !record_is_current(record, &source_image, &indexing_profile)
                                    })
                                    .map(|record| record.point_id.clone()),
                            );
                        } else {
                            pending.push(PendingSource {
                                source_image,
                                indexed_point_ids: indexed_records
                                    .iter()
                                    .map(|record| record.point_id.clone())
                                    .collect(),
                            });
                        }
                    }
                }
                Err(SourceUnavailable(error)) => {
                    skipped += 1;
                    errors.push(error);
                }
            }
        }

        prune_point_ids.extend(
            indexed_sources
                .iter()
                .filter(|(source_item_uri, _)| !scanned_source_items.contains(*source_item_uri))
                .flat_map(|(_, records)| records.iter().map(|record| record.point_id.clone())),
        );
        prune_point_ids.sort();
        prune_point_ids.dedup();

        errors.truncate(50);
        Ok(SourceIndexPlan {
            source_uris,
            pending,
            already_indexed,
            skipped,
            prune_point_ids,
            errors,
        })
    }

    async fn indexed_source_records(
        &self,
    ) -> Result<BTreeMap<String, Vec<IndexedSourceRecord>>, String> {
        let mut records = BTreeMap::<String, Vec<IndexedSourceRecord>>::new();
        for point in self.store.scroll_media_points().await? {
            let Some(payload) = point.payload else {
                continue;
            };
            let Ok(payload) = serde_json::from_value::<ImagePayload>(payload) else {
                continue;
            };
            let Some(source_item_uri) = payload
                .source_item_uri
                .clone()
                .or_else(|| legacy_source_item_uri(&payload))
            else {
                continue;
            };
            records
                .entry(source_item_uri)
                .or_default()
                .push(IndexedSourceRecord {
                    point_id: point.id,
                    size_bytes: payload.size_bytes,
                    modified_at: payload.modified_at,
                    indexing_profile: payload.indexing_profile.clone(),
                    analysis_complete: payload_analysis_complete(&payload, &self.settings),
                });
        }
        Ok(records)
    }

    async fn index_one(&self, source_image: &SourceImage) -> Result<IndexOneOutcome, String> {
        if source_image.is_video() {
            return self.index_video(source_image).await;
        }
        if source_image.is_audio() {
            return self.index_audio(source_image).await;
        }

        let media = source_image.load_media(&self.settings)?;
        let media_id = image_id_for_uri(&source_image.id_base);
        let face_analysis = analyze_faces_for_media(
            &self.settings,
            &self.store,
            &media,
            &media_id,
            Some(source_image.item_uri.clone()),
        )
        .await;
        let payload = self.build_payload(source_image, &media, None, None, &face_analysis)?;
        let vector = self
            .embedder
            .embed_media(&media.sampled_frames, self.settings.gif_motion_weight)?;
        self.store.upsert_media(&payload, vector).await?;
        Ok(IndexOneOutcome::single(payload.id))
    }

    async fn index_video(&self, source_image: &SourceImage) -> Result<IndexOneOutcome, String> {
        let path = source_image
            .local_path()
            .ok_or_else(|| "Video source does not have a local path".to_string())?;
        let scenes = decode_source_video_scenes(path, &source_image.id_base, &self.settings)?;
        let mut outcome = IndexOneOutcome::default();
        for scene in &scenes {
            let id_base = format!("{}#scene={}", source_image.id_base, scene.scene_index + 1);
            let media_id = image_id_for_uri(&id_base);
            let face_analysis = analyze_faces_for_media(
                &self.settings,
                &self.store,
                &scene.media,
                &media_id,
                Some(source_image.item_uri.clone()),
            )
            .await;
            let payload = self.build_payload(
                source_image,
                &scene.media,
                Some(scene),
                None,
                &face_analysis,
            )?;
            let vector = self
                .embedder
                .embed_media(&scene.media.sampled_frames, self.settings.gif_motion_weight)?;
            let point_id = payload.id.clone();
            self.store.upsert_media(&payload, vector).await?;
            outcome.insert(point_id);
        }
        Ok(outcome)
    }

    async fn index_audio(&self, source_image: &SourceImage) -> Result<IndexOneOutcome, String> {
        let path = source_image
            .local_path()
            .ok_or_else(|| "Audio source does not have a local path".to_string())?;
        let segments = decode_source_audio_segments(path, &source_image.id_base, &self.settings)?;
        let mut outcome = IndexOneOutcome::default();
        for segment in &segments {
            let payload = self.build_payload(
                source_image,
                &segment.media,
                None,
                Some(segment),
                &FaceAnalysis::default(),
            )?;
            let vector = self.embedder.embed_media(
                &segment.media.sampled_frames,
                self.settings.gif_motion_weight,
            )?;
            let point_id = payload.id.clone();
            self.store.upsert_media(&payload, vector).await?;
            outcome.insert(point_id);
        }
        Ok(outcome)
    }

    fn build_payload(
        &self,
        source_image: &SourceImage,
        media: &DecodedMedia,
        video_scene: Option<&SourceVideoScene>,
        audio_segment: Option<&SourceAudioSegment>,
        face_analysis: &FaceAnalysis,
    ) -> Result<ImagePayload, String> {
        let id_base = if let Some(scene) = video_scene {
            format!("{}#scene={}", source_image.id_base, scene.scene_index + 1)
        } else if let Some(segment) = audio_segment {
            format!(
                "{}#audio-bit={}",
                source_image.id_base,
                segment.scene_index + 1
            )
        } else {
            source_image.id_base.clone()
        };
        let image_id = image_id_for_uri(&id_base);
        let full_audio_url = if let Some(segment) = audio_segment {
            segment.full_audio_url.clone()
        } else if media.kind == MediaKind::Audio {
            source_image
                .local_path()
                .and_then(|path| expose_source_audio(path, &image_id, &self.settings).ok())
                .flatten()
        } else {
            None
        };
        let thumbnail_url = ensure_thumbnail(
            &media.poster,
            &self.settings.thumbnail_dir,
            &image_id,
            (320, 320),
        )?;
        let animated_thumbnail_url = if media.kind == MediaKind::AnimatedGif {
            Some(ensure_animated_thumbnail(
                &media.preview_frames,
                &self.settings.thumbnail_dir,
                &image_id,
                (320, 320),
            )?)
        } else {
            None
        };
        let (width, height) = dimensions(&media.poster);
        let ocr_analysis = extract_media_ocr(media, &self.settings).unwrap_or_else(|error| {
            tracing::warn!(%error, "OCR extraction failed");
            Default::default()
        });
        let relative_path = if let Some(scene) = video_scene {
            format!(
                "{}#scene-{:03}",
                source_image.relative_path,
                scene.scene_index + 1
            )
        } else if let Some(segment) = audio_segment {
            format!(
                "{}#audio-bit-{:03}",
                source_image.relative_path,
                segment.scene_index + 1
            )
        } else {
            source_image.relative_path.clone()
        };
        let filename = if let Some(scene) = video_scene {
            format!(
                "{} scene {:03}",
                source_image.filename,
                scene.scene_index + 1
            )
        } else if let Some(segment) = audio_segment {
            format!(
                "{} bit {:03}",
                source_image.filename,
                segment.scene_index + 1
            )
        } else {
            source_image.filename.clone()
        };
        let path = if let Some(scene) = video_scene {
            format!(
                "{}#t={:.3},{:.3}",
                source_image.display_path,
                scene.start.timestamp.seconds(),
                scene.end.timestamp.seconds()
            )
        } else if let Some(segment) = audio_segment {
            format!(
                "{}#t={:.3},{:.3}",
                source_image.display_path, segment.start_seconds, segment.end_seconds
            )
        } else {
            source_image.display_path.clone()
        };
        Ok(ImagePayload {
            id: image_id,
            path,
            relative_path,
            filename,
            width,
            height,
            size_bytes: source_image.size_bytes,
            modified_at: source_image.modified_at,
            phash: phash_image(&media.poster),
            thumbnail_url: Some(thumbnail_url),
            animated_thumbnail_url,
            media_kind: media.kind.as_str().to_string(),
            frame_count: media.frame_count,
            duration_ms: media.duration_ms,
            full_video_url: video_scene.and_then(|scene| scene.full_video_url.clone()),
            full_audio_url,
            audio_analysis: media.audio_analysis.clone(),
            ocr_text: ocr_analysis.text,
            ocr_frames: ocr_analysis.frames,
            visual_embedding_model: Some(self.embedder.model_name().to_string()),
            faces: face_analysis.faces.clone(),
            people: face_analysis.person_clusters.clone(),
            scene_clip_url: video_scene.and_then(|scene| scene.clip_url.clone()),
            scene_index: video_scene
                .map(|scene| scene.scene_index)
                .or_else(|| audio_segment.map(|segment| segment.scene_index)),
            scene_start_frame: video_scene.map(|scene| scene.start.frame_index),
            scene_end_frame: video_scene.map(|scene| scene.end.frame_index),
            scene_start_seconds: video_scene
                .map(|scene| scene.start.timestamp.seconds())
                .or_else(|| audio_segment.map(|segment| segment.start_seconds)),
            scene_end_seconds: video_scene
                .map(|scene| scene.end.timestamp.seconds())
                .or_else(|| audio_segment.map(|segment| segment.end_seconds)),
            source_type: source_image.source_type.clone(),
            source_item_uri: Some(source_image.item_uri.clone()),
            indexing_profile: Some(indexing_profile(&self.settings)),
            source_uri: Some(source_image.source_uri.clone()),
        })
    }
}

struct SourceIndexPlan {
    source_uris: Vec<String>,
    pending: Vec<PendingSource>,
    already_indexed: usize,
    skipped: usize,
    prune_point_ids: Vec<String>,
    errors: Vec<String>,
}

struct PendingSource {
    source_image: SourceImage,
    indexed_point_ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct IndexedSourceRecord {
    point_id: String,
    size_bytes: u64,
    modified_at: f64,
    indexing_profile: Option<String>,
    analysis_complete: bool,
}

fn source_is_current(
    indexed_records: &[IndexedSourceRecord],
    source_image: &SourceImage,
    indexing_profile: &str,
) -> bool {
    indexed_records
        .iter()
        .any(|record| record_is_current(record, source_image, indexing_profile))
}

fn record_is_current(
    record: &IndexedSourceRecord,
    source_image: &SourceImage,
    indexing_profile: &str,
) -> bool {
    record.size_bytes == source_image.size_bytes
        && (record.modified_at - source_image.modified_at).abs() <= 0.001
        && record.indexing_profile.as_deref() == Some(indexing_profile)
        && record.analysis_complete
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

fn payload_analysis_complete(payload: &ImagePayload, settings: &Settings) -> bool {
    if payload.media_kind == "video_scene"
        && (payload.scene_index.is_none()
            || payload.scene_start_seconds.is_none()
            || payload.scene_end_seconds.is_none())
    {
        return false;
    }

    if settings.audio_transcription_enabled && payload.media_kind == "audio" {
        let Some(analysis) = &payload.audio_analysis else {
            return false;
        };
        if analysis.speech_detected && analysis.transcript_text.trim().is_empty() {
            return false;
        }
    }

    true
}

fn indexing_profile(settings: &Settings) -> String {
    let profile = IndexingProfile {
        version: 2,
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
    audio_transcription_enabled: bool,
    audio_transcription_model: &'a str,
    audio_transcription_language: Option<&'a str>,
    audio_transcription_threads: Option<usize>,
    ocr_enabled: bool,
    ocr_command: &'a str,
    ocr_language: Option<&'a str>,
    ocr_max_frames: usize,
}

fn legacy_source_item_uri(payload: &ImagePayload) -> Option<String> {
    let source_path = payload
        .path
        .split_once('#')
        .map_or(payload.path.as_str(), |(path, _)| path);
    if source_path.is_empty() {
        None
    } else {
        Some(source_path.to_string())
    }
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
