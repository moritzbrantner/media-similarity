from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path
from types import SimpleNamespace
from typing import Callable

import pytest
from PIL import Image

from image_similarity import sources as sources_module
from image_similarity.sources import (
    CameraStreamSource,
    LocalFolderSource,
    MinioSource,
    SourceUnavailable,
    UnavailableSource,
    VideoFileSource,
    build_image_sources,
)

pytestmark = pytest.mark.unit


def test_build_sources_defaults_to_source_image_dir(temp_settings) -> None:
    sources = build_image_sources(temp_settings)

    assert len(sources) == 1
    assert isinstance(sources[0], LocalFolderSource)
    assert sources[0].uri == str(temp_settings.source_image_dir)


def test_local_folder_source_yields_metadata_and_loads_images(temp_settings, image_bytes: Callable[..., bytes]) -> None:
    source = temp_settings.source_image_dir
    source.mkdir()
    image_path = source / "sample.jpg"
    image_path.write_bytes(image_bytes(image_format="JPEG"))

    items = list(LocalFolderSource(source, temp_settings.image_extensions).iter_images())

    assert len(items) == 1
    item = items[0]
    assert item.source_type == "local"
    assert item.relative_path == "sample.jpg"
    assert item.size_bytes == image_path.stat().st_size
    assert item.load_image().mode == "RGB"


def test_build_sources_accepts_local_file_and_plain_paths(temp_settings, tmp_path: Path) -> None:
    local_path = tmp_path / "local"
    file_path = tmp_path / "file"
    plain_path = tmp_path / "plain"
    settings = temp_settings.model_copy(
        update={
            "image_sources": [
                f"local://{local_path}",
                file_path.as_uri(),
                str(plain_path),
            ],
        },
    )

    sources = build_image_sources(settings)

    assert [source.uri for source in sources] == [str(local_path), str(file_path), str(plain_path)]
    assert all(isinstance(source, LocalFolderSource) for source in sources)


def test_missing_local_folder_is_unavailable(temp_settings) -> None:
    source = LocalFolderSource(temp_settings.source_image_dir, temp_settings.image_extensions)

    with pytest.raises(SourceUnavailable, match="Source directory does not exist"):
        list(source.iter_images())


def test_build_sources_supports_video_camera_and_unavailable_minio(temp_settings) -> None:
    settings = temp_settings.model_copy(
        update={
            "image_sources": [
                "video:///tmp/demo.mp4?every_n_frames=12&max_frames=4",
                "camera://0?every_n_frames=3&max_frames=2",
                "minio://missing-creds/images",
            ],
        },
    )

    sources = build_image_sources(settings)

    assert isinstance(sources[0], VideoFileSource)
    assert sources[0].every_n_frames == 12
    assert sources[0].max_frames == 4
    assert isinstance(sources[1], CameraStreamSource)
    assert sources[1].every_n_frames == 3
    assert sources[1].max_frames == 2
    assert isinstance(sources[2], UnavailableSource)


def test_build_sources_uses_global_minio_settings(temp_settings) -> None:
    settings = temp_settings.model_copy(
        update={
            "image_sources": ["minio://images/catalog"],
            "minio_endpoint": "minio:9000",
            "minio_access_key": "access",
            "minio_secret_key": "secret",
            "minio_secure": False,
        },
    )

    sources = build_image_sources(settings)

    assert len(sources) == 1
    source = sources[0]
    assert isinstance(source, MinioSource)
    assert source.endpoint == "minio:9000"
    assert source.bucket == "images"
    assert source.prefix == "catalog"
    assert source.access_key == "access"
    assert source.secret_key == "secret"
    assert source.secure is False


def test_build_sources_allows_minio_query_overrides(temp_settings) -> None:
    settings = temp_settings.model_copy(
        update={
            "image_sources": [
                "minio://images/catalog?endpoint=other:9000&access_key=override&secret_key=override-secret&secure=false",
            ],
            "minio_endpoint": "minio:9000",
            "minio_access_key": "access",
            "minio_secret_key": "secret",
            "minio_secure": True,
        },
    )

    sources = build_image_sources(settings)

    source = sources[0]
    assert isinstance(source, MinioSource)
    assert source.endpoint == "other:9000"
    assert source.access_key == "override"
    assert source.secret_key == "override-secret"
    assert source.secure is False


