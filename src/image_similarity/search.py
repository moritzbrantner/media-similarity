from __future__ import annotations

from PIL import Image

from image_similarity.config import Settings
from image_similarity.embeddings import ImageEmbedder
from image_similarity.hashing import hash_distance, phash_image
from image_similarity.models import ImagePayload, SearchResponse, SearchResult
from image_similarity.qdrant_store import QdrantImageStore


class ImageSearchService:
    def __init__(self, settings: Settings, store: QdrantImageStore, embedder: ImageEmbedder) -> None:
        self.settings = settings
        self.store = store
        self.embedder = embedder

    def search_image(self, image: Image.Image, limit: int | None = None) -> SearchResponse:
        self.store.ensure_collection()
        query_phash = phash_image(image)
        query_vector = self.embedder.encode(image)
        points = self.store.search(query_vector, limit or self.settings.default_search_limit)

        results: list[SearchResult] = []
        for point in points:
            if not point.payload:
                continue
            payload = ImagePayload.model_validate(point.payload)
            distance = hash_distance(query_phash, payload.phash)
            results.append(
                SearchResult(
                    image=payload,
                    vector_score=float(point.score),
                    hash_distance=distance,
                    near_duplicate=distance <= self.settings.duplicate_hash_distance,
                )
            )

        return SearchResponse(query_phash=query_phash, count=len(results), results=results)

