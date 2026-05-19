from __future__ import annotations

from image_similarity.config import Settings
from image_similarity.embeddings import ImageEmbedder
from image_similarity.hashing import phash_image
from image_similarity.image_io import image_id_for_uri
from image_similarity.models import ImagePayload, IndexResponse
from image_similarity.qdrant_store import QdrantImageStore
from image_similarity.sources import SourceImage, SourceUnavailable, build_image_sources
from image_similarity.thumbnails import ensure_thumbnail


class ImageIndexer:
    def __init__(self, settings: Settings, store: QdrantImageStore, embedder: ImageEmbedder) -> None:
        self.settings = settings
        self.store = store
        self.embedder = embedder

    def index_sources(self) -> IndexResponse:
        self.store.ensure_collection()
        self.settings.thumbnail_dir.mkdir(parents=True, exist_ok=True)

        indexed = 0
        skipped = 0
        failed = 0
        errors: list[str] = []
        sources = build_image_sources(self.settings)

        for source in sources:
            try:
                for source_image in source.iter_images():
                    try:
                        image = source_image.load_image()
                        payload = self._build_payload(source_image, image)
                        vector = self.embedder.encode(image)
                        self.store.upsert_image(payload, vector)
                        indexed += 1
                    except Exception as exc:  # noqa: BLE001 - keep indexing the rest of the source.
                        failed += 1
                        errors.append(f"{source_image.display_path}: {exc}")
            except SourceUnavailable as exc:
                skipped += 1
                errors.append(str(exc))
            except Exception as exc:  # noqa: BLE001 - continue with remaining sources.
                failed += 1
                errors.append(f"{source.uri}: {exc}")

        return IndexResponse(
            indexed=indexed,
            skipped=skipped,
            failed=failed,
            collection=self.settings.qdrant_collection,
            source_dir=str(self.settings.source_image_dir),
            sources=[source.uri for source in sources],
            errors=errors[:50],
        )

    def index_source_dir(self) -> IndexResponse:
        return self.index_sources()

    def _build_payload(self, source_image: SourceImage, image) -> ImagePayload:
        image_id = image_id_for_uri(source_image.id_base)
        thumbnail_url = ensure_thumbnail(image, self.settings.thumbnail_dir, image_id)
        width, height = image.size
        return ImagePayload(
            id=image_id,
            path=source_image.display_path,
            relative_path=source_image.relative_path,
            filename=source_image.filename,
            width=width,
            height=height,
            size_bytes=source_image.size_bytes,
            modified_at=source_image.modified_at,
            phash=phash_image(image),
            thumbnail_url=thumbnail_url,
            source_type=source_image.source_type,
            source_uri=source_image.source_uri,
        )
