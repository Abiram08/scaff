"""Relay API - REST + WebSocket."""
from __future__ import annotations

import asyncio
import json
from typing import Optional

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
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


class ResearchRequest(BaseModel):
    question: str
    model: Optional[str] = None
    web_enabled: bool = True


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
    created_at: str
    updated_at: str


@app.on_event("startup")
async def startup():
    await pipeline.corpus.init_db()


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
    async for event in pipeline.research(request.question, session):
        pass


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


@app.websocket("/api/v1/research/{session_id}/stream")
async def stream_research(websocket: WebSocket, session_id: str):
    await websocket.accept()

    session = sessions.get(session_id)
    if not session:
        await websocket.send_json({"event": "error", "data": {"error": "Session not found"}})
        await websocket.close()
        return

    try:
        async for event in pipeline.research(session.query, session):
            await websocket.send_json(event)

            if event["event"] == "finding":
                cmd = await asyncio.wait_for(websocket.receive_text(), timeout=0.1)
                try:
                    cmd_data = json.loads(cmd)
                    if cmd_data.get("command") == "deepen":
                        topic = cmd_data.get("topic", "")
                        await websocket.send_json({
                            "event": "status",
                            "data": {"status": "deepening", "message": f"Deepening on: {topic}"},
                        })
                    elif cmd_data.get("command") == "redirect":
                        new_query = cmd_data.get("query", "")
                        session.query = new_query
                        await websocket.send_json({
                            "event": "status",
                            "data": {"status": "redirecting", "message": f"Redirecting to: {new_query}"},
                        })
                except asyncio.TimeoutError:
                    pass

    except WebSocketDisconnect:
        pass
    except Exception as e:
        await websocket.send_json({"event": "error", "data": {"error": str(e)}})


def cli():
    import uvicorn
    uvicorn.run(app, host=config.api.host, port=config.api.port)
