from __future__ import annotations

from pathlib import Path

from image_similarity.config import Settings
from image_similarity.embeddings import ImageEmbedder
from image_similarity.hashing import phash_image
from image_similarity.image_io import image_id_for_path, iter_image_paths, load_image, relative_path
from image_similarity.models import ImagePayload, IndexResponse
from image_similarity.qdrant_store import QdrantImageStore
from image_similarity.thumbnails import ensure_thumbnail


class ImageIndexer:
    def __init__(self, settings: Settings, store: QdrantImageStore, embedder: ImageEmbedder) -> None:
        self.settings = settings
        self.store = store
        self.embedder = embedder

    def index_source_dir(self) -> IndexResponse:
        self.store.ensure_collection()
        self.settings.thumbnail_dir.mkdir(parents=True, exist_ok=True)

        indexed = 0
        skipped = 0
        failed = 0
        errors: list[str] = []

        for path in iter_image_paths(self.settings.source_image_dir, self.settings.image_extensions):
            try:
                payload = self._build_payload(path)
                vector = self.embedder.encode(load_image(path))
                self.store.upsert_image(payload, vector)
                indexed += 1
            except Exception as exc:  # noqa: BLE001 - keep indexing the rest of the folder.
                failed += 1
                errors.append(f"{path}: {exc}")

        if not self.settings.source_image_dir.exists():
            skipped += 1
            errors.append(f"Source directory does not exist: {self.settings.source_image_dir}")

        return IndexResponse(
            indexed=indexed,
            skipped=skipped,
            failed=failed,
            collection=self.settings.qdrant_collection,
            source_dir=str(self.settings.source_image_dir),
            errors=errors[:50],
        )

    def _build_payload(self, path: Path) -> ImagePayload:
        image = load_image(path)
        stat = path.stat()
        image_id = image_id_for_path(path)
        thumbnail_url = ensure_thumbnail(image, self.settings.thumbnail_dir, image_id)
        width, height = image.size
        return ImagePayload(
            id=image_id,
            path=str(path.resolve()),
            relative_path=relative_path(path, self.settings.source_image_dir),
            filename=path.name,
            width=width,
            height=height,
            size_bytes=stat.st_size,
            modified_at=stat.st_mtime,
            phash=phash_image(image),
            thumbnail_url=thumbnail_url,
        )

