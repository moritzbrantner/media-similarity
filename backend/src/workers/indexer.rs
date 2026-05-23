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

impl ImageIndexer {
    pub fn new(
        settings: Settings,
        store: Arc<dyn MediaVectorStore>,
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
            match self.delete_generated_records(&plan.prune_point_ids).await {
                Ok(deleted) => {
                    pruned += deleted;
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
                        match self.delete_generated_records(&stale_point_ids).await {
                            Ok(deleted) => {
                                pruned += deleted;
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
        if source_image.is_pdf() {
            return self.index_pdf(source_image).await;
        }

        let photo_metadata = source_image
            .local_path()
            .filter(|_| !source_image.is_video() && !source_image.is_audio() && !source_image.is_pdf())
            .and_then(|path| match extract_photo_metadata(path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "photo metadata extraction failed");
                    None
                }
            });
        let media = source_image.load_media(&self.settings)?;
        let media_id = image_id_for_uri(&source_image.id_base);
        let face_analysis = analyze_faces_for_media(
            &self.settings,
            self.store.as_ref(),
            &media,
            &media_id,
            Some(source_image.source_uri.clone()),
            Some(source_image.item_uri.clone()),
        )
        .await;
        let payload = self.build_payload(
            source_image,
            &media,
            PayloadBuildOptions::new(&face_analysis).with_photo_metadata(photo_metadata),
        )?;
        let vector = self
            .embedder
            .embed_media(&media.sampled_frames, self.settings.gif_motion_weight)?;
        self.store.upsert_media(&payload, vector).await?;
        Ok(IndexOneOutcome::single(payload.id))
    }

    async fn delete_generated_records(&self, point_ids: &[String]) -> Result<usize, String> {
        let mut deleted = 0;
        let mut errors = Vec::new();
        for point_id in point_ids {
            let response =
                delete_indexed_media(&self.settings, self.store.as_ref(), point_id).await;
            deleted += response.deleted_points;
            errors.extend(response.errors);
        }
        if errors.is_empty() {
            Ok(deleted)
        } else {
            Err(errors.join("; "))
        }
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
                self.store.as_ref(),
                &scene.media,
                &media_id,
                Some(source_image.source_uri.clone()),
                Some(source_image.item_uri.clone()),
            )
            .await;
            let payload = self.build_payload(
                source_image,
                &scene.media,
                PayloadBuildOptions::new(&face_analysis).with_video_scene(scene),
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
            let face_analysis = FaceAnalysis::default();
            let payload = self.build_payload(
                source_image,
                &segment.media,
                PayloadBuildOptions::new(&face_analysis).with_audio_segment(segment),
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

    async fn index_pdf(&self, source_image: &SourceImage) -> Result<IndexOneOutcome, String> {
        let path = source_image
            .local_path()
            .ok_or_else(|| "PDF source does not have a local path".to_string())?;
        let pdf = decode_pdf(path, &self.settings)?;
        let source_pdf_id = image_id_for_uri(&source_image.id_base);
        let full_pdf_url = expose_source_pdf(path, &source_pdf_id, &self.settings)?;
        let document_id_base = format!("{}#document", source_image.id_base);
        let document_id = image_id_for_uri(&document_id_base);
        let mut outcome = IndexOneOutcome::default();
        let mut page_texts = Vec::new();

        for page in &pdf.pages {
            let page_ocr = extract_media_ocr(&page.media, &self.settings).unwrap_or_else(|error| {
                tracing::warn!(%error, "PDF page OCR extraction failed");
                Default::default()
            });
            let merged_text = merge_pdf_text(&page.embedded_text, &page_ocr.text);
            if !merged_text.is_empty() {
                page_texts.push(merged_text.clone());
            }
            let page_number = page.page_number;
            let page_context = PdfPayloadContext {
                id_base: format!("{}#page={page_number}", source_image.id_base),
                relative_path: format!("{}#page-{page_number:03}", source_image.relative_path),
                filename: format!("{} page {page_number:03}", source_image.filename),
                path: format!("{}#page={page_number}", source_image.display_path),
                full_pdf_url: full_pdf_url.clone(),
                pdf_page_url: full_pdf_url
                    .as_ref()
                    .map(|url| format!("{url}#page={page_number}")),
                pdf_document_id: Some(document_id.clone()),
                pdf_page_index: Some(page.page_index),
                pdf_page_number: Some(page.page_number),
                pdf_page_count: Some(pdf.page_count),
            };
            let face_analysis = FaceAnalysis::default();
            let payload = self.build_payload(
                source_image,
                &page.media,
                PayloadBuildOptions::new(&face_analysis)
                    .with_pdf_context(&page_context)
                    .with_ocr(OcrAnalysis {
                        text: merged_text,
                        frames: page_ocr.frames,
                    }),
            )?;
            let vector = self
                .embedder
                .embed_media(&page.media.sampled_frames, self.settings.gif_motion_weight)?;
            let point_id = payload.id.clone();
            self.store.upsert_media(&payload, vector).await?;
            outcome.insert(point_id);
        }

        let document_text = merge_pdf_text(&pdf.document_text, &page_texts.join(" "));
        let document_context = PdfPayloadContext {
            id_base: document_id_base,
            relative_path: format!("{}#document", source_image.relative_path),
            filename: format!("{} document", source_image.filename),
            path: format!("{}#document", source_image.display_path),
            full_pdf_url,
            pdf_page_url: None,
            pdf_document_id: None,
            pdf_page_index: None,
            pdf_page_number: None,
            pdf_page_count: Some(pdf.page_count),
        };
        let face_analysis = FaceAnalysis::default();
        let payload = self.build_payload(
            source_image,
            &pdf.document_media,
            PayloadBuildOptions::new(&face_analysis)
                .with_pdf_context(&document_context)
                .with_ocr(OcrAnalysis {
                    text: document_text,
                    frames: Vec::new(),
                }),
        )?;
        let vector = self.embedder.embed_media(
            &pdf.document_media.sampled_frames,
            self.settings.gif_motion_weight,
        )?;
        let point_id = payload.id.clone();
        self.store.upsert_media(&payload, vector).await?;
        outcome.insert(point_id);

        Ok(outcome)
    }

    fn build_payload(
        &self,
        source_image: &SourceImage,
        media: &DecodedMedia,
        options: PayloadBuildOptions<'_>,
    ) -> Result<ImagePayload, String> {
        let video_scene = options.video_scene;
        let audio_segment = options.audio_segment;
        let pdf_context = options.pdf_context;
        let id_base = if let Some(pdf) = pdf_context {
            pdf.id_base.clone()
        } else if let Some(scene) = video_scene {
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
        let ocr_analysis = options.ocr_override.unwrap_or_else(|| {
            extract_media_ocr(media, &self.settings).unwrap_or_else(|error| {
                tracing::warn!(%error, "OCR extraction failed");
                Default::default()
            })
        });
        let relative_path = if let Some(pdf) = pdf_context {
            pdf.relative_path.clone()
        } else if let Some(scene) = video_scene {
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
        let filename = if let Some(pdf) = pdf_context {
            pdf.filename.clone()
        } else if let Some(scene) = video_scene {
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
        let path = if let Some(pdf) = pdf_context {
            pdf.path.clone()
        } else if let Some(scene) = video_scene {
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
        let full_video_url = video_scene.and_then(|scene| scene.full_video_url.clone());
        let full_pdf_url = pdf_context.and_then(|pdf| pdf.full_pdf_url.clone());
        let pdf_page_url = pdf_context.and_then(|pdf| pdf.pdf_page_url.clone());
        let scene_clip_url = video_scene.and_then(|scene| scene.clip_url.clone());
        let artifacts = generated_artifacts(
            Some(&thumbnail_url),
            animated_thumbnail_url.as_deref(),
            full_video_url.as_deref(),
            full_audio_url.as_deref(),
            full_pdf_url.as_deref(),
            pdf_page_url.as_deref(),
            scene_clip_url.as_deref(),
        );
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
            full_video_url,
            full_audio_url,
            full_pdf_url,
            pdf_page_url,
            pdf_document_id: pdf_context.and_then(|pdf| pdf.pdf_document_id.clone()),
            pdf_page_index: pdf_context.and_then(|pdf| pdf.pdf_page_index),
            pdf_page_number: pdf_context.and_then(|pdf| pdf.pdf_page_number),
            pdf_page_count: pdf_context.and_then(|pdf| pdf.pdf_page_count),
            audio_analysis: media.audio_analysis.clone(),
            ocr_text: ocr_analysis.text,
            ocr_frames: ocr_analysis.frames,
            visual_embedding_model: Some(self.embedder.model_name().to_string()),
            faces: options.face_analysis.faces.clone(),
            people: options.face_analysis.person_clusters.clone(),
            artifacts,
            tags: Vec::new(),
            photo_metadata: options.photo_metadata.clone(),
            scene_clip_url,
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

struct PayloadBuildOptions<'a> {
    video_scene: Option<&'a SourceVideoScene>,
    audio_segment: Option<&'a SourceAudioSegment>,
    pdf_context: Option<&'a PdfPayloadContext>,
    ocr_override: Option<OcrAnalysis>,
    photo_metadata: Option<PhotoMetadataPayload>,
    face_analysis: &'a FaceAnalysis,
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
        version: 4,
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
