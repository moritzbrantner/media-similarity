from __future__ import annotations

from pathlib import Path
from typing import Callable

import pytest
from PIL import Image

from image_similarity.thumbnails import ensure_thumbnail, thumbnail_path

pytestmark = pytest.mark.unit


def test_ensure_thumbnail_creates_jpeg_and_returns_url(
    tmp_path: Path,
    make_image: Callable[..., Image.Image],
) -> None:
    image_id = "image-1"

    url = ensure_thumbnail(make_image(size=(800, 600)), tmp_path / "thumbs", image_id)
    output_path = thumbnail_path(tmp_path / "thumbs", image_id)

    assert url == "/thumbnails/image-1.jpg"
    assert output_path.exists()
    with Image.open(output_path) as thumbnail:
        assert thumbnail.format == "JPEG"
        assert thumbnail.size[0] <= 320
        assert thumbnail.size[1] <= 320


def test_ensure_thumbnail_does_not_overwrite_existing_file(
    tmp_path: Path,
    make_image: Callable[..., Image.Image],
) -> None:
    thumb_dir = tmp_path / "thumbs"
    thumb_dir.mkdir()
    output_path = thumbnail_path(thumb_dir, "image-1")
    output_path.write_bytes(b"existing")

    ensure_thumbnail(make_image(), thumb_dir, "image-1")

    assert output_path.read_bytes() == b"existing"