def test_build_sources_reports_bad_source_specs_without_raising(temp_settings) -> None:
    settings = temp_settings.model_copy(
        update={
            "image_sources": [
                "s3://bucket/images",
                "camera://0?every_n_frames=none",
                "minio:///missing-bucket?endpoint=minio:9000&access_key=a&secret_key=s",
            ],
        },
    )

    sources = build_image_sources(settings)

    assert all(isinstance(source, UnavailableSource) for source in sources)
    assert "Unsupported image source" in sources[0].error
    assert "Frame options must be positive integers" in sources[1].error
    assert "bucket name" in sources[2].error


def test_minio_source_filters_objects_and_loads_image(image_bytes: Callable[..., bytes]) -> None:
    last_modified = datetime(2026, 1, 2, 3, 4, 5, tzinfo=timezone.utc)

    class FakeResponse:
        def __init__(self) -> None:
            self.closed = False
            self.released = False

        def read(self) -> bytes:
            return image_bytes()

        def close(self) -> None:
            self.closed = True

        def release_conn(self) -> None:
            self.released = True

    class FakeClient:
        def __init__(self) -> None:
            self.response = FakeResponse()

        def list_objects(self, bucket: str, prefix: str, recursive: bool):
            assert bucket == "images"
            assert prefix == "catalog/"
            assert recursive is True
            return [
                SimpleNamespace(object_name="catalog/a.jpg", size=123, last_modified=last_modified),
                SimpleNamespace(object_name="catalog/b.PNG", size=456, last_modified=None),
                SimpleNamespace(object_name="catalog/notes.txt", size=3, last_modified=None),
            ]

        def get_object(self, bucket: str, object_name: str) -> FakeResponse:
            assert bucket == "images"
            assert object_name == "catalog/a.jpg"
            return self.response

    client = FakeClient()

    source = MinioSource(
        endpoint="localhost:9000",
        bucket="images",
        prefix="catalog",
        access_key="minio",
        secret_key="secret",
        secure=False,
        extensions={".jpg", ".png"},
        client=client,
    )

    items = list(source.iter_images())

    assert len(items) == 2
    assert items[0].item_uri == "minio://images/catalog/a.jpg"
    assert items[0].relative_path == "a.jpg"
    assert items[0].filename == "a.jpg"
    assert items[0].source_type == "minio"
    assert items[0].source_uri == "minio://images/catalog"
    assert items[0].size_bytes == 123
    assert items[0].modified_at == last_modified.timestamp()
    assert items[0].load_image().size == (64, 48)
    assert client.response.closed is True
    assert client.response.released is True
    assert items[1].relative_path == "b.PNG"


def test_minio_source_reports_listing_and_loading_errors(image_bytes: Callable[..., bytes]) -> None:
    class ListFailureClient:
        def list_objects(self, bucket: str, prefix: str, recursive: bool):
            raise RuntimeError("offline")

    source = MinioSource(
        endpoint="localhost:9000",
        bucket="images",
        prefix="catalog",
        access_key="minio",
        secret_key="secret",
        secure=False,
        extensions={".jpg"},
        client=ListFailureClient(),
    )

    with pytest.raises(SourceUnavailable, match="Could not list MinIO source minio://images/catalog"):
        list(source.iter_images())

    class LoadFailureClient:
        def list_objects(self, bucket: str, prefix: str, recursive: bool):
            return [SimpleNamespace(object_name="catalog/a.jpg", size=123, last_modified=None)]

        def get_object(self, bucket: str, object_name: str):
            raise RuntimeError("permission denied")

    source = MinioSource(
        endpoint="localhost:9000",
        bucket="images",
        prefix="catalog",
        access_key="minio",
        secret_key="secret",
        secure=False,
        extensions={".jpg"},
        client=LoadFailureClient(),
    )
    item = next(iter(source.iter_images()))

    with pytest.raises(RuntimeError, match="permission denied"):
        item.load_image()


