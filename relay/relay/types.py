"""Pipeline types and data structures."""
from __future__ import annotations

import uuid
from datetime import datetime, timezone
from enum import Enum
from dataclasses import dataclass, field
from typing import Optional


class StageStatus(str, Enum):
    PENDING = "pending"
    RUNNING = "running"
    OK = "ok"
    FAILED = "failed"
    SKIPPED = "skipped"


class ExecutionStatus(str, Enum):
    RUNNING = "running"
    SUCCEEDED = "succeeded"
    FAILED = "failed"


class Confidence(str, Enum):
    HIGH = "high"
    MEDIUM = "medium"
    LOW = "low"
    CONTESTED = "contested"

    @classmethod
    def from_score(cls, score: float, has_contradiction: bool = False) -> "Confidence":
        if has_contradiction:
            return cls.CONTESTED
        if score >= 0.8:
            return cls.HIGH
        if score >= 0.6:
            return cls.MEDIUM
        return cls.LOW

    def emoji(self) -> str:
        return {
            Confidence.HIGH: "✅",
            Confidence.MEDIUM: "⚠️",
            Confidence.LOW: "🔴",
            Confidence.CONTESTED: "⚡",
        }[self]

    def description(self) -> str:
        return {
            Confidence.HIGH: "Multiple sources agree",
            Confidence.MEDIUM: "Single source; verify before acting",
            Confidence.LOW: "Limited corroboration; treat as hypothesis",
            Confidence.CONTESTED: "Sources disagree; see conflict note",
        }[self]


class Stance(str, Enum):
    SUPPORTS = "supports"
    CONTRADICTS = "contradicts"
    NEUTRAL = "neutral"


class SourceKind(str, Enum):
    CORPUS = "corpus"
    WEB = "web"
    LOCAL_FILE = "local_file"


class SourceTag(str, Enum):
    HARNESS_CORPUS = "harness-corpus"
    WEB = "web"
    BOTH = "both"


class SessionStatus(str, Enum):
    PLANNING = "planning"
    SEARCHING = "searching"
    VERIFYING = "verifying"
    SYNTHESIZING = "synthesizing"
    DONE = "done"
    ERROR = "error"


@dataclass
class Stage:
    name: str
    status: StageStatus = StageStatus.PENDING
    started_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    finished_at: Optional[datetime] = None
    duration_ms: int = 0
    details: dict = field(default_factory=dict)


@dataclass
class Source:
    id: int
    title: str
    url: str
    kind: SourceKind
    score: float = 0.0
    content: str = ""


@dataclass
class SubQuestion:
    id: str
    question: str
    source_tag: SourceTag
    priority: int = 1


@dataclass
class Claim:
    id: int
    text: str
    source_ids: list[int]
    confidence: Confidence
    conflict_note: Optional[str] = None


@dataclass
class VerificationResult:
    claim_id: int
    source_id: int
    supported: bool
    evidence: str
    stance: Stance


@dataclass
class Finding:
    index: int
    total: int
    confidence: Confidence
    content: str
    sources: list[Source]
    conflict_note: Optional[str] = None


@dataclass
class ResearchResult:
    question: str
    tldr: str
    body: str
    report: str
    claims: list[Claim]
    verifications: list[VerificationResult]
    findings: list[Finding]
    sources: list[Source]
    sub_questions: list[SubQuestion]
    model: str
    provider: str
    input_tokens: int = 0
    output_tokens: int = 0
    estimated_cost_usd: float = 0.0
    duration_ms: int = 0


@dataclass
class Session:
    id: str
    query: str
    status: SessionStatus
    sub_questions: list[SubQuestion] = field(default_factory=list)
    findings: list[Finding] = field(default_factory=list)
    synthesis: Optional[str] = None
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    updated_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    input_tokens: int = 0
    output_tokens: int = 0

    @classmethod
    def create(cls, query: str) -> "Session":
        return cls(
            id=f"sess-{uuid.uuid4().hex[:12]}",
            query=query,
            status=SessionStatus.PLANNING,
        )

    def update_status(self, status: SessionStatus):
        self.status = status
        self.updated_at = datetime.now(timezone.utc)

    def add_finding(self, finding: Finding):
        self.findings.append(finding)
        self.updated_at = datetime.now(timezone.utc)

    def add_token_usage(self, input_tokens: int, output_tokens: int):
        self.input_tokens += input_tokens
        self.output_tokens += output_tokens

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "query": self.query,
            "status": self.status.value,
            "sub_questions": [
                {"id": sq.id, "question": sq.question, "source_tag": sq.source_tag.value, "priority": sq.priority}
                for sq in self.sub_questions
            ],
            "findings": [
                {
                    "index": f.index,
                    "total": f.total,
                    "confidence": f.confidence.value,
                    "content": f.content,
                    "conflict_note": f.conflict_note,
                }
                for f in self.findings
            ],
            "synthesis": self.synthesis,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "created_at": self.created_at.isoformat(),
            "updated_at": self.updated_at.isoformat(),
        }
