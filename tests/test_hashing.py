from PIL import Image
import pytest

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
