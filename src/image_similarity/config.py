from functools import lru_cache
from pathlib import Path

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


@lru_cache
def get_settings() -> Settings:
    return Settings()
