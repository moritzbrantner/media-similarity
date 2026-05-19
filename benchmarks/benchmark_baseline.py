#!/usr/bin/env python
from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import sys
import tempfile
import time
from datetime import UTC, datetime
from pathlib import Path
from types import SimpleNamespace
from typing import Callable

from PIL import Image

ROOT_DIR = Path(__file__).resolve().parents[1]
SRC_DIR = ROOT_DIR / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from image_similarity.config import Settings
from image_similarity._rust_backend import is_available as rust_backend_available
from image_similarity.hashing import phash_image
from image_similarity.image_io import load_image
from image_similarity.indexer import ImageIndexer
from image_similarity.models import ImagePayload
from image_similarity.search import ImageSearchService
from image_similarity.sources import LocalFolderSource
from image_similarity.thumbnails import ensure_thumbnail


class SyntheticEmbedder:
    def __init__(self, vector_size: int) -> None:
        self.vector_size = vector_size

    def encode(self, image: Image.Image) -> list[float]:
        width, height = image.size
        base = float(width + height)
        return [base + float(index) for index in range(self.vector_size)]


class MemoryStore:
    def __init__(self, search_results: list[SimpleNamespace] | None = None) -> None:
        self.points: list[tuple[ImagePayload, list[float]]] = []
        self.search_results = search_results or []

    def ensure_collection(self) -> None:
        return None

    def upsert_image(self, payload: ImagePayload, vector: list[float]) -> None:
        self.points.append((payload, vector))

    def search(self, vector: list[float], limit: int) -> list[SimpleNamespace]:
        return self.search_results[:limit]

    def count(self) -> int:
        return len(self.points)


def make_image(index: int, size: int) -> Image.Image:
    color = ((index * 37) % 255, (index * 71) % 255, (index * 109) % 255)
    return Image.new("RGB", (size, size), color=color)


def write_images(source_dir: Path, count: int, size: int) -> list[Path]:
    source_dir.mkdir(parents=True, exist_ok=True)
    paths = []
    for index in range(count):
        path = source_dir / f"image-{index:04d}.jpg"
        make_image(index, size).save(path, format="JPEG", quality=90)
        paths.append(path)
    return paths


def summarize(samples_ms: list[float], operations_per_sample: int) -> dict[str, float]:
    mean_ms = statistics.fmean(samples_ms)
    stdev_ms = statistics.stdev(samples_ms) if len(samples_ms) > 1 else 0.0
    return {
        "min_ms": min(samples_ms),
        "median_ms": statistics.median(samples_ms),
        "mean_ms": mean_ms,
        "max_ms": max(samples_ms),
        "stdev_ms": stdev_ms,
        "ops_per_sec": operations_per_sample / (mean_ms / 1000.0) if mean_ms else 0.0,
    }


def measure(
    iterations: int,
    warmup: int,
    operations_per_sample: int,
    operation: Callable[[], None],
) -> dict[str, float]:
    for _ in range(warmup):
        operation()

    samples_ms = []
    for _ in range(iterations):
        start = time.perf_counter()
        operation()
        samples_ms.append((time.perf_counter() - start) * 1000.0)
    return summarize(samples_ms, operations_per_sample)


def benchmark_synthetic(args: argparse.Namespace) -> tuple[dict[str, dict[str, float]], list[str], dict[str, object]]:
    notes: list[str] = []
    with tempfile.TemporaryDirectory(prefix="image-sim-bench-") as temp_dir_name:
        temp_dir = Path(temp_dir_name)
        source_dir = temp_dir / "source"
        image_paths = write_images(source_dir, args.images, args.image_size)
        loaded_images = [load_image(path) for path in image_paths]
        settings = Settings(
            SOURCE_IMAGE_DIR=str(source_dir),
            THUMBNAIL_DIR=str(temp_dir / "thumbnails"),
            UPLOAD_DIR=str(temp_dir / "uploads"),
            QDRANT_COLLECTION="benchmark_synthetic",
            DEFAULT_SEARCH_LIMIT=min(12, args.images),
            DUPLICATE_HASH_DISTANCE=8,
            VECTOR_SIZE=args.vector_size,
        )
        embedder = SyntheticEmbedder(args.vector_size)
        source_images = list(LocalFolderSource(source_dir, settings.image_extensions).iter_images())
        payloads = [
            ImagePayload(
                id=f"00000000-0000-0000-0000-{index:012d}",
                path=str(path),
                relative_path=path.name,
                filename=path.name,
                width=args.image_size,
                height=args.image_size,
                size_bytes=path.stat().st_size,
                modified_at=path.stat().st_mtime,
                phash=phash_image(loaded_images[index]),
                thumbnail_url=f"/thumbnails/{index}.jpg",
            )
            for index, path in enumerate(image_paths)
        ]
        search_store = MemoryStore(
            [
                SimpleNamespace(payload=payload.model_dump(), score=1.0 - (index * 0.001))
                for index, payload in enumerate(payloads[: settings.default_search_limit])
            ]
        )

        thumbnail_iteration = {"value": 0}
        payload_iteration = {"value": 0}

        def generate_thumbnails() -> None:
            thumbnail_iteration["value"] += 1
            thumb_dir = temp_dir / f"thumbnail-run-{thumbnail_iteration['value']}"
            for index, image in enumerate(loaded_images):
                ensure_thumbnail(image, thumb_dir, f"image-{index}")

        def build_payloads() -> None:
            payload_iteration["value"] += 1
            local_settings = Settings(
                SOURCE_IMAGE_DIR=str(source_dir),
                THUMBNAIL_DIR=str(temp_dir / f"payload-run-{payload_iteration['value']}"),
                UPLOAD_DIR=str(temp_dir / "uploads"),
                QDRANT_COLLECTION="benchmark_synthetic",
                VECTOR_SIZE=args.vector_size,
            )
            indexer = ImageIndexer(local_settings, MemoryStore(), embedder)
            for source_image, image in zip(source_images, loaded_images, strict=True):
                indexer._build_payload(source_image, image)

        def synthetic_index() -> None:
            local_settings = Settings(
                SOURCE_IMAGE_DIR=str(source_dir),
                THUMBNAIL_DIR=str(temp_dir / f"index-run-{time.perf_counter_ns()}"),
                UPLOAD_DIR=str(temp_dir / "uploads"),
                QDRANT_COLLECTION="benchmark_synthetic",
                VECTOR_SIZE=args.vector_size,
            )
            ImageIndexer(local_settings, MemoryStore(), embedder).index_source_dir()

        def assemble_search_response() -> None:
            ImageSearchService(settings, search_store, embedder).search_image(loaded_images[0])

        metrics = {
            "image_loading": measure(args.iterations, args.warmup, args.images, lambda: [load_image(path) for path in image_paths]),
            "phash": measure(args.iterations, args.warmup, args.images, lambda: [phash_image(image) for image in loaded_images]),
            "thumbnail_generation": measure(args.iterations, args.warmup, args.images, generate_thumbnails),
            "payload_building": measure(args.iterations, args.warmup, args.images, build_payloads),
            "search_response_assembly": measure(
                args.iterations,
                args.warmup,
                settings.default_search_limit,
                assemble_search_response,
            ),
            "synthetic_indexing": measure(args.iterations, args.warmup, args.images, synthetic_index),
        }

    settings_payload = {
        "images": args.images,
        "image_size": args.image_size,
        "iterations": args.iterations,
        "warmup": args.warmup,
        "vector_size": args.vector_size,
    }
    return metrics, notes, settings_payload


