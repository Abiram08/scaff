"""Relay API - REST + WebSocket."""
from __future__ import annotations

import asyncio
import json
import time
from collections import defaultdict
from typing import Optional

from fastapi import FastAPI, Request, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from pydantic import BaseModel

from ..config import RelayConfig
from ..pipeline_runner import ResearchPipeline
from ..types import Session, SessionStatus


app = FastAPI(title="Relay", version="0.1.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

config = RelayConfig.from_env()
pipeline = ResearchPipeline(config)
sessions: dict[str, Session] = {}


# ── Sliding window rate limiter (per-IP) ───────────────────────────

class SlidingWindowLimiter:
    """Simple in-memory sliding window for per-IP request throttling."""

    def __init__(self, max_requests: int = 30, window_seconds: int = 60):
        self.max_requests = max_requests
        self.window = window_seconds
        self._hits: dict[str, list[float]] = defaultdict(list)

    def is_allowed(self, key: str) -> bool:
        now = time.monotonic()
        cutoff = now - self.window
        self._hits[key] = [t for t in self._hits[key] if t > cutoff]
        if len(self._hits[key]) >= self.max_requests:
            return False
        self._hits[key].append(now)
        return True

    def retry_after(self, key: str) -> int:
        """Seconds until the oldest request in the window expires."""
        hits = self._hits.get(key, [])
        if not hits:
            return 0
        return max(1, int(hits[0] + self.window - time.monotonic()))


_limiter = SlidingWindowLimiter(max_requests=30, window_seconds=60)


@app.middleware("http")
async def rate_limit_middleware(request: Request, call_next):
    # Skip rate limiting for health checks
    if request.url.path in ("/health", "/docs", "/openapi.json"):
        return await call_next(request)

    client_ip = request.client.host if request.client else "unknown"
    if not _limiter.is_allowed(client_ip):
        retry = _limiter.retry_after(client_ip)
        return JSONResponse(
            status_code=429,
            content={"error": "Rate limit exceeded", "retry_after_seconds": retry},
            headers={"Retry-After": str(retry)},
        )
    return await call_next(request)


class ResearchRequest(BaseModel):
    question: str
    model: Optional[str] = None
    web_enabled: bool = True
    api_key: Optional[str] = None
    provider: Optional[str] = None


class ResearchResponse(BaseModel):
    session_id: str
    status: str
    message: str


class SessionResponse(BaseModel):
    id: str
    query: str
    status: str
    sub_questions: list[dict]
    findings: list[dict]
    synthesis: Optional[str]
    input_tokens: int = 0
    output_tokens: int = 0
    created_at: str
    updated_at: str


@app.on_event("startup")
async def startup():
    await pipeline.corpus.init_db()
    await pipeline.memory.init_db()


@app.on_event("shutdown")
async def shutdown():
    await pipeline.close()


@app.post("/api/v1/research", response_model=ResearchResponse)
async def start_research(request: ResearchRequest):
    session = Session.create(request.question)
    sessions[session.id] = session

    asyncio.create_task(_run_research(session, request))

    return ResearchResponse(
        session_id=session.id,
        status="planning",
        message="Research session started",
    )


async def _run_research(session: Session, request: ResearchRequest):
    try:
        async for event in pipeline.research(
            request.question,
            session,
            api_key=request.api_key,
            provider=request.provider,
        ):
            pass
    except Exception as e:
        session.update_status(SessionStatus.ERROR)
        session.synthesis = str(e)


@app.get("/api/v1/research/{session_id}", response_model=SessionResponse)
async def get_session(session_id: str):
    session = sessions.get(session_id)
    if not session:
        return {"error": "Session not found"}
    return SessionResponse(**session.to_dict())


@app.delete("/api/v1/research/{session_id}")
async def cancel_session(session_id: str):
    if session_id in sessions:
        del sessions[session_id]
    return {"status": "cancelled"}


@app.get("/api/v1/research/{session_id}/export")
async def export_session(session_id: str):
    session = sessions.get(session_id)
    if not session:
        return {"error": "Session not found"}
    return {"markdown": session.synthesis or ""}


# ── API Key Validation ──────────────────────────────────────────────


class ValidateKeyRequest(BaseModel):
    provider: str
    api_key: str


@app.post("/api/v1/validate-key")
async def validate_api_key(request: ValidateKeyRequest):
    """Test if an API key is valid by making a minimal API call."""
    if not request.api_key:
        return JSONResponse(
            status_code=400,
            content={"valid": False, "error": "API key cannot be empty"},
        )

    if request.provider == "ollama":
        return {"valid": True, "message": "Ollama runs locally — no API key needed"}

    try:
        from ..llm import ChatMessage, LLMClient

        test_client = LLMClient(config)
        test_client.set_api_key(request.api_key)
        config.llm.provider = request.provider

        resp = await test_client.chat(
            [ChatMessage.user("Say 'ok' and nothing else.")],
            max_tokens=10,
            temperature=0,
        )
        return {"valid": True, "model": resp.model, "usage": {
            "input_tokens": resp.usage.input_tokens,
            "output_tokens": resp.usage.output_tokens,
        }}
    except Exception as e:
        error_msg = str(e)
        if "401" in error_msg:
            return {"valid": False, "error": "Invalid API key — authentication failed"}
        if "402" in error_msg or "quota" in error_msg:
            return {"valid": False, "error": "API rate limit or quota exceeded — check billing"}
        return {"valid": False, "error": error_msg}


# ── Token Tracking Endpoints ────────────────────────────────────────


@app.get("/api/v1/tokens")
async def get_global_token_stats():
    return pipeline._token_tracker.stats()


@app.get("/api/v1/tokens/{session_id}")
async def get_session_token_stats(session_id: str):
    return pipeline._token_tracker.stats(session_id=session_id)


# ── Memory Endpoints ────────────────────────────────────────────────


@app.get("/api/v1/memory/context")
async def get_memory_context(query: str = ""):
    if not query:
        return {"context": ""}
    context = await pipeline.memory.get_relevant_context(query)
    return {"context": context}


@app.get("/api/v1/memory/sessions")
async def list_memory_sessions(limit: int = 20):
    sessions_list = await pipeline.memory.list_sessions(limit=limit)
    return {"sessions": sessions_list}


@app.get("/api/v1/memory/sessions/{session_id}/conversation")
async def get_session_conversation(session_id: str):
    logs = await pipeline.memory.get_conversation(session_id)
    return {"logs": logs}


# ── Corpus Management ────────────────────────────────────────────────


@app.post("/api/v1/corpus/refresh")
async def refresh_corpus():
    """Crawl Harness docs + seed + re-embed the corpus."""
    from ..corpus.crawler import HarnessCrawler
    from ..corpus.embedder import EmbeddingClient, chunk_document

    crawler = HarnessCrawler()
    try:
        result = await crawler.crawl_docs(max_pages=30)
    finally:
        await crawler.close()

    # Insert crawled chunks
    await pipeline.corpus.insert_chunks(result.chunks)
    n_crawled = len(result.chunks)

    # Embed all unembedded chunks
    embedder = EmbeddingClient(
        api_key=pipeline.get_api_key("openai") or "",
        model=config.corpus.embedding.model,
    )
    unembedded = await pipeline.corpus.get_unembedded_chunks()
    n_embedded = 0
    n_errors = 0

    try:
        for chunk in unembedded:
            try:
                vector = await embedder.embed(chunk.content[:2048])
                await pipeline.corpus.set_embedding(chunk.id, vector)
                n_embedded += 1
            except Exception:
                n_errors += 1
    finally:
        await embedder.close()

    total = await pipeline.corpus.count()
    emb_count = await pipeline.corpus.get_embedding_count()

    return {
        "status": "ok",
        "chunks_crawled": n_crawled,
        "chunks_embedded": n_embedded,
        "embedding_errors": n_errors,
        "total_chunks": total,
        "total_embedded": emb_count,
        "crawl_errors": result.errors[:5],
    }


@app.post("/api/v1/corpus/seed")
async def seed_corpus():
    """Seed the corpus from the local seed.jsonl file."""
    from pathlib import Path
    seed_path = Path(__file__).parent.parent.parent / "corpus" / "seed.jsonl"
    if not seed_path.exists():
        return {"status": "error", "error": f"Seed file not found at {seed_path}"}

    chunks = pipeline.corpus.load_seed_file(seed_path)
    await pipeline.corpus.insert_chunks(chunks)
    return {"status": "ok", "chunks_seeded": len(chunks)}


@app.websocket("/api/v1/research/{session_id}/stream")
async def stream_research(websocket: WebSocket, session_id: str):
    await websocket.accept()

    session = sessions.get(session_id)
    if not session:
        await websocket.send_json({"event": "error", "data": {"error": "Session not found"}})
        await websocket.close()
        return

    async def _process_commands(ws: WebSocket, session: Session):
        """Background task to listen for user commands during streaming."""
        try:
            while True:
                cmd_text = await asyncio.wait_for(ws.receive_text(), timeout=0.5)
                try:
                    cmd_data = json.loads(cmd_text)
                    yield cmd_data
                except json.JSONDecodeError:
                    pass
        except (asyncio.TimeoutError, WebSocketDisconnect):
            pass

    cmd_iter = _process_commands(websocket, session)
    cmd_queue: list[dict] = []

    try:
        async for event in pipeline.research(session.query, session):
            await websocket.send_json(event)

            # Check for queued commands after each event
            try:
                cmd = await anext(cmd_iter)
                cmd_queue.append(cmd)
            except StopAsyncIteration:
                pass

            if cmd_queue:
                cmd = cmd_queue.pop(0)
                if cmd.get("command") == "deepen":
                    topic = cmd.get("topic", "")
                    async for ev in pipeline.deepen(session, topic):
                        await websocket.send_json(ev)
                elif cmd.get("command") == "redirect":
                    new_query = cmd.get("query", "")
                    async for ev in pipeline.redirect(session, new_query):
                        await websocket.send_json(ev)

    except WebSocketDisconnect:
        pass
    except Exception as e:
        await websocket.send_json({"event": "error", "data": {"error": str(e)}})


def cli():
    import uvicorn
    uvicorn.run(app, host=config.api.host, port=config.api.port)
