"""Corpus storage with SQLite + FTS5."""
from __future__ import annotations

import json
import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

import aiosqlite


@dataclass
class CorpusChunk:
    id: str
    url: str
    title: str
    section: Optional[str]
    content: str
    source: str = "seed"


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
            await db.commit()

    async def insert_chunks(self, chunks: list[CorpusChunk]):
        async with aiosqlite.connect(self.db_path) as db:
            for chunk in chunks:
                await db.execute(
                    "INSERT OR REPLACE INTO chunks (id, url, title, section, content, source) VALUES (?, ?, ?, ?, ?, ?)",
                    (chunk.id, chunk.url, chunk.title, chunk.section, chunk.content, chunk.source),
                )
            await db.commit()

            await db.execute("DELETE FROM chunks_fts")
            await db.execute("""
                INSERT INTO chunks_fts(rowid, id, title, section, content)
                SELECT rowid, id, title, section, content FROM chunks
            """)
            await db.commit()

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
