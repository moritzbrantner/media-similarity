import json
from functools import lru_cache
from pathlib import Path
from typing import Any

from pydantic import Field, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=".env", env_file_encoding="utf-8", extra="ignore")

    source_image_dir: Path = Field(default=Path("/images"), validation_alias="SOURCE_IMAGE_DIR")
    qdrant_url: str = Field(default="http://qdrant:6333", validation_alias="QDRANT_URL")
    qdrant_collection: str = Field(default="image_similarity", validation_alias="QDRANT_COLLECTION")
    clip_model_name: str = Field(
        default="sentence-transformers/clip-ViT-B-32",
        validation_alias="CLIP_MODEL_NAME",
    )
    thumbnail_dir: Path = Field(default=Path("data/thumbnails"), validation_alias="THUMBNAIL_DIR")
    upload_dir: Path = Field(default=Path("data/uploads"), validation_alias="UPLOAD_DIR")
    image_extensions: set[str] = Field(
        default={".jpg", ".jpeg", ".png", ".webp", ".bmp", ".tif", ".tiff"},
        validation_alias="IMAGE_EXTENSIONS",
    )
    image_sources: list[str] = Field(default_factory=list, validation_alias="IMAGE_SOURCES")
    minio_endpoint: str | None = Field(default=None, validation_alias="MINIO_ENDPOINT")
    minio_access_key: str | None = Field(default=None, validation_alias="MINIO_ACCESS_KEY")
    minio_secret_key: str | None = Field(default=None, validation_alias="MINIO_SECRET_KEY")
    minio_secure: bool = Field(default=True, validation_alias="MINIO_SECURE")
    video_frame_stride: int = Field(default=30, ge=1, validation_alias="VIDEO_FRAME_STRIDE")
    video_max_frames: int | None = Field(default=None, ge=1, validation_alias="VIDEO_MAX_FRAMES")
    camera_frame_stride: int = Field(default=30, ge=1, validation_alias="CAMERA_FRAME_STRIDE")
    camera_max_frames: int = Field(default=100, ge=1, validation_alias="CAMERA_MAX_FRAMES")
    default_search_limit: int = Field(default=12, ge=1, le=100, validation_alias="DEFAULT_SEARCH_LIMIT")
    duplicate_hash_distance: int = Field(default=8, ge=0, le=64, validation_alias="DUPLICATE_HASH_DISTANCE")
    max_upload_mb: int = Field(default=20, ge=1, le=200, validation_alias="MAX_UPLOAD_MB")
    vector_size: int = Field(default=512, ge=1, validation_alias="VECTOR_SIZE")

    @field_validator("image_extensions", mode="before")
    @classmethod
    def parse_extensions(cls, value: str | list[str] | set[str]) -> set[str]:
        if isinstance(value, str):
            parts = [part.strip() for part in value.split(",")]
        else:
            parts = [str(part).strip() for part in value]

        extensions = {
            part.lower() if part.startswith(".") else f".{part.lower()}"
            for part in parts
            if part
        }
        if not extensions:
            raise ValueError("At least one image extension is required")
        return extensions

    @field_validator("image_sources", mode="before")
    @classmethod
    def parse_image_sources(cls, value: Any) -> list[str]:
        if value is None or value == "":
            return []
        if isinstance(value, str):
            stripped = value.strip()
            if not stripped:
                return []
            if stripped.startswith("["):
                parsed = json.loads(stripped)
                return [str(part).strip() for part in parsed if str(part).strip()]
            separators = ["\n", ";", ","]
            parts = [stripped]
            for separator in separators:
                if separator in stripped:
                    parts = stripped.split(separator)
                    break
            return [part.strip() for part in parts if part.strip()]
        return [str(part).strip() for part in value if str(part).strip()]

    @field_validator("minio_endpoint", "minio_access_key", "minio_secret_key", "video_max_frames", mode="before")
    @classmethod
    def empty_string_is_none(cls, value: Any) -> Any:
        if value == "":
            return None
        return value


@lru_cache
def get_settings() -> Settings:
    return Settings()
