from pathlib import Path

from PIL import Image


def thumbnail_path(thumbnail_dir: Path, image_id: str) -> Path:
    return thumbnail_dir / f"{image_id}.jpg"


def ensure_thumbnail(image: Image.Image, thumbnail_dir: Path, image_id: str, size: tuple[int, int] = (320, 320)) -> str:
    thumbnail_dir.mkdir(parents=True, exist_ok=True)
    output_path = thumbnail_path(thumbnail_dir, image_id)
    if not output_path.exists():
        thumb = image.copy()
        thumb.thumbnail(size)
        thumb.save(output_path, format="JPEG", quality=85, optimize=True)
    return f"/thumbnails/{output_path.name}"

