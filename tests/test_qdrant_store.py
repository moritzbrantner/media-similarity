from __future__ import annotations

import sys
from types import ModuleType, SimpleNamespace

import pytest

from image_similarity.models import ImagePayload
from image_similarity.qdrant_store import QdrantImageStore

pytestmark = pytest.mark.unit


class VectorParams:
    def __init__(self, *, size: int, distance: str) -> None:
        self.size = size
        self.distance = distance


class PointStruct:
    def __init__(self, *, id: str, vector: list[float], payload: dict) -> None:
        self.id = id
        self.vector = vector
        self.payload = payload


class FakeQdrantClient:
    def __init__(self, *, url: str) -> None:
        self.url = url
        self.collections: list[str] = []
        self.created_collections: list[tuple[str, VectorParams]] = []
        self.upserts: list[tuple[str, list[PointStruct]]] = []
        self.search_calls: list[dict] = []
        self.count_calls: list[dict] = []
        self.search_results = [SimpleNamespace(payload={"id": "one"}, score=0.7)]
        self.count_result = 42

    def get_collections(self):
        return SimpleNamespace(
            collections=[SimpleNamespace(name=name) for name in self.collections],
        )

    def create_collection(self, *, collection_name: str, vectors_config: VectorParams) -> None:
        self.created_collections.append((collection_name, vectors_config))
        self.collections.append(collection_name)

    def upsert(self, *, collection_name: str, points: list[PointStruct]) -> None:
        self.upserts.append((collection_name, points))

    def search(self, **kwargs):
        self.search_calls.append(kwargs)
        return self.search_results

    def count(self, **kwargs):
        self.count_calls.append(kwargs)
        return SimpleNamespace(count=self.count_result)


@pytest.fixture
def fake_qdrant_modules(monkeypatch: pytest.MonkeyPatch):
    clients: list[FakeQdrantClient] = []

    class TrackingQdrantClient(FakeQdrantClient):
        def __init__(self, *, url: str) -> None:
            super().__init__(url=url)
            clients.append(self)

    qdrant_client = ModuleType("qdrant_client")
    qdrant_client.QdrantClient = TrackingQdrantClient

    http_module = ModuleType("qdrant_client.http")
    models_module = ModuleType("qdrant_client.http.models")
    models_module.Distance = SimpleNamespace(COSINE="Cosine")
    models_module.VectorParams = VectorParams
    models_module.PointStruct = PointStruct
    http_module.models = models_module

    monkeypatch.setitem(sys.modules, "qdrant_client", qdrant_client)
    monkeypatch.setitem(sys.modules, "qdrant_client.http", http_module)
    monkeypatch.setitem(sys.modules, "qdrant_client.http.models", models_module)

    return SimpleNamespace(clients=clients)


def sample_payload() -> ImagePayload:
    return ImagePayload(
        id="image-id",
        path="/images/cat.jpg",
        relative_path="cat.jpg",
        filename="cat.jpg",
        width=64,
        height=48,
        size_bytes=1234,
        modified_at=1.5,
        phash="0000000000000000",
        thumbnail_url="/thumbnails/image-id.jpg",
    )


def test_store_initializes_qdrant_client_without_network(fake_qdrant_modules) -> None:
    store = QdrantImageStore("http://localhost:6333", "images", 128)

    assert store.client is fake_qdrant_modules.clients[0]
    assert store.client.url == "http://localhost:6333"
    assert store.collection == "images"
    assert store.vector_size == 128


def test_ensure_collection_creates_missing_collection_with_cosine_vectors(fake_qdrant_modules) -> None:
    store = QdrantImageStore("http://qdrant:6333", "images", 512)

    store.ensure_collection()

    client = fake_qdrant_modules.clients[0]
    assert len(client.created_collections) == 1
    collection_name, vectors_config = client.created_collections[0]
    assert collection_name == "images"
    assert vectors_config.size == 512
    assert vectors_config.distance == "Cosine"


def test_ensure_collection_reuses_existing_collection(fake_qdrant_modules) -> None:
    store = QdrantImageStore("http://qdrant:6333", "images", 512)
    fake_qdrant_modules.clients[0].collections = ["images"]

    store.ensure_collection()

    assert fake_qdrant_modules.clients[0].created_collections == []


def test_upsert_image_serializes_payload_and_vector(fake_qdrant_modules) -> None:
    store = QdrantImageStore("http://qdrant:6333", "images", 512)
    payload = sample_payload()

    store.upsert_image(payload, [0.1, 0.2, 0.3])

    collection_name, points = fake_qdrant_modules.clients[0].upserts[0]
    assert collection_name == "images"
    assert len(points) == 1
    assert points[0].id == "image-id"
    assert points[0].vector == [0.1, 0.2, 0.3]
    assert points[0].payload == payload.model_dump()


def test_search_delegates_query_vector_limit_and_payload_flag(fake_qdrant_modules) -> None:
    store = QdrantImageStore("http://qdrant:6333", "images", 512)

    results = store.search([1.0, 2.0], limit=5)

    assert results == fake_qdrant_modules.clients[0].search_results
    assert fake_qdrant_modules.clients[0].search_calls == [
        {
            "collection_name": "images",
            "query_vector": [1.0, 2.0],
            "limit": 5,
            "with_payload": True,
        }
    ]


def test_count_ensures_collection_and_requests_exact_count(fake_qdrant_modules) -> None:
    store = QdrantImageStore("http://qdrant:6333", "images", 512)

    assert store.count() == 42

    client = fake_qdrant_modules.clients[0]
    assert client.created_collections[0][0] == "images"
    assert client.count_calls == [{"collection_name": "images", "exact": True}]
