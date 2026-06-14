"""Corpus storage with SQLite + FTS5 + embedding vector search."""
from __future__ import annotations

import json
import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

import aiosqlite

from .embedder import cosine_similarity, deserialize_embedding, serialize_embedding


@dataclass
class CorpusChunk:
    id: str
    url: str
    title: str
    section: Optional[str]
    content: str
    source: str = "seed"
    embedding: Optional[list[float]] = None


@dataclass
class CorpusSearchResult:
    chunk: CorpusChunk
    score: float
    rank: int


class CorpusStore:
    def __init__(self, db_path: str = "./corpus/harness.db"):
        self.db_path = Path(db_path)
        self.db_path.parent.mkdir(parents=True, exist_ok=True)

    async def init_db(self):
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("""
                CREATE TABLE IF NOT EXISTS chunks (
                    id TEXT PRIMARY KEY,
                    url TEXT NOT NULL,
                    title TEXT NOT NULL,
                    section TEXT,
                    content TEXT NOT NULL,
                    source TEXT DEFAULT 'seed'
                )
            """)
            await db.execute("""
                CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                    id, title, section, content
                )
            """)
            await db.execute("""
                CREATE TABLE IF NOT EXISTS embeddings (
                    chunk_id TEXT PRIMARY KEY,
                    vector TEXT NOT NULL,
                    model TEXT NOT NULL DEFAULT 'text-embedding-3-small',
                    FOREIGN KEY (chunk_id) REFERENCES chunks(id)
                )
            """)
            await db.commit()

    async def insert_chunks(self, chunks: list[CorpusChunk]):
        async with aiosqlite.connect(self.db_path) as db:
            for chunk in chunks:
                await db.execute(
                    "INSERT OR REPLACE INTO chunks (id, url, title, section, content, source) VALUES (?, ?, ?, ?, ?, ?)",
                    (chunk.id, chunk.url, chunk.title, chunk.section, chunk.content, chunk.source),
                )
                if chunk.embedding:
                    await db.execute(
                        "INSERT OR REPLACE INTO embeddings (chunk_id, vector) VALUES (?, ?)",
                        (chunk.id, serialize_embedding(chunk.embedding)),
                    )
            await db.commit()

            await db.execute("DELETE FROM chunks_fts")
            await db.execute("""
                INSERT INTO chunks_fts(rowid, id, title, section, content)
                SELECT rowid, id, title, section, content FROM chunks
            """)
            await db.commit()

    async def search_vector(
        self,
        query_vector: list[float],
        limit: int = 10,
        min_score: float = 0.5,
    ) -> list[CorpusSearchResult]:
        """Search by vector similarity (cosine). Loads all embeddings and scores in memory."""
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute("""
                SELECT c.*, e.vector as emb
                FROM chunks c
                JOIN embeddings e ON c.id = e.chunk_id
            """)
            rows = await cursor.fetchall()

        scored = []
        for row in rows:
            emb = deserialize_embedding(row["emb"])
            score = cosine_similarity(query_vector, emb)
            if score >= min_score:
                scored.append(CorpusSearchResult(
                    chunk=CorpusChunk(
                        id=row["id"],
                        url=row["url"],
                        title=row["title"],
                        section=row["section"],
                        content=row["content"],
                        source=row["source"],
                        embedding=emb,
                    ),
                    score=score,
                    rank=0,
                ))

        scored.sort(key=lambda r: r.score, reverse=True)
        for i, r in enumerate(scored):
            r.rank = i + 1
        return scored[:limit]

    async def hybrid_search(
        self,
        query: str,
        query_vector: list[float],
        limit: int = 10,
        fts_weight: float = 0.4,
        vector_weight: float = 0.6,
    ) -> list[CorpusSearchResult]:
        """Combine FTS and vector search with weighted scoring."""
        fts_results = await self.search_fts(query, limit=limit * 2)
        vector_results = await self.search_vector(query_vector, limit=limit * 2)

        merged: dict[str, CorpusSearchResult] = {}

        for r in fts_results:
            merged[r.chunk.id] = CorpusSearchResult(
                chunk=r.chunk,
                score=r.score * fts_weight,
                rank=0,
            )

        for r in vector_results:
            if r.chunk.id in merged:
                merged[r.chunk.id].score += r.score * vector_weight
            else:
                merged[r.chunk.id] = CorpusSearchResult(
                    chunk=r.chunk,
                    score=r.score * vector_weight,
                    rank=0,
                )

        sorted_results = sorted(merged.values(), key=lambda r: r.score, reverse=True)
        for i, r in enumerate(sorted_results):
            r.rank = i + 1
        return sorted_results[:limit]

    async def set_embedding(self, chunk_id: str, vector: list[float], model: str = "text-embedding-3-small"):
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute(
                "INSERT OR REPLACE INTO embeddings (chunk_id, vector, model) VALUES (?, ?, ?)",
                (chunk_id, serialize_embedding(vector), model),
            )
            await db.commit()

    async def get_unembedded_chunks(self) -> list[CorpusChunk]:
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute("""
                SELECT c.* FROM chunks c
                LEFT JOIN embeddings e ON c.id = e.chunk_id
                WHERE e.chunk_id IS NULL
            """)
            rows = await cursor.fetchall()
            return [
                CorpusChunk(
                    id=row["id"],
                    url=row["url"],
                    title=row["title"],
                    section=row["section"],
                    content=row["content"],
                    source=row["source"],
                )
                for row in rows
            ]

    async def get_embedding_count(self) -> int:
        async with aiosqlite.connect(self.db_path) as db:
            cursor = await db.execute("SELECT COUNT(*) FROM embeddings")
            row = await cursor.fetchone()
            return row[0]

    async def search_fts(self, query: str, limit: int = 10) -> list[CorpusChunk]:
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute(
                """SELECT c.* FROM chunks c
                   JOIN chunks_fts f ON c.id = f.id
                   WHERE chunks_fts MATCH ?
                   ORDER BY rank
                   LIMIT ?""",
                (query, limit),
            )
            rows = await cursor.fetchall()
            return [
                CorpusChunk(
                    id=row["id"],
                    url=row["url"],
                    title=row["title"],
                    section=row["section"],
                    content=row["content"],
                    source=row["source"],
                )
                for row in rows
            ]

    async def get_all_chunks(self) -> list[CorpusChunk]:
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute("SELECT * FROM chunks")
            rows = await cursor.fetchall()
            return [
                CorpusChunk(
                    id=row["id"],
                    url=row["url"],
                    title=row["title"],
                    section=row["section"],
                    content=row["content"],
                    source=row["source"],
                )
                for row in rows
            ]

    async def count(self) -> int:
        async with aiosqlite.connect(self.db_path) as db:
            cursor = await db.execute("SELECT COUNT(*) FROM chunks")
            row = await cursor.fetchone()
            return row[0]

    async def clear(self):
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("DELETE FROM chunks")
            await db.commit()

    def load_seed_file(self, path: Path) -> list[CorpusChunk]:
        chunks = []
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    data = json.loads(line)
                    chunks.append(CorpusChunk(
                        id=data.get("id", ""),
                        url=data.get("url", ""),
                        title=data.get("title", ""),
                        section=data.get("section"),
                        content=data.get("content", ""),
                        source=data.get("source", "seed"),
                    ))
                except json.JSONDecodeError:
                    continue
        return chunks

    async def ensure_seeded(self, seed_path: Optional[Path] = None):
        if await self.count() > 0:
            return

        if seed_path is None:
            seed_path = Path(__file__).parent.parent.parent / "corpus" / "seed.jsonl"

        if seed_path.exists():
            chunks = self.load_seed_file(seed_path)
            if chunks:
                await self.insert_chunks(chunks)