def test_video_file_source_samples_frames_and_releases_capture(temp_settings, tmp_path: Path, monkeypatch) -> None:
    frames = [
        Image.new("RGB", (10 + index, 20), color=(index, 0, 0))
        for index in range(6)
    ]
    capture = FakeCapture(frames)
    video_path = tmp_path / "demo.mp4"
    video_path.write_bytes(b"video")
    monkeypatch.setattr(sources_module, "_open_video_capture", lambda target: capture)
    monkeypatch.setattr(sources_module, "_image_from_cv2_frame", lambda frame: frame)

    source = VideoFileSource(video_path, every_n_frames=2, max_frames=2)
    items = list(source.iter_images())

    assert capture.target is None
    assert capture.released is True
    assert [item.item_uri for item in items] == [
        f"video://{video_path.resolve()}#frame=0",
        f"video://{video_path.resolve()}#frame=2",
    ]
    assert [item.filename for item in items] == [
        "demo.mp4-frame-000000.jpg",
        "demo.mp4-frame-000002.jpg",
    ]
    assert items[0].source_type == "video"
    assert items[0].size_bytes == len(b"video")
    loaded = items[0].load_image()
    assert loaded.size == (10, 20)
    assert loaded is not frames[0]


def test_video_file_source_reports_missing_and_unopenable_sources(tmp_path: Path, monkeypatch) -> None:
    missing = tmp_path / "missing.mp4"
    with pytest.raises(SourceUnavailable, match="Video source does not exist"):
        list(VideoFileSource(missing, every_n_frames=1, max_frames=None).iter_images())

    capture = FakeCapture([], opened=False)
    video_path = tmp_path / "demo.mp4"
    video_path.write_bytes(b"video")
    monkeypatch.setattr(sources_module, "_open_video_capture", lambda target: capture)

    with pytest.raises(SourceUnavailable, match="Could not open video source"):
        list(VideoFileSource(video_path, every_n_frames=1, max_frames=None).iter_images())
    assert capture.released is True


def test_camera_stream_source_samples_frames_and_stops_at_max(monkeypatch) -> None:
    frames = [
        Image.new("RGB", (10 + index, 20), color=(index, 0, 0))
        for index in range(8)
    ]
    capture = FakeCapture(frames)
    monkeypatch.setattr(sources_module, "_open_video_capture", lambda target: capture.with_target(target))
    monkeypatch.setattr(sources_module, "_image_from_cv2_frame", lambda frame: frame)

    source = CameraStreamSource(0, every_n_frames=3, max_frames=2)
    items = list(source.iter_images())

    assert capture.target == 0
    assert capture.released is True
    assert [item.item_uri for item in items] == ["camera://0#frame=0", "camera://0#frame=3"]
    assert [item.filename for item in items] == [
        "camera-0-frame-000000.jpg",
        "camera-0-frame-000003.jpg",
    ]
    assert items[0].source_type == "camera"
    assert items[0].size_bytes == 0
    assert items[0].modified_at == 0.0


def test_camera_stream_source_reports_unopenable_sources(monkeypatch) -> None:
    capture = FakeCapture([], opened=False)
    monkeypatch.setattr(sources_module, "_open_video_capture", lambda target: capture.with_target(target))

    with pytest.raises(SourceUnavailable, match="Could not open camera source"):
        list(CameraStreamSource("rtsp://camera/stream", every_n_frames=1, max_frames=1).iter_images())
    assert capture.target == "rtsp://camera/stream"
    assert capture.released is True


class FakeCapture:
    def __init__(self, frames: list[Image.Image], opened: bool = True) -> None:
        self.frames = list(frames)
        self.opened = opened
        self.released = False
        self.target = None

    def with_target(self, target):
        self.target = target
        return self

    def isOpened(self) -> bool:
        return self.opened

    def read(self):
        if not self.frames:
            return False, None
        return True, self.frames.pop(0)

    def release(self) -> None:
        self.released = True
