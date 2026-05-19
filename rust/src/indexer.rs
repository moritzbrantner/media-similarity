use crate::audio::{decode_source_audio_segments, expose_source_audio, SourceAudioSegment};
use crate::config::Settings;
use crate::embedder::ImageEmbedder;
use crate::hashing::phash_image;
use crate::image_io::{dimensions, image_id_for_uri};
use crate::media::{DecodedMedia, MediaKind};
use crate::models::{ImagePayload, IndexResponse};
use crate::qdrant::QdrantImageStore;
use crate::sources::{build_image_sources, SourceImage, SourceUnavailable};
use crate::thumbnails::{ensure_animated_thumbnail, ensure_thumbnail};
use crate::video::{decode_source_video_scenes, SourceVideoScene};

#[derive(Clone)]
pub struct ImageIndexer {
    settings: Settings,
    store: QdrantImageStore,
    embedder: ImageEmbedder,
}

impl ImageIndexer {
    pub fn new(settings: Settings, store: QdrantImageStore, embedder: ImageEmbedder) -> Self {
        Self {
            settings,
            store,
            embedder,
        }
    }

    pub async fn index_sources(&self) -> IndexResponse {
        let mut indexed = 0;
        let mut skipped = 0;
        let mut failed = 0;
        let mut errors = Vec::new();
        let sources = build_image_sources(&self.settings);
        let source_uris = sources
            .iter()
            .map(|source| source.uri())
            .collect::<Vec<_>>();

        if let Err(error) = self.store.ensure_collection().await {
            return IndexResponse {
                indexed,
                skipped,
                failed: 1,
                collection: self.settings.qdrant_collection.clone(),
                source_dir: self.settings.source_image_dir.to_string_lossy().to_string(),
                sources: source_uris,
                errors: vec![format!("Could not ensure Qdrant collection: {error}")],
            };
        }

        for source in &sources {
            match source.iter_images() {
                Ok(images) => {
                    for source_image in images {
                        match self.index_one(&source_image).await {
                            Ok(count) => indexed += count,
                            Err(error) => {
                                failed += 1;
                                errors.push(format!("{}: {error}", source_image.display_path));
                            }
                        }
                    }
                }
                Err(SourceUnavailable(error)) => {
                    skipped += 1;
                    errors.push(error);
                }
            }
        }

        errors.truncate(50);
        IndexResponse {
            indexed,
            skipped,
            failed,
            collection: self.settings.qdrant_collection.clone(),
            source_dir: self.settings.source_image_dir.to_string_lossy().to_string(),
            sources: source_uris,
            errors,
        }
    }

    async fn index_one(&self, source_image: &SourceImage) -> Result<usize, String> {
        if source_image.is_video() {
            return self.index_video(source_image).await;
        }
        if source_image.is_audio() {
            return self.index_audio(source_image).await;
        }

        let media = source_image.load_media(&self.settings)?;
        let payload = self.build_payload(source_image, &media, None, None)?;
        let vector = self
            .embedder
            .encode_media(&media.sampled_frames, self.settings.gif_motion_weight);
        self.store.upsert_image(&payload, vector).await?;
        Ok(1)
    }

    async fn index_video(&self, source_image: &SourceImage) -> Result<usize, String> {
        let path = source_image
            .local_path()
            .ok_or_else(|| "Video source does not have a local path".to_string())?;
        let scenes = decode_source_video_scenes(path, &source_image.id_base, &self.settings)?;
        let mut indexed = 0;
        for scene in &scenes {
            let payload = self.build_payload(source_image, &scene.media, Some(scene), None)?;
            let vector = self
                .embedder
                .encode_media(&scene.media.sampled_frames, self.settings.gif_motion_weight);
            self.store.upsert_image(&payload, vector).await?;
            indexed += 1;
        }
        Ok(indexed)
    }

    async fn index_audio(&self, source_image: &SourceImage) -> Result<usize, String> {
        let path = source_image
            .local_path()
            .ok_or_else(|| "Audio source does not have a local path".to_string())?;
        let segments = decode_source_audio_segments(path, &source_image.id_base, &self.settings)?;
        let mut indexed = 0;
        for segment in &segments {
            let payload = self.build_payload(source_image, &segment.media, None, Some(segment))?;
            let vector = self.embedder.encode_media(
                &segment.media.sampled_frames,
                self.settings.gif_motion_weight,
            );
            self.store.upsert_image(&payload, vector).await?;
            indexed += 1;
        }
        Ok(indexed)
    }

    fn build_payload(
        &self,
        source_image: &SourceImage,
        media: &DecodedMedia,
        video_scene: Option<&SourceVideoScene>,
        audio_segment: Option<&SourceAudioSegment>,
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
            source_uri: Some(source_image.source_uri.clone()),
        })
    }
}
