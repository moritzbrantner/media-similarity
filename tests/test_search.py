from __future__ import annotations

from types import SimpleNamespace
from typing import Callable

import pytest
from PIL import Image

from image_similarity.hashing import phash_image
from image_similarity.models import ImagePayload
from image_similarity.search import ImageSearchService
from conftest import FakeEmbedder, FakeStore

pytestmark = pytest.mark.unit


def payload_for_image(image: Image.Image, phash: str | None = None) -> ImagePayload:
    return ImagePayload(
        id="image-id",
        path="/images/image.jpg",
        relative_path="image.jpg",
        filename="image.jpg",
        width=image.width,
        height=image.height,
        size_bytes=100,
        modified_at=1.0,
        phash=phash or phash_image(image),
        thumbnail_url="/thumbnails/image-id.jpg",
    )


def test_search_image_uses_default_limit_and_converts_results(
    temp_settings,
    make_image: Callable[..., Image.Image],
) -> None:
    image = make_image()
    store = FakeStore(
        search_results=[
            SimpleNamespace(payload=payload_for_image(image).model_dump(), score=0.9),
            SimpleNamespace(payload=None, score=0.1),
        ]
    )
    embedder = FakeEmbedder(vector_size=temp_settings.vector_size)

    response = ImageSearchService(temp_settings, store, embedder).search_image(image)

    assert store.ensure_collection_calls == 1
    assert store.search_calls == [([112.0, 113.0, 114.0, 115.0], temp_settings.default_search_limit)]
    assert response.count == 1
    assert response.results[0].image.filename == "image.jpg"
    assert response.results[0].vector_score == 0.9
    assert response.results[0].hash_distance == 0
    assert response.results[0].near_duplicate is True


def test_search_image_passes_explicit_limit_and_flags_non_duplicates(
    temp_settings,
    make_image: Callable[..., Image.Image],
) -> None:
    image = make_image()
    store = FakeStore(
        search_results=[
            SimpleNamespace(payload=payload_for_image(image, phash="ffffffffffffffff").model_dump(), score=1),
        ]
    )

    response = ImageSearchService(temp_settings, store, FakeEmbedder()).search_image(image, limit=1)

    assert store.search_calls[0][1] == 1
    assert isinstance(response.results[0].vector_score, float)
    assert response.results[0].hash_distance is not None
    assert response.results[0].near_duplicate is False
