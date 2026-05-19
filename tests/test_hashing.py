from PIL import Image, ImageDraw
import pytest

from image_similarity._rust_backend import BACKEND
from image_similarity.hashing import hash_distance, is_near_duplicate, phash_image

pytestmark = pytest.mark.unit


def test_same_image_hash_distance_is_zero() -> None:
    image = Image.new("RGB", (64, 64), color="white")
    left = phash_image(image)
    right = phash_image(image.copy())

    assert hash_distance(left, right) == 0
    assert is_near_duplicate(left, right, max_distance=0)


def test_different_hashes_have_distance() -> None:
    left = "0000000000000000"
    right = "ffffffffffffffff"

    assert hash_distance(left, right) == 64
    assert not is_near_duplicate(left, right, max_distance=8)


def test_near_duplicate_threshold_is_inclusive() -> None:
    left = "0000000000000000"
    right = "0000000000000001"

    assert hash_distance(left, right) == 1
    assert is_near_duplicate(left, right, max_distance=1)
    assert not is_near_duplicate(left, right, max_distance=0)


def test_invalid_hash_input_raises_current_imagehash_error() -> None:
    with pytest.raises(Exception):
        hash_distance("not-a-hash", "0000000000000000")


def resized_luma(image: Image.Image) -> Image.Image:
    return image.convert("L").resize((32, 32), Image.Resampling.LANCZOS)


def test_rust_phash_matches_python_imagehash_for_resampled_luma() -> None:
    if BACKEND is None:
        pytest.skip("Rust backend is not built")

    import imagehash

    image = Image.new("RGB", (96, 72))
    pixels = image.load()
    for y in range(image.height):
        for x in range(image.width):
            pixels[x, y] = ((x * 3) % 256, (y * 5) % 256, ((x + y) * 2) % 256)
    luma_image = resized_luma(image)

    assert BACKEND.phash_luma(
        luma_image.tobytes(),
        luma_image.width,
        luma_image.height,
        8,
    ) == str(imagehash.phash(image))


def test_phash_image_uses_rust_backend_without_changing_hash() -> None:
    import imagehash

    image = Image.new("RGB", (80, 80), "white")
    draw = ImageDraw.Draw(image)
    for y in range(0, 80, 10):
        for x in range(0, 80, 10):
            if (x // 10 + y // 10) % 2:
                draw.rectangle([x, y, x + 9, y + 9], fill="black")

    assert phash_image(image) == str(imagehash.phash(image))


def test_rust_phash_rejects_invalid_luma_buffer() -> None:
    if BACKEND is None:
        pytest.skip("Rust backend is not built")

    with pytest.raises(ValueError, match="luma buffer length"):
        BACKEND.phash_luma(b"\x00\x00\x00", 2, 2, 8)
