from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Callable, Iterable
from urllib.parse import parse_qs, unquote, urlparse

from PIL import Image

from image_similarity.config import Settings
from image_similarity.image_io import iter_image_paths, load_image, load_image_bytes, relative_path


class SourceUnavailable(RuntimeError):
    pass


@dataclass(frozen=True)
class SourceImage:
    source_type: str
    source_uri: str
    item_uri: str
    id_base: str
    display_path: str
    relative_path: str
    filename: str
    size_bytes: int
    modified_at: float
    _loader: Callable[[], Image.Image]

    def load_image(self) -> Image.Image:
        return self._loader()


class ImageSource:
    source_type = "source"

    @property
    def uri(self) -> str:
        raise NotImplementedError

    def iter_images(self) -> Iterable[SourceImage]:
        raise NotImplementedError


class UnavailableSource(ImageSource):
    source_type = "unavailable"

    def __init__(self, uri: str, error: str) -> None:
        self._uri = uri
        self.error = error

    @property
    def uri(self) -> str:
        return self._uri

    def iter_images(self) -> Iterable[SourceImage]:
        raise SourceUnavailable(self.error)


class LocalFolderSource(ImageSource):
    source_type = "local"

    def __init__(self, root: Path, extensions: Iterable[str]) -> None:
        self.root = root
        self.extensions = extensions

    @property
    def uri(self) -> str:
        return str(self.root)

    def iter_images(self) -> Iterable[SourceImage]:
        if not self.root.exists():
            raise SourceUnavailable(f"Source directory does not exist: {self.root}")

        for path in iter_image_paths(self.root, self.extensions):
            stat = path.stat()
            resolved = path.resolve()
            yield SourceImage(
                source_type=self.source_type,
                source_uri=self.uri,
                item_uri=str(resolved),
                id_base=str(resolved),
                display_path=str(resolved),
                relative_path=relative_path(path, self.root),
                filename=path.name,
                size_bytes=stat.st_size,
                modified_at=stat.st_mtime,
                _loader=lambda path=path: load_image(path),
            )


class MinioSource(ImageSource):
    source_type = "minio"

    def __init__(
        self,
        *,
        endpoint: str,
        bucket: str,
        prefix: str,
        access_key: str,
        secret_key: str,
        secure: bool,
        extensions: Iterable[str],
        client: object | None = None,
    ) -> None:
        self.endpoint = endpoint
        self.bucket = bucket
        self.prefix = prefix.strip("/")
        self.access_key = access_key
        self.secret_key = secret_key
        self.secure = secure
        self.extensions = {extension.lower() for extension in extensions}
        self._client = client

    @property
    def uri(self) -> str:
        suffix = f"/{self.prefix}" if self.prefix else ""
        return f"minio://{self.bucket}{suffix}"

    @property
    def client(self) -> object:
        if self._client is None:
            try:
                from minio import Minio
            except ImportError as exc:
                raise SourceUnavailable("MinIO sources require the `minio` package") from exc
            self._client = Minio(
                self.endpoint,
                access_key=self.access_key,
                secret_key=self.secret_key,
                secure=self.secure,
            )
        return self._client

    def iter_images(self) -> Iterable[SourceImage]:
        prefix = f"{self.prefix}/" if self.prefix else ""
        try:
            objects = self.client.list_objects(self.bucket, prefix=prefix, recursive=True)
        except Exception as exc:  # noqa: BLE001 - surface storage setup errors as source failures.
            raise SourceUnavailable(f"Could not list MinIO source {self.uri}: {exc}") from exc

        for obj in objects:
            object_name = getattr(obj, "object_name", "")
            if not object_name or Path(object_name).suffix.lower() not in self.extensions:
                continue
            size = int(getattr(obj, "size", 0) or 0)
            last_modified = getattr(obj, "last_modified", None)
            modified_at = _timestamp(last_modified)
            relative = object_name[len(prefix) :] if prefix and object_name.startswith(prefix) else object_name
            item_uri = f"minio://{self.bucket}/{object_name}"
            yield SourceImage(
                source_type=self.source_type,
                source_uri=self.uri,
                item_uri=item_uri,
                id_base=item_uri,
                display_path=item_uri,
                relative_path=relative,
                filename=Path(object_name).name,
                size_bytes=size,
                modified_at=modified_at,
                _loader=lambda object_name=object_name: self._load_object(object_name),
            )

    def _load_object(self, object_name: str) -> Image.Image:
        try:
            response = self.client.get_object(self.bucket, object_name)
            try:
                return load_image_bytes(response.read())
            finally:
                response.close()
                response.release_conn()
        except Exception as exc:  # noqa: BLE001 - include object URI in index errors.
            raise RuntimeError(f"Could not load minio://{self.bucket}/{object_name}: {exc}") from exc


