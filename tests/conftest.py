from __future__ import annotations

from io import BytesIO
from pathlib import Path
from types import SimpleNamespace
from typing import Callable

import pytest
from PIL import Image

from image_similarity.config import Settings, get_settings


class FakeEmbedder:
    def __init__(self, vector_size: int = 4) -> None:
        self.vector_size = vector_size
        self.calls: list[tuple[int, int]] = []

    def encode(self, image: Image.Image) -> list[float]:
        self.calls.append(image.size)
        width, height = image.size
        base = float((width + height) or 1)
        return [base + float(index) for index in range(self.vector_size)]

    @property
    def dimension(self) -> int:
        return self.vector_size


class FakeStore:
    def __init__(self, search_results: list[SimpleNamespace] | None = None) -> None:
        self.ensure_collection_calls = 0
        self.upserts: list[tuple[object, list[float]]] = []
        self.search_calls: list[tuple[list[float], int]] = []
        self.search_results = search_results or []

    def ensure_collection(self) -> None:
        self.ensure_collection_calls += 1

    def upsert_image(self, payload: object, vector: list[float]) -> None:
        self.upserts.append((payload, vector))

    def search(self, vector: list[float], limit: int) -> list[SimpleNamespace]:
        self.search_calls.append((vector, limit))
        return self.search_results[:limit]

    def count(self) -> int:
        return len(self.upserts)


@pytest.fixture(autouse=True)
def reset_dependency_caches() -> None:
    get_settings.cache_clear()
    try:
        from image_similarity import api

        api._cached_embedder.cache_clear()
        api._cached_store.cache_clear()
    except ImportError:
        pass
    yield
    get_settings.cache_clear()


@pytest.fixture
def make_image() -> Callable[..., Image.Image]:
    def _make_image(size: tuple[int, int] = (64, 48), color: tuple[int, int, int] = (40, 120, 200)) -> Image.Image:
        return Image.new("RGB", size, color=color)

    return _make_image


@pytest.fixture
def image_bytes(make_image: Callable[..., Image.Image]) -> Callable[..., bytes]:
    def _image_bytes(
        image: Image.Image | None = None,
        image_format: str = "PNG",
    ) -> bytes:
        buffer = BytesIO()
        (image or make_image()).save(buffer, format=image_format)
        return buffer.getvalue()

    return _image_bytes


@pytest.fixture
def temp_settings(tmp_path: Path) -> Settings:
    source_dir = tmp_path / "source"
    return Settings(
        SOURCE_IMAGE_DIR=str(source_dir),
        THUMBNAIL_DIR=str(tmp_path / "thumbnails"),
        UPLOAD_DIR=str(tmp_path / "uploads"),
        QDRANT_COLLECTION="test_collection",
        DEFAULT_SEARCH_LIMIT=3,
        DUPLICATE_HASH_DISTANCE=8,
        MAX_UPLOAD_MB=1,
        VECTOR_SIZE=4,
        IMAGE_EXTENSIONS=".jpg,.jpeg,.png,.webp",
    )

