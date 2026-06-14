"""SQLite-backed persistent memory for sessions, conversations, and extracted knowledge."""
from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

import aiosqlite


@dataclass
class MemoryEntry:
    id: int = 0
    session_id: str = ""
    claim: str = ""
    confidence: str = ""
    source_urls: str = ""
    created_at: str = ""


class MemoryStore:
    """Persistent storage for sessions, conversation logs, memories, and token usage.

    Provides cross-session context retrieval so the agent can recall past research.
    """

    def __init__(self, db_path: str = "./memory/relay_memory.db"):
        self.db_path = Path(db_path)
        self.db_path.parent.mkdir(parents=True, exist_ok=True)

    async def init_db(self) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("""
                CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT PRIMARY KEY,
                    query TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'planning',
                    synthesis TEXT,
                    input_tokens INTEGER DEFAULT 0,
                    output_tokens INTEGER DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )
            """)
            await db.execute("""
                CREATE TABLE IF NOT EXISTS conversation_logs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    input_tokens INTEGER DEFAULT 0,
                    output_tokens INTEGER DEFAULT 0,
                    provider TEXT DEFAULT '',
                    model TEXT DEFAULT '',
                    stage TEXT DEFAULT '',
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (session_id) REFERENCES sessions(id)
                )
            """)
            await db.execute("""
                CREATE TABLE IF NOT EXISTS memories (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    claim TEXT NOT NULL,
                    confidence TEXT NOT NULL DEFAULT 'medium',
                    source_urls TEXT DEFAULT '',
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (session_id) REFERENCES sessions(id)
                )
            """)
            await db.execute("""
                CREATE INDEX IF NOT EXISTS idx_conversation_session
                ON conversation_logs(session_id)
            """)
            await db.execute("""
                CREATE INDEX IF NOT EXISTS idx_memories_session
                ON memories(session_id)
            """)
            await db.execute("""
                CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                    claim, content
                )
            """)
            await db.commit()

    async def save_session(self, session_id: str, query: str, status: str, synthesis: Optional[str],
                           input_tokens: int = 0, output_tokens: int = 0) -> None:
        now = datetime.now(timezone.utc).isoformat()
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("""
                INSERT INTO sessions (id, query, status, synthesis, input_tokens, output_tokens, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    status = excluded.status,
                    synthesis = excluded.synthesis,
                    input_tokens = excluded.input_tokens,
                    output_tokens = excluded.output_tokens,
                    updated_at = excluded.updated_at
            """, (session_id, query, status, synthesis, input_tokens, output_tokens, now, now))
            await db.commit()

    async def update_session_tokens(self, session_id: str, input_tokens: int, output_tokens: int) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("""
                UPDATE sessions SET
                    input_tokens = input_tokens + ?,
                    output_tokens = output_tokens + ?,
                    updated_at = ?
                WHERE id = ?
            """, (input_tokens, output_tokens, datetime.now(timezone.utc).isoformat(), session_id))
            await db.commit()

    async def get_session(self, session_id: str) -> Optional[dict]:
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute("SELECT * FROM sessions WHERE id = ?", (session_id,))
            row = await cursor.fetchone()
            if row:
                return dict(row)
            return None

    async def list_sessions(self, limit: int = 50) -> list[dict]:
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute(
                "SELECT id, query, status, input_tokens, output_tokens, created_at, updated_at "
                "FROM sessions ORDER BY created_at DESC LIMIT ?", (limit,)
            )
            return [dict(row) for row in await cursor.fetchall()]

    async def log_conversation(
        self, session_id: str, role: str, content: str,
        input_tokens: int = 0, output_tokens: int = 0,
        provider: str = "", model: str = "", stage: str = "",
    ) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("""
                INSERT INTO conversation_logs
                    (session_id, role, content, input_tokens, output_tokens, provider, model, stage, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """, (
                session_id, role, content, input_tokens, output_tokens,
                provider, model, stage,
                datetime.now(timezone.utc).isoformat(),
            ))
            await db.commit()

    async def get_conversation(self, session_id: str, limit: int = 100) -> list[dict]:
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute(
                "SELECT * FROM conversation_logs WHERE session_id = ? ORDER BY id ASC LIMIT ?",
                (session_id, limit),
            )
            return [dict(row) for row in await cursor.fetchall()]

    async def save_memory(self, session_id: str, claim: str, confidence: str = "medium",
                          source_urls: Optional[list[str]] = None) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            now = datetime.now(timezone.utc).isoformat()
            urls = json.dumps(source_urls or [])
            await db.execute(
                "INSERT INTO memories (session_id, claim, confidence, source_urls, created_at) VALUES (?, ?, ?, ?, ?)",
                (session_id, claim, confidence, urls, now),
            )
            await db.execute(
                "INSERT INTO memories_fts (claim, content) VALUES (?, ?)",
                (claim, urls),
            )
            await db.commit()

    async def search_memories(self, query: str, limit: int = 5) -> list[MemoryEntry]:
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            try:
                cursor = await db.execute("""
                    SELECT m.* FROM memories m
                    JOIN memories_fts f ON m.id = f.rowid
                    WHERE memories_fts MATCH ?
                    ORDER BY rank
                    LIMIT ?
                """, (query, limit))
                rows = await cursor.fetchall()
                return [MemoryEntry(**dict(r)) for r in rows]
            except Exception:
                return []

    async def get_relevant_context(self, query: str, limit: int = 3) -> str:
        """Retrieve relevant past research context for a new query."""
        memories = await self.search_memories(query, limit)
        if not memories:
            recent = await self.list_sessions(limit=3)
            if not recent:
                return ""
            parts = ["Recent research sessions:"]
            for s in recent:
                parts.append(f"  - {s.get('query', '?')} [{s.get('status', '?')}]")
            return "\n".join(parts)

        parts = ["Context from past research:"]
        for m in memories:
            parts.append(f"  - {m.claim} (confidence: {m.confidence})")
            if m.source_urls:
                try:
                    urls = json.loads(m.source_urls)
                    for u in urls[:2]:
                        parts.append(f"    source: {u}")
                except json.JSONDecodeError:
                    pass
        return "\n".join(parts)

    async def get_token_stats(self, session_id: Optional[str] = None) -> dict:
        async with aiosqlite.connect(self.db_path) as db:
            if session_id:
                cursor = await db.execute(
                    "SELECT COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0) "
                    "FROM conversation_logs WHERE session_id = ?", (session_id,)
                )
            else:
                cursor = await db.execute(
                    "SELECT COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0) "
                    "FROM conversation_logs"
                )
            row = await cursor.fetchone()
            inp, out = row[0], row[1]
            return {
                "input_tokens": inp,
                "output_tokens": out,
                "total_tokens": inp + out,
            }
