from pathlib import Path

from PIL import Image

from image_similarity._rust_backend import BACKEND


def thumbnail_path(thumbnail_dir: Path, image_id: str) -> Path:
    return thumbnail_dir / f"{image_id}.jpg"


def ensure_thumbnail(image: Image.Image, thumbnail_dir: Path, image_id: str, size: tuple[int, int] = (320, 320)) -> str:
    thumbnail_dir.mkdir(parents=True, exist_ok=True)
    output_path = thumbnail_path(thumbnail_dir, image_id)
    if not output_path.exists():
        rgb_image = image.convert("RGB")
        if BACKEND is not None:
            try:
                BACKEND.write_thumbnail_rgb(
                    rgb_image.tobytes(),
                    rgb_image.width,
                    rgb_image.height,
                    str(output_path),
                    size[0],
                    size[1],
                )
                return f"/thumbnails/{output_path.name}"
            except Exception:
                pass

        thumb = rgb_image.copy()
        thumb.thumbnail(size)
        thumb.save(output_path, format="JPEG", quality=85, optimize=True)
    return f"/thumbnails/{output_path.name}"