class VideoFileSource(ImageSource):
    source_type = "video"

    def __init__(
        self,
        path: Path,
        *,
        every_n_frames: int,
        max_frames: int | None,
    ) -> None:
        self.path = path
        self.every_n_frames = every_n_frames
        self.max_frames = max_frames

    @property
    def uri(self) -> str:
        return f"video://{self.path.resolve()}"

    def iter_images(self) -> Iterable[SourceImage]:
        if not self.path.exists():
            raise SourceUnavailable(f"Video source does not exist: {self.path}")

        capture = _open_video_capture(str(self.path))
        if not capture.isOpened():
            capture.release()
            raise SourceUnavailable(f"Could not open video source: {self.path}")

        stat = self.path.stat()
        yielded = 0
        frame_index = 0
        try:
            while True:
                ok, frame = capture.read()
                if not ok:
                    break
                if frame_index % self.every_n_frames == 0:
                    image = _image_from_cv2_frame(frame)
                    item_uri = f"{self.uri}#frame={frame_index}"
                    filename = f"{self.path.name}-frame-{frame_index:06d}.jpg"
                    yield SourceImage(
                        source_type=self.source_type,
                        source_uri=self.uri,
                        item_uri=item_uri,
                        id_base=item_uri,
                        display_path=item_uri,
                        relative_path=filename,
                        filename=filename,
                        size_bytes=stat.st_size,
                        modified_at=stat.st_mtime,
                        _loader=lambda image=image: image.copy(),
                    )
                    yielded += 1
                    if self.max_frames is not None and yielded >= self.max_frames:
                        break
                frame_index += 1
        finally:
            capture.release()


class CameraStreamSource(ImageSource):
    source_type = "camera"

    def __init__(self, target: str | int, *, every_n_frames: int, max_frames: int) -> None:
        self.target = target
        self.every_n_frames = every_n_frames
        self.max_frames = max_frames

    @property
    def uri(self) -> str:
        return f"camera://{self.target}"

    def iter_images(self) -> Iterable[SourceImage]:
        capture = _open_video_capture(self.target)
        if not capture.isOpened():
            capture.release()
            raise SourceUnavailable(f"Could not open camera source: {self.target}")

        yielded = 0
        frame_index = 0
        try:
            while yielded < self.max_frames:
                ok, frame = capture.read()
                if not ok:
                    break
                if frame_index % self.every_n_frames == 0:
                    image = _image_from_cv2_frame(frame)
                    item_uri = f"{self.uri}#frame={frame_index}"
                    filename = f"camera-{self.target}-frame-{frame_index:06d}.jpg"
                    yield SourceImage(
                        source_type=self.source_type,
                        source_uri=self.uri,
                        item_uri=item_uri,
                        id_base=item_uri,
                        display_path=item_uri,
                        relative_path=filename,
                        filename=filename,
                        size_bytes=0,
                        modified_at=0.0,
                        _loader=lambda image=image: image.copy(),
                    )
                    yielded += 1
                frame_index += 1
        finally:
            capture.release()


def build_image_sources(settings: Settings) -> list[ImageSource]:
    specs = settings.image_sources or [str(settings.source_image_dir)]
    sources: list[ImageSource] = []
    for spec in specs:
        try:
            sources.append(_source_from_spec(spec, settings))
        except SourceUnavailable as exc:
            sources.append(UnavailableSource(spec, str(exc)))
    return sources


