from __future__ import annotations

import hashlib
import math
import os
import re
from dataclasses import dataclass
from typing import Protocol


class EmbeddingProvider(Protocol):
    """Example-owned embedding interface.

    TongGraph stores and searches vectors, but it does not call embedding
    providers. This protocol keeps model selection outside the core package.
    """

    backend: str
    model_name: str
    model_version: str
    dimensions: int

    def embed_texts(self, texts: list[str], *, role: str) -> list[list[float]]:
        """Embed texts for either document or query usage."""


@dataclass
class HashEmbeddingProvider:
    """Small deterministic embedding backend for offline examples and tests."""

    dimensions: int = 128
    model_name: str = "hash-bow"
    model_version: str = "1"
    backend: str = "hash"

    def __post_init__(self) -> None:
        if self.dimensions <= 0:
            raise ValueError("hash embedding dimensions must be greater than zero")

    def embed_texts(self, texts: list[str], *, role: str) -> list[list[float]]:
        return [self._embed(text) for text in texts]

    def _embed(self, text: str) -> list[float]:
        vector = [0.0] * self.dimensions
        tokens = _tokenize(text)
        if not tokens:
            vector[0] = 1.0
            return vector

        for token in tokens:
            weight = 1.0 + min(len(token), 16) / 16.0
            _add_feature(vector, token, weight)
            for ngram in _char_ngrams(token):
                _add_feature(vector, f"ng:{ngram}", weight * 0.35)

        norm = math.sqrt(sum(value * value for value in vector))
        if norm == 0.0:
            vector[0] = 1.0
            return vector
        return [value / norm for value in vector]


class SentenceTransformerProvider:
    """Optional sentence-transformers backend for larger local builds."""

    backend = "sentence-transformers"

    def __init__(self, model_name: str = "intfloat/e5-small-v2") -> None:
        try:
            from sentence_transformers import SentenceTransformer
        except ImportError as exc:  # pragma: no cover - optional dependency.
            raise RuntimeError(
                "Install optional embeddings dependencies with "
                "`uv sync --extra embeddings`."
            ) from exc

        self.model_name = model_name
        self.model_version = model_name
        device = os.environ.get("SENTENCE_TRANSFORMERS_DEVICE")
        self._model = (
            SentenceTransformer(model_name, device=device)
            if device
            else SentenceTransformer(model_name)
        )
        get_dimension = getattr(
            self._model,
            "get_embedding_dimension",
            self._model.get_sentence_embedding_dimension,
        )
        self.dimensions = int(get_dimension())

    def embed_texts(self, texts: list[str], *, role: str) -> list[list[float]]:
        prefix = "query: " if role == "query" else "passage: "
        encoded = self._model.encode(
            [prefix + text for text in texts],
            normalize_embeddings=True,
            show_progress_bar=False,
        )
        return [[float(value) for value in row] for row in encoded]


def create_embedding_provider(
    backend: str,
    *,
    dimensions: int = 128,
    model_name: str | None = None,
) -> EmbeddingProvider:
    if backend == "hash":
        return HashEmbeddingProvider(dimensions=dimensions)
    if backend == "sentence-transformers":
        return SentenceTransformerProvider(model_name or "intfloat/e5-small-v2")
    raise ValueError(f"unsupported embedding backend: {backend}")


def _tokenize(text: str) -> list[str]:
    return re.findall(r"[\w]+", text.casefold())


def _char_ngrams(token: str) -> list[str]:
    if len(token) <= 3:
        return [token]
    return [token[index : index + 3] for index in range(len(token) - 2)]


def _add_feature(vector: list[float], feature: str, weight: float) -> None:
    digest = hashlib.blake2b(feature.encode("utf-8"), digest_size=8).digest()
    bucket = int.from_bytes(digest[:4], "little") % len(vector)
    sign = 1.0 if digest[4] % 2 == 0 else -1.0
    vector[bucket] += sign * weight
