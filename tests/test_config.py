from pathlib import Path

import pytest
from pydantic import ValidationError

from image_similarity.config import Settings

pytestmark = pytest.mark.unit


def test_extensions_are_normalized() -> None:
    settings = Settings(IMAGE_EXTENSIONS="jpg, .PNG,webp")

    assert settings.image_extensions == {".jpg", ".png", ".webp"}


def test_source_dir_can_be_overridden() -> None:
    settings = Settings(SOURCE_IMAGE_DIR="/tmp/source-images")

    assert settings.source_image_dir == Path("/tmp/source-images")


def test_extensions_accept_iterables() -> None:
    from_list = Settings(IMAGE_EXTENSIONS=["jpg", ".PNG"])
    from_set = Settings(IMAGE_EXTENSIONS={"webp", ".JPEG"})

    assert from_list.image_extensions == {".jpg", ".png"}
    assert from_set.image_extensions == {".jpeg", ".webp"}


def test_image_sources_accept_delimited_strings_and_json() -> None:
    delimited = Settings(IMAGE_SOURCES="local:///images;minio://bucket/prefix")
    json_sources = Settings(IMAGE_SOURCES='["/images", "video:///clips/demo.mp4"]')

    assert delimited.image_sources == ["local:///images", "minio://bucket/prefix"]
    assert json_sources.image_sources == ["/images", "video:///clips/demo.mp4"]


def test_empty_extensions_are_rejected() -> None:
    with pytest.raises(ValidationError, match="At least one image extension"):
        Settings(IMAGE_EXTENSIONS=" , ")


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("DEFAULT_SEARCH_LIMIT", 0),
        ("DEFAULT_SEARCH_LIMIT", 101),
        ("DUPLICATE_HASH_DISTANCE", -1),
        ("DUPLICATE_HASH_DISTANCE", 65),
        ("MAX_UPLOAD_MB", 0),
        ("MAX_UPLOAD_MB", 201),
        ("VECTOR_SIZE", 0),
    ],
)
def test_numeric_bounds_are_validated(field: str, value: int) -> None:
    with pytest.raises(ValidationError):
        Settings(**{field: value})
