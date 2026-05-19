from __future__ import annotations

import os
import time
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw


DEFAULT_COLORS = [
    (220, 68, 55),
    (58, 132, 92),
    (58, 104, 184),
    (232, 176, 59),
    (141, 87, 166),
    (54, 155, 171),
    (226, 116, 65),
    (96, 96, 96),
]


def main() -> None:
    output_dir = Path(os.environ.get("DUMMY_DATA_DIR", "sample-images"))
    count = _positive_int(os.environ.get("DUMMY_IMAGE_COUNT"), 8)
    width, height = _image_size(os.environ.get("DUMMY_IMAGE_SIZE", "640x480"))
    image_format = os.environ.get("DUMMY_IMAGE_FORMAT", "JPEG").upper()
    extension = "jpg" if image_format == "JPEG" else image_format.lower()

    output_dir.mkdir(parents=True, exist_ok=True)
    generated = []
    for index in range(1, count + 1):
        path = output_dir / f"dummy-{index:02d}.{extension}"
        image = _make_image(index=index, size=(width, height))
        image.save(path, format=image_format, quality=90)
        generated.append(path)

    print(f"Generated {len(generated)} dummy images in {output_dir}")

    if _bool(os.environ.get("SEED_MINIO"), default=True):
        _seed_minio(generated)


def _make_image(*, index: int, size: tuple[int, int]) -> Image.Image:
    width, height = size
    background = DEFAULT_COLORS[(index - 1) % len(DEFAULT_COLORS)]
    accent = tuple(255 - channel for channel in background)
    image = Image.new("RGB", size, background)
    draw = ImageDraw.Draw(image)

    margin = max(24, min(width, height) // 12)
    draw.rectangle(
        (margin, margin, width - margin, height - margin),
        outline=accent,
        width=max(4, min(width, height) // 60),
    )

    step = max(16, min(width, height) // 8)
    for offset in range(-height, width, step):
        line_color = tuple((channel + index * 17) % 256 for channel in accent)
        draw.line((offset, height, offset + height, 0), fill=line_color, width=3)

    radius = min(width, height) // 6
    center_x = width // 2 + ((index % 3) - 1) * radius // 2
    center_y = height // 2 + (((index + 1) % 3) - 1) * radius // 2
    draw.ellipse(
        (center_x - radius, center_y - radius, center_x + radius, center_y + radius),
        fill=accent,
    )

    label = f"DEV {index:02d}"
    text_box = draw.textbbox((0, 0), label)
    text_width = text_box[2] - text_box[0]
    text_height = text_box[3] - text_box[1]
    draw.rectangle(
        (margin, height - margin - text_height - 16, margin + text_width + 16, height - margin),
        fill=(255, 255, 255),
    )
    draw.text((margin + 8, height - margin - text_height - 8), label, fill=(20, 20, 20))
    return image


def _seed_minio(paths: list[Path]) -> None:
    endpoint = os.environ.get("MINIO_ENDPOINT")
    access_key = os.environ.get("MINIO_ACCESS_KEY")
    secret_key = os.environ.get("MINIO_SECRET_KEY")
    bucket = os.environ.get("MINIO_BUCKET", "image-similarity-dev")
    prefix = os.environ.get("MINIO_PREFIX", "catalog").strip("/")
    secure = _bool(os.environ.get("MINIO_SECURE"), default=False)

    if not endpoint or not access_key or not secret_key:
        print("Skipping MinIO seed because MINIO_ENDPOINT, MINIO_ACCESS_KEY, or MINIO_SECRET_KEY is unset")
        return

    try:
        from minio import Minio
    except ImportError as exc:
        raise RuntimeError("MinIO seed requires the `minio` package") from exc

    client = Minio(endpoint, access_key=access_key, secret_key=secret_key, secure=secure)
    _wait_for_minio(client, bucket)

    if not client.bucket_exists(bucket):
        client.make_bucket(bucket)

    for path in paths:
        object_name = f"{prefix}/{path.name}" if prefix else path.name
        data = path.read_bytes()
        client.put_object(
            bucket,
            object_name,
            BytesIO(data),
            length=len(data),
            content_type=_content_type(path),
        )
    print(f"Uploaded {len(paths)} dummy images to minio://{bucket}/{prefix}")


def _wait_for_minio(client, bucket: str) -> None:
    deadline = time.monotonic() + _positive_int(os.environ.get("MINIO_WAIT_SECONDS"), 60)
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            client.bucket_exists(bucket)
            return
        except Exception as exc:  # noqa: BLE001 - readiness can fail with transport or S3 errors.
            last_error = exc
            time.sleep(1)
    raise RuntimeError(f"MinIO did not become ready: {last_error}")


def _content_type(path: Path) -> str:
    suffix = path.suffix.lower()
    if suffix in {".jpg", ".jpeg"}:
        return "image/jpeg"
    if suffix == ".png":
        return "image/png"
    if suffix == ".webp":
        return "image/webp"
    return "application/octet-stream"


def _image_size(value: str) -> tuple[int, int]:
    raw_width, separator, raw_height = value.lower().partition("x")
    if not separator:
        raise ValueError("DUMMY_IMAGE_SIZE must use WIDTHxHEIGHT format")
    return _positive_int(raw_width, 640), _positive_int(raw_height, 480)


def _positive_int(value: str | None, default: int | None = None) -> int:
    if value in {None, ""}:
        if default is None:
            raise ValueError("Expected a positive integer")
        return default
    parsed = int(value)
    if parsed < 1:
        raise ValueError("Expected a positive integer")
    return parsed


def _bool(value: str | None, *, default: bool) -> bool:
    if value is None or value == "":
        return default
    return value.lower() in {"1", "true", "yes", "on"}


if __name__ == "__main__":
    main()
