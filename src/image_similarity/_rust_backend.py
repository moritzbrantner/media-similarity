from __future__ import annotations

from importlib import import_module
from types import ModuleType


def _load_backend() -> ModuleType | None:
    try:
        return import_module("image_similarity._rust")
    except ImportError:
        return None


BACKEND = _load_backend()


def is_available() -> bool:
    return BACKEND is not None
