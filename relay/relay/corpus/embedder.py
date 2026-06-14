"""Chunking and embedding for corpus documents."""
from __future__ import annotations

import json
import math
from typing import Optional

import httpx
import numpy as np
from numpy.linalg import norm

_EMBEDDING_CACHE: dict[str, list[float]] = {}


def cosine_similarity(a: list[float], b: list[float]) -> float:
    arr_a = np.array(a, dtype=np.float32)
    arr_b = np.array(b, dtype=np.float32)
    denom = norm(arr_a) * norm(arr_b)
    if denom == 0:
        return 0.0
    return float(np.dot(arr_a, arr_b) / denom)


class EmbeddingClient:
    """Generates embeddings via an OpenAI-compatible API."""

    def __init__(
        self,
        api_key: Optional[str] = None,
        model: str = "text-embedding-3-small",
        base_url: str = "https://api.openai.com/v1",
    ):
        self.api_key = api_key or ""
        self.model = model
        self.base_url = base_url
        self._http = httpx.AsyncClient(timeout=30.0)

    async def close(self):
        await self._http.aclose()

    async def embed(self, text: str) -> list[float]:
        if text in _EMBEDDING_CACHE:
            return _EMBEDDING_CACHE[text]

        resp = await self._http.post(
            f"{self.base_url}/embeddings",
            headers={"Authorization": f"Bearer {self.api_key}"},
            json={"model": self.model, "input": text},
        )
        resp.raise_for_status()
        data = resp.json()
        vector = data["data"][0]["embedding"]

        _EMBEDDING_CACHE[text] = vector
        return vector

    async def embed_batch(self, texts: list[str]) -> list[list[float]]:
        uncached = [(i, t) for i, t in enumerate(texts) if t not in _EMBEDDING_CACHE]
        results: list[Optional[list[float]]] = [None] * len(texts)

        for idx, text in [(i, t) for i, t in enumerate(texts) if t in _EMBEDDING_CACHE]:
            cached = _EMBEDDING_CACHE.get(text)
            if cached is not None:
                results[idx] = cached

        if uncached:
            raw_texts = [t for _, t in uncached]
            resp = await self._http.post(
                f"{self.base_url}/embeddings",
                headers={"Authorization": f"Bearer {self.api_key}"},
                json={"model": self.model, "input": raw_texts},
            )
            resp.raise_for_status()
            data = resp.json()

            for i, (orig_idx, text) in enumerate(uncached):
                vector = data["data"][i]["embedding"]
                _EMBEDDING_CACHE[text] = vector
                results[orig_idx] = vector

        return [r for r in results if r is not None]

    def embed_sync(self, text: str) -> list[float]:
        """Synchronous fallback for non-async contexts."""
        if text in _EMBEDDING_CACHE:
            return _EMBEDDING_CACHE[text]

        with httpx.Client(timeout=30.0) as client:
            resp = client.post(
                f"{self.base_url}/embeddings",
                headers={"Authorization": f"Bearer {self.api_key}"},
                json={"model": self.model, "input": text},
            )
            resp.raise_for_status()
            data = resp.json()
            vector = data["data"][0]["embedding"]
            _EMBEDDING_CACHE[text] = vector
            return vector


def chunk_document(
    content: str,
    title: str,
    url: str,
    chunk_size: int = 512,
    overlap: int = 64,
) -> list[dict]:
    """Split a document into overlapping chunks for embedding."""
    words = content.split()
    if len(words) <= chunk_size:
        return [{"text": content, "title": title, "url": url}]

    chunks = []
    step = chunk_size - overlap
    for i in range(0, len(words), step):
        chunk_words = words[i:i + chunk_size]
        if len(chunk_words) < 32:
            continue
        chunk_text = " ".join(chunk_words)
        chunk_num = int(math.ceil((i + 1) / step))
        chunks.append({
            "text": chunk_text,
            "title": f"{title} (part {chunk_num})",
            "url": url,
        })
    return chunks


def serialize_embedding(vector: list[float]) -> str:
    return json.dumps(vector)


def deserialize_embedding(data: str) -> list[float]:
    return json.loads(data)
