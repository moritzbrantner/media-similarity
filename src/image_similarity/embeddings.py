from __future__ import annotations

from functools import cached_property
from typing import Any

import numpy as np
from PIL import Image


class ImageEmbedder:
    def __init__(self, model_name: str) -> None:
        self.model_name = model_name

    @cached_property
    def model(self) -> Any:
        from sentence_transformers import SentenceTransformer

        return SentenceTransformer(self.model_name)

    def encode(self, image: Image.Image) -> list[float]:
        embedding = self.model.encode(
            [image],
            convert_to_numpy=True,
            normalize_embeddings=True,
            show_progress_bar=False,
        )[0]
        return np.asarray(embedding, dtype=np.float32).tolist()

    @cached_property
    def dimension(self) -> int:
        sample = Image.new("RGB", (8, 8), color="white")
        return len(self.encode(sample))
