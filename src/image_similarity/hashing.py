from PIL import Image
import imagehash

from image_similarity._rust_backend import BACKEND

PHASH_SIZE = 8
PHASH_HIGHFREQ_FACTOR = 4


def phash_image(image: Image.Image) -> str:
    """Return an image pHash as a compact hexadecimal string."""
    if BACKEND is not None:
        try:
            image_size = PHASH_SIZE * PHASH_HIGHFREQ_FACTOR
            luma_image = image.convert("L").resize(
                (image_size, image_size),
                Image.Resampling.LANCZOS,
            )
            return str(
                BACKEND.phash_luma(
                    luma_image.tobytes(),
                    luma_image.width,
                    luma_image.height,
                    PHASH_SIZE,
                )
            )
        except Exception:
            pass
    return str(imagehash.phash(image))


def hash_distance(left: str, right: str) -> int:
    """Return the Hamming distance between two imagehash hex strings."""
    if BACKEND is not None:
        return int(BACKEND.hash_distance(left, right))
    return imagehash.hex_to_hash(left) - imagehash.hex_to_hash(right)


def is_near_duplicate(left: str, right: str, max_distance: int) -> bool:
    return hash_distance(left, right) <= max_distance
