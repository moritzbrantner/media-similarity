from __future__ import annotations

from pathlib import Path
from typing import Callable

import pytest

from image_similarity.indexer import ImageIndexer
from image_similarity.models import ImagePayload
from conftest import FakeEmbedder, FakeStore

pytestmark = pytest.mark.unit


def test_index_source_dir_indexes_supported_images_and_payload_metadata(
    temp_settings,
    image_bytes: Callable[..., bytes],
) -> None:
    source = temp_settings.source_image_dir
    nested = source / "nested"
    nested.mkdir(parents=True)
    image_path = nested / "sample.PNG"
    image_path.write_bytes(image_bytes())
    (source / "ignored.txt").write_text("skip")
    store = FakeStore()
    embedder = FakeEmbedder(vector_size=temp_settings.vector_size)

    response = ImageIndexer(temp_settings, store, embedder).index_source_dir()

    assert response.indexed == 1
    assert response.skipped == 0
    assert response.failed == 0
    assert store.ensure_collection_calls == 1
    assert len(store.upserts) == 1
    payload, vector = store.upserts[0]
    assert isinstance(payload, ImagePayload)
    assert payload.filename == "sample.PNG"
    assert payload.relative_path == "nested/sample.PNG"
    assert payload.width == 64
    assert payload.height == 48
    assert payload.size_bytes == image_path.stat().st_size
    assert payload.modified_at == image_path.stat().st_mtime
    assert len(payload.phash) == 16
    assert payload.thumbnail_url == f"/thumbnails/{payload.id}.jpg"
    assert vector == [112.0, 113.0, 114.0, 115.0]


def test_index_source_dir_reports_missing_source(temp_settings) -> None:
    response = ImageIndexer(temp_settings, FakeStore(), FakeEmbedder()).index_source_dir()

    assert response.indexed == 0
    assert response.skipped == 1
    assert response.failed == 0
    assert response.errors == [f"Source directory does not exist: {temp_settings.source_image_dir}"]


def test_index_source_dir_continues_after_corrupt_image(
    temp_settings,
    image_bytes: Callable[..., bytes],
) -> None:
    source = temp_settings.source_image_dir
    source.mkdir()
    (source / "valid.jpg").write_bytes(image_bytes(image_format="JPEG"))
    (source / "broken.jpg").write_bytes(b"not an image")
    store = FakeStore()

    response = ImageIndexer(temp_settings, store, FakeEmbedder()).index_source_dir()

    assert response.indexed == 1
    assert response.failed == 1
    assert len(store.upserts) == 1
    assert "broken.jpg" in response.errors[0]


def test_index_source_dir_truncates_errors(temp_settings) -> None:
    source = temp_settings.source_image_dir
    source.mkdir()
    for index in range(51):
        (source / f"broken-{index:02d}.jpg").write_bytes(b"not an image")

    response = ImageIndexer(temp_settings, FakeStore(), FakeEmbedder()).index_source_dir()

    assert response.indexed == 0
    assert response.failed == 51
    assert len(response.errors) == 50
