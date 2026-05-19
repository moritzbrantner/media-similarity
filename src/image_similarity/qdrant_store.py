from __future__ import annotations

from typing import Any

from image_similarity.models import ImagePayload


class QdrantImageStore:
    def __init__(self, url: str, collection: str, vector_size: int) -> None:
        from qdrant_client import QdrantClient

        self.client = QdrantClient(url=url)
        self.collection = collection
        self.vector_size = vector_size

    def ensure_collection(self) -> None:
        from qdrant_client.http import models as qmodels

        existing = [collection.name for collection in self.client.get_collections().collections]
        if self.collection in existing:
            return
        self.client.create_collection(
            collection_name=self.collection,
            vectors_config=qmodels.VectorParams(size=self.vector_size, distance=qmodels.Distance.COSINE),
        )

    def upsert_image(self, payload: ImagePayload, vector: list[float]) -> None:
        from qdrant_client.http import models as qmodels

        self.client.upsert(
            collection_name=self.collection,
            points=[
                qmodels.PointStruct(
                    id=payload.id,
                    vector=vector,
                    payload=payload.model_dump(),
                )
            ],
        )

    def search(self, vector: list[float], limit: int) -> list[Any]:
        return self.client.search(
            collection_name=self.collection,
            query_vector=vector,
            limit=limit,
            with_payload=True,
        )

    def count(self) -> int:
        self.ensure_collection()
        return self.client.count(collection_name=self.collection, exact=True).count