def _source_from_spec(spec: str, settings: Settings) -> ImageSource:
    parsed = urlparse(spec)
    scheme = parsed.scheme.lower()

    if scheme in {"", "file", "local"}:
        return LocalFolderSource(_local_path_from_url(spec), settings.image_extensions)
    if scheme == "minio":
        return _minio_source_from_url(parsed, settings)
    if scheme == "video":
        query = _query(parsed)
        return VideoFileSource(
            _path_from_url_parts(parsed),
            every_n_frames=_positive_int(query.get("every_n_frames"), settings.video_frame_stride),
            max_frames=_optional_positive_int(query.get("max_frames"), settings.video_max_frames),
        )
    if scheme == "camera":
        query = _query(parsed)
        return CameraStreamSource(
            _camera_target(parsed, query),
            every_n_frames=_positive_int(query.get("every_n_frames"), settings.camera_frame_stride),
            max_frames=_positive_int(query.get("max_frames"), settings.camera_max_frames),
        )

    path = Path(spec)
    if path.exists() or not scheme:
        return LocalFolderSource(path, settings.image_extensions)
    raise SourceUnavailable(f"Unsupported image source: {spec}")


def _minio_source_from_url(parsed, settings: Settings) -> MinioSource:
    query = _query(parsed)
    bucket = parsed.netloc
    prefix = unquote(parsed.path.lstrip("/"))
    if not bucket:
        raise SourceUnavailable("MinIO source must include a bucket name")

    endpoint = query.get("endpoint") or settings.minio_endpoint
    access_key = query.get("access_key") or settings.minio_access_key
    secret_key = query.get("secret_key") or settings.minio_secret_key
    secure = _bool(query.get("secure"), settings.minio_secure)
    if not endpoint or not access_key or not secret_key:
        raise SourceUnavailable("MinIO sources require endpoint, access key, and secret key")

    return MinioSource(
        endpoint=endpoint,
        bucket=bucket,
        prefix=prefix,
        access_key=access_key,
        secret_key=secret_key,
        secure=secure,
        extensions=settings.image_extensions,
    )


def _open_video_capture(target: str | int):
    try:
        import cv2
    except ImportError as exc:
        raise SourceUnavailable("Video and camera sources require the `opencv-python-headless` package") from exc
    return cv2.VideoCapture(target)


def _image_from_cv2_frame(frame) -> Image.Image:
    import cv2

    rgb = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
    return Image.fromarray(rgb).convert("RGB")


def _query(parsed) -> dict[str, str]:
    return {key: values[-1] for key, values in parse_qs(parsed.query).items() if values}


def _local_path_from_url(spec: str) -> Path:
    parsed = urlparse(spec)
    if parsed.scheme == "file":
        return _path_from_url_parts(parsed)
    if parsed.scheme == "local":
        return _path_from_url_parts(parsed)
    return Path(spec)


def _path_from_url_parts(parsed) -> Path:
    if parsed.netloc and parsed.path:
        return Path(unquote(f"/{parsed.netloc}{parsed.path}"))
    if parsed.netloc:
        return Path(unquote(parsed.netloc))
    return Path(unquote(parsed.path))


def _camera_target(parsed, query: dict[str, str]) -> str | int:
    target = query.get("url") or parsed.netloc or parsed.path.lstrip("/")
    if not target:
        target = "0"
    return int(target) if target.isdigit() else target


def _positive_int(value: str | None, default: int) -> int:
    if value is None or value == "":
        return default
    try:
        parsed = int(value)
    except ValueError as exc:
        raise SourceUnavailable("Frame options must be positive integers") from exc
    if parsed < 1:
        raise SourceUnavailable("Frame options must be positive integers")
    return parsed


def _optional_positive_int(value: str | None, default: int | None) -> int | None:
    if value is None or value == "":
        return default
    return _positive_int(value, 1)


def _bool(value: str | None, default: bool) -> bool:
    if value is None:
        return default
    return value.lower() in {"1", "true", "yes", "on"}


def _timestamp(value: object) -> float:
    if isinstance(value, datetime):
        return value.timestamp()
    return 0.0