def benchmark_real(args: argparse.Namespace) -> tuple[dict[str, dict[str, float]], list[str], dict[str, object]]:
    notes: list[str] = []
    try:
        from image_similarity.embeddings import ImageEmbedder
        from image_similarity.qdrant_store import QdrantImageStore
    except ImportError as exc:
        notes.append(f"Skipped real benchmark because a dependency is unavailable: {exc}")
        return {}, notes, {}

    qdrant_url = os.environ.get("QDRANT_URL", "http://localhost:6333")
    model_name = os.environ.get("CLIP_MODEL_NAME", "sentence-transformers/clip-ViT-B-32")
    collection = os.environ.get("QDRANT_COLLECTION", "image_similarity_benchmark")

    with tempfile.TemporaryDirectory(prefix="image-sim-real-bench-") as temp_dir_name:
        temp_dir = Path(temp_dir_name)
        source_dir = temp_dir / "source"
        image_paths = write_images(source_dir, args.images, args.image_size)
        images = [load_image(path) for path in image_paths]
        settings = Settings(
            SOURCE_IMAGE_DIR=str(source_dir),
            THUMBNAIL_DIR=str(temp_dir / "thumbnails"),
            UPLOAD_DIR=str(temp_dir / "uploads"),
            QDRANT_URL=qdrant_url,
            QDRANT_COLLECTION=collection,
            CLIP_MODEL_NAME=model_name,
            VECTOR_SIZE=args.vector_size,
        )

        try:
            embedder = ImageEmbedder(model_name)
            store = QdrantImageStore(qdrant_url, collection, args.vector_size)
            store.ensure_collection()
        except Exception as exc:  # noqa: BLE001 - report optional environment failures as benchmark notes.
            notes.append(f"Skipped real benchmark because Qdrant or the model could not initialize: {exc}")
            return {}, notes, {"model_name": model_name, "qdrant_url": qdrant_url}

        metrics = {
            "embedder_cold_load": measure(args.iterations, args.warmup, 1, lambda: ImageEmbedder(model_name).dimension),
            "embedding": measure(args.iterations, args.warmup, args.images, lambda: [embedder.encode(image) for image in images]),
            "real_indexing": measure(args.iterations, args.warmup, args.images, lambda: ImageIndexer(settings, store, embedder).index_source_dir()),
            "qdrant_search": measure(args.iterations, args.warmup, 1, lambda: store.search(embedder.encode(images[0]), 12)),
        }

    settings_payload = {
        "images": args.images,
        "image_size": args.image_size,
        "iterations": args.iterations,
        "warmup": args.warmup,
        "vector_size": args.vector_size,
        "model_name": model_name,
        "qdrant_url": qdrant_url,
        "collection": collection,
    }
    return metrics, notes, settings_payload


def build_report(args: argparse.Namespace) -> dict[str, object]:
    if args.profile == "synthetic":
        metrics, notes, settings_payload = benchmark_synthetic(args)
    else:
        metrics, notes, settings_payload = benchmark_real(args)

    return {
        "timestamp_utc": datetime.now(UTC).isoformat(),
        "profile": args.profile,
        "python_version": sys.version,
        "platform": platform.platform(),
        "cpu_count": os.cpu_count(),
        "rust_backend_available": rust_backend_available(),
        "settings": settings_payload,
        "metrics": metrics,
        "notes": notes,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Baseline benchmark for image-similarity-service.")
    parser.add_argument("--profile", choices=["synthetic", "real"], default="synthetic")
    parser.add_argument("--images", type=int, default=100)
    parser.add_argument("--image-size", type=int, default=256)
    parser.add_argument("--iterations", type=int, default=5)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--vector-size", type=int, default=512)
    parser.add_argument("--output", type=Path, default=Path("benchmarks/results/latest.json"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    report = build_report(args)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
