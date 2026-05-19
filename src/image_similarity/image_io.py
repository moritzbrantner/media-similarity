from __future__ import annotations

import uuid
from pathlib import Path
from typing import Iterable

from PIL import Image, ImageOps


def iter_image_paths(source_dir: Path, extensions: Iterable[str]) -> Iterable[Path]:
    normalized = {extension.lower() for extension in extensions}
    if not source_dir.exists():
        return
    for path in sorted(source_dir.rglob("*")):
        if path.is_file() and path.suffix.lower() in normalized:
            yield path


def load_image(path: Path) -> Image.Image:
    with Image.open(path) as image:
        return ImageOps.exif_transpose(image).convert("RGB")


def image_id_for_path(path: Path) -> str:
    return str(uuid.uuid5(uuid.NAMESPACE_URL, str(path.resolve())))


def relative_path(path: Path, root: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.name

