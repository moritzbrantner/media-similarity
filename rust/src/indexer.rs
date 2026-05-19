use crate::config::Settings;
use crate::embedder::ImageEmbedder;
use crate::hashing::phash_image;
use crate::image_io::{dimensions, image_id_for_uri};
use crate::models::{ImagePayload, IndexResponse};
use crate::qdrant::QdrantImageStore;
use crate::sources::{build_image_sources, SourceImage, SourceUnavailable};
use crate::thumbnails::ensure_thumbnail;

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
                            Ok(()) => indexed += 1,
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

    async fn index_one(&self, source_image: &SourceImage) -> Result<(), String> {
        let image = source_image.load_image()?;
        let payload = self.build_payload(source_image, &image)?;
        let vector = self.embedder.encode(&image);
        self.store.upsert_image(&payload, vector).await
    }

    fn build_payload(
        &self,
        source_image: &SourceImage,
        image: &image::RgbImage,
    ) -> Result<ImagePayload, String> {
        let image_id = image_id_for_uri(&source_image.id_base);
        let thumbnail_url =
            ensure_thumbnail(image, &self.settings.thumbnail_dir, &image_id, (320, 320))?;
        let (width, height) = dimensions(image);
        Ok(ImagePayload {
            id: image_id,
            path: source_image.display_path.clone(),
            relative_path: source_image.relative_path.clone(),
            filename: source_image.filename.clone(),
            width,
            height,
            size_bytes: source_image.size_bytes,
            modified_at: source_image.modified_at,
            phash: phash_image(image),
            thumbnail_url: Some(thumbnail_url),
            source_type: source_image.source_type.clone(),
            source_uri: Some(source_image.source_uri.clone()),
        })
    }
}
