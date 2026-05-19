from PIL import Image
import imagehash


def phash_image(image: Image.Image) -> str:
    """Return an image pHash as a compact hexadecimal string."""
    return str(imagehash.phash(image))


def hash_distance(left: str, right: str) -> int:
    """Return the Hamming distance between two imagehash hex strings."""
    return imagehash.hex_to_hash(left) - imagehash.hex_to_hash(right)


def is_near_duplicate(left: str, right: str, max_distance: int) -> bool:
    return hash_distance(left, right) <= max_distance

