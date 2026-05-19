from __future__ import annotations

from pathlib import Path
from typing import Callable

import pytest
from PIL import Image

from image_similarity import indexer as indexer_module
from image_similarity.indexer import ImageIndexer
from image_similarity.models import ImagePayload
from image_similarity.sources import SourceImage, SourceUnavailable
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
    assert payload.source_type == "local"
    assert payload.source_uri == str(source)
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
    assert response.sources == [str(temp_settings.source_image_dir)]
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


def test_index_sources_indexes_multiple_configured_local_sources(
    temp_settings,
    tmp_path: Path,
    image_bytes: Callable[..., bytes],
) -> None:
    first = tmp_path / "first"
    second = tmp_path / "second"
    first.mkdir()
    second.mkdir()
    (first / "one.jpg").write_bytes(image_bytes(image_format="JPEG"))
    (second / "two.png").write_bytes(image_bytes())
    settings = temp_settings.model_copy(update={"image_sources": [str(first), str(second)]})
    store = FakeStore()

    response = ImageIndexer(settings, store, FakeEmbedder()).index_sources()

    assert response.indexed == 2
    assert response.skipped == 0
    assert response.failed == 0
    assert response.sources == [str(first), str(second)]
    assert [payload.source_uri for payload, _ in store.upserts] == [str(first), str(second)]
    assert [payload.filename for payload, _ in store.upserts] == ["one.jpg", "two.png"]


def test_index_sources_continues_across_item_and_source_failures(temp_settings, monkeypatch) -> None:
    valid_image = Image.new("RGB", (12, 8), color=(10, 20, 30))

    def broken_loader() -> Image.Image:
        raise RuntimeError("broken image")

    valid_item = SourceImage(
        source_type="fake",
        source_uri="fake://good",
        item_uri="fake://good/valid.jpg",
        id_base="fake://good/valid.jpg",
        display_path="fake://good/valid.jpg",
        relative_path="valid.jpg",
        filename="valid.jpg",
        size_bytes=10,
        modified_at=1.0,
        _loader=lambda: valid_image,
    )
    broken_item = SourceImage(
        source_type="fake",
        source_uri="fake://good",
        item_uri="fake://good/broken.jpg",
        id_base="fake://good/broken.jpg",
        display_path="fake://good/broken.jpg",
        relative_path="broken.jpg",
        filename="broken.jpg",
        size_bytes=20,
        modified_at=2.0,
        _loader=broken_loader,
    )
    sources = [
        FakeSource("fake://good", [valid_item, broken_item]),
        FakeSource("fake://missing", unavailable=SourceUnavailable("source unavailable")),
        FakeSource("fake://boom", failure=RuntimeError("source exploded")),
    ]
    monkeypatch.setattr(indexer_module, "build_image_sources", lambda settings: sources)
    store = FakeStore()

    response = ImageIndexer(temp_settings, store, FakeEmbedder()).index_sources()

    assert response.indexed == 1
    assert response.skipped == 1
    assert response.failed == 2
    assert response.sources == ["fake://good", "fake://missing", "fake://boom"]
    assert len(store.upserts) == 1
    payload, vector = store.upserts[0]
    assert payload.path == "fake://good/valid.jpg"
    assert payload.source_type == "fake"
    assert payload.source_uri == "fake://good"
    assert vector == [20.0, 21.0, 22.0, 23.0]
    assert "fake://good/broken.jpg: broken image" in response.errors
    assert "source unavailable" in response.errors
    assert "fake://boom: source exploded" in response.errors


class FakeSource:
    def __init__(
        self,
        uri: str,
        items: list[SourceImage] | None = None,
        *,
        unavailable: SourceUnavailable | None = None,
        failure: Exception | None = None,
    ) -> None:
        self.uri = uri
        self.items = items or []
        self.unavailable = unavailable
        self.failure = failure

    def iter_images(self):
        if self.unavailable is not None:
            raise self.unavailable
        if self.failure is not None:
            raise self.failure
        yield from self.items
