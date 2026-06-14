"""Token usage tracking across all LLM calls."""
from __future__ import annotations

import asyncio
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Optional


@dataclass
class TokenUsage:
    input_tokens: int = 0
    output_tokens: int = 0

    @property
    def total(self) -> int:
        return self.input_tokens + self.output_tokens

    def __add__(self, other: TokenUsage) -> TokenUsage:
        return TokenUsage(
            input_tokens=self.input_tokens + other.input_tokens,
            output_tokens=self.output_tokens + other.output_tokens,
        )

    def __iadd__(self, other: TokenUsage) -> TokenUsage:
        self.input_tokens += other.input_tokens
        self.output_tokens += other.output_tokens
        return self


# Approximate cost per 1M tokens for common providers (input / output)
COST_PER_1M: dict[str, tuple[float, float]] = {
    "anthropic": (3.0, 15.0),
    "openai": (2.5, 10.0),
    "gemini": (0.5, 1.5),
    "groq": (0.6, 2.0),
    "ollama": (0.0, 0.0),
}


@dataclass
class CallRecord:
    provider: str
    model: str
    usage: TokenUsage
    session_id: str
    stage: str
    duration_ms: int
    timestamp: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())


class TokenTracker:
    """Thread-safe token usage accumulator.

    Tracks every LLM call globally and per-session.
    """

    def __init__(self):
        self._calls: list[CallRecord] = []
        self._lock = asyncio.Lock()

    async def record(
        self,
        provider: str,
        model: str,
        input_tokens: int,
        output_tokens: int,
        session_id: str = "",
        stage: str = "",
        duration_ms: int = 0,
    ) -> None:
        async with self._lock:
            self._calls.append(CallRecord(
                provider=provider,
                model=model,
                usage=TokenUsage(input_tokens, output_tokens),
                session_id=session_id,
                stage=stage,
                duration_ms=duration_ms,
            ))

    def global_usage(self) -> TokenUsage:
        usage = TokenUsage()
        for c in self._calls:
            usage += c.usage
        return usage

    def session_usage(self, session_id: str) -> TokenUsage:
        usage = TokenUsage()
        for c in self._calls:
            if c.session_id == session_id:
                usage += c.usage
        return usage

    def total_calls(self) -> int:
        return len(self._calls)

    def estimated_cost(self, usage: Optional[TokenUsage] = None) -> float:
        if usage is None:
            usage = self.global_usage()
        inp, out = usage.input_tokens, usage.output_tokens
        cost = 0.0
        seen = set()
        for c in self._calls:
            rate = COST_PER_1M.get(c.provider, (1.0, 3.0))
            cost += (c.usage.input_tokens * rate[0] + c.usage.output_tokens * rate[1]) / 1_000_000
        return round(cost, 6)

    def stats(self, session_id: Optional[str] = None) -> dict:
        calls = self._calls
        if session_id:
            calls = [c for c in calls if c.session_id == session_id]

        if not calls:
            return {
                "total_calls": 0,
                "total_input_tokens": 0,
                "total_output_tokens": 0,
                "total_tokens": 0,
                "estimated_cost_usd": 0.0,
                "calls": [],
            }

        usage = TokenUsage()
        for c in calls:
            usage += c.usage

        return {
            "total_calls": len(calls),
            "total_input_tokens": usage.input_tokens,
            "total_output_tokens": usage.output_tokens,
            "total_tokens": usage.total,
            "estimated_cost_usd": self.estimated_cost(usage),
            "calls": [
                {
                    "provider": c.provider,
                    "model": c.model,
                    "input_tokens": c.usage.input_tokens,
                    "output_tokens": c.usage.output_tokens,
                    "stage": c.stage,
                    "duration_ms": c.duration_ms,
                    "timestamp": c.timestamp,
                }
                for c in calls[-50:]
            ],
        }

    def recent_context(self, limit: int = 5) -> str:
        """Build a concise summary of recent research for context injection."""
        if not self._calls:
            return ""
        seen_sessions: list[str] = []
        parts: list[str] = []
        for c in reversed(self._calls):
            if c.session_id and c.session_id not in seen_sessions:
                seen_sessions.append(c.session_id)
                if len(seen_sessions) > limit:
                    break
        return f"Recent sessions: {', '.join(seen_sessions)}"
