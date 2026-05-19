from __future__ import annotations

from typing import Callable

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from image_similarity import api
from image_similarity.models import IndexResponse, SearchResponse

pytestmark = [pytest.mark.unit, pytest.mark.api]


class FakeIndexer:
    def __init__(self) -> None:
        self.calls = 0

    def index_sources(self) -> IndexResponse:
        self.calls += 1
        return IndexResponse(
            indexed=2,
            skipped=0,
            failed=0,
            collection="test_collection",
            source_dir="/tmp/source",
            sources=["/tmp/source"],
        )


class FakeSearchService:
    def __init__(self) -> None:
        self.limits: list[int | None] = []

    def search_image(self, image, limit: int | None = None) -> SearchResponse:
        self.limits.append(limit)
        return SearchResponse(query_phash="0000000000000000", count=0, results=[])


@pytest.fixture
def api_client(temp_settings):
    app = FastAPI()
    app.include_router(api.router)
    indexer = FakeIndexer()
    search_service = FakeSearchService()
    app.dependency_overrides[api.get_settings] = lambda: temp_settings
    app.dependency_overrides[api.get_indexer] = lambda: indexer
    app.dependency_overrides[api.get_search_service] = lambda: search_service
    return TestClient(app), indexer, search_service, temp_settings


def test_health_returns_settings(api_client) -> None:
    client, _, _, settings = api_client

    response = client.get("/api/health")

    assert response.status_code == 200
    assert response.json() == {
        "status": "ok",
        "collection": "test_collection",
        "source_dir": str(settings.source_image_dir),
        "sources": [str(settings.source_image_dir)],
    }


def test_index_uses_fake_indexer(api_client) -> None:
    client, indexer, _, _ = api_client

    response = client.post("/api/index")

    assert response.status_code == 200
    assert response.json()["indexed"] == 2
    assert indexer.calls == 1


def test_search_accepts_valid_image_and_passes_limit(
    api_client,
    image_bytes: Callable[..., bytes],
) -> None:
    client, _, search_service, _ = api_client

    response = client.post(
        "/api/search?limit=7",
        files={"file": ("query.png", image_bytes(), "image/png")},
    )

    assert response.status_code == 200
    assert response.json()["count"] == 0
    assert search_service.limits == [7]


def test_search_rejects_non_image_content_type(api_client) -> None:
    client, _, _, _ = api_client

    response = client.post(
        "/api/search",
        files={"file": ("query.txt", b"hello", "text/plain")},
    )

    assert response.status_code == 400
    assert response.json()["detail"] == "Upload must be an image file"


def test_search_rejects_oversized_upload(api_client) -> None:
    client, _, _, _ = api_client

    response = client.post(
        "/api/search",
        files={"file": ("query.png", b"x" * (1024 * 1024 + 1), "image/png")},
    )

    assert response.status_code == 413
    assert response.json()["detail"] == "Upload is larger than 1 MB"


def test_search_rejects_undecodable_image(api_client) -> None:
    client, _, _, _ = api_client

    response = client.post(
        "/api/search",
        files={"file": ("query.png", b"not an image", "image/png")},
    )

    assert response.status_code == 400
    assert response.json()["detail"] == "Could not decode image"
