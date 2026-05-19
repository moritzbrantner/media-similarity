from __future__ import annotations

from pathlib import Path
from typing import Callable

import pytest
from PIL import Image

from image_similarity.image_io import image_id_for_path, image_id_for_uri, iter_image_paths, load_image, load_image_bytes, relative_path

pytestmark = pytest.mark.unit


def test_iter_image_paths_filters_extensions_case_insensitively_and_sorts(
    tmp_path: Path,
    image_bytes: Callable[..., bytes],
) -> None:
    source = tmp_path / "source"
    nested = source / "nested"
    nested.mkdir(parents=True)
    (source / "b.PNG").write_bytes(image_bytes())
    (source / "a.jpg").write_bytes(image_bytes())
    (nested / "c.webp").write_bytes(image_bytes(image_format="WEBP"))
    (source / "notes.txt").write_text("not an image")
    (source / "empty_dir.jpg").mkdir()

    paths = list(iter_image_paths(source, {".jpg", ".png", ".webp"}))

    assert [path.relative_to(source).as_posix() for path in paths] == ["a.jpg", "b.PNG", "nested/c.webp"]


def test_iter_image_paths_missing_source_yields_nothing(tmp_path: Path) -> None:
    assert list(iter_image_paths(tmp_path / "missing", {".jpg"})) == []


def test_relative_path_uses_posix_inside_root(tmp_path: Path) -> None:
    root = tmp_path / "root"
    path = root / "nested" / "image.jpg"

    assert relative_path(path, root) == "nested/image.jpg"


def test_relative_path_falls_back_to_filename_outside_root(tmp_path: Path) -> None:
    assert relative_path(tmp_path / "other" / "image.jpg", tmp_path / "root") == "image.jpg"


def test_image_id_for_path_is_deterministic(tmp_path: Path) -> None:
    path = tmp_path / "image.jpg"
    path.write_text("content")

    assert image_id_for_path(path) == image_id_for_path(path)


def test_image_id_for_uri_is_deterministic_and_uri_specific() -> None:
    first = image_id_for_uri("minio://bucket/a.jpg")
    second = image_id_for_uri("minio://bucket/b.jpg")

    assert first == image_id_for_uri("minio://bucket/a.jpg")
    assert first != second


def test_load_image_returns_rgb(tmp_path: Path) -> None:
    path = tmp_path / "palette.png"
    Image.new("P", (10, 12)).save(path)

    loaded = load_image(path)

    assert loaded.mode == "RGB"
    assert loaded.size == (10, 12)


def test_load_image_bytes_returns_rgb(image_bytes: Callable[..., bytes]) -> None:
    loaded = load_image_bytes(image_bytes(Image.new("P", (9, 11))))

    assert loaded.mode == "RGB"
    assert loaded.size == (9, 11)
