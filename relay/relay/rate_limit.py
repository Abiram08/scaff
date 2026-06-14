"""Token-bucket rate limiter for API calls.

Usage:
    from relay.rate_limit import RateLimiter

    limiter = RateLimiter(default_rps=2.0, burst=5)
    async with limiter.acquire("anthropic"):
        await call_llm()
"""
from __future__ import annotations

import asyncio
import time
from dataclasses import dataclass, field


@dataclass
class Bucket:
    """A single token bucket."""
    capacity: float
    tokens: float
    refill_rate: float  # tokens per second
    last_refill: float = field(default_factory=time.monotonic)

    def _refill(self) -> None:
        now = time.monotonic()
        elapsed = now - self.last_refill
        self.tokens = min(self.capacity, self.tokens + elapsed * self.refill_rate)
        self.last_refill = now

    def consume(self, n: float = 1.0) -> float:
        """Try to consume n tokens. Returns seconds to wait (0 if available)."""
        self._refill()
        if self.tokens >= n:
            self.tokens -= n
            return 0.0
        deficit = n - self.tokens
        wait = deficit / self.refill_rate
        self.tokens = 0
        return wait

    @property
    def available(self) -> float:
        self._refill()
        return self.tokens


# ── Provider defaults (requests/second) ─────────────────────────────

DEFAULT_RPS: dict[str, float] = {
    "anthropic": 2.0,
    "openai": 3.0,
    "gemini": 2.0,
    "groq": 5.0,
    "ollama": 10.0,
    "tavily": 1.0,
    "brave": 1.0,
    "duckduckgo": 2.0,
}

DEFAULT_BURST: dict[str, float] = {
    "anthropic": 5,
    "openai": 8,
    "gemini": 5,
    "groq": 10,
    "ollama": 20,
    "tavily": 3,
    "brave": 3,
    "duckduckgo": 5,
}


class RateLimiter:
    """Per-provider token-bucket rate limiter with async waiting."""

    def __init__(
        self,
        default_rps: float = 2.0,
        burst: float = 5,
        overrides: dict[str, tuple[float, float]] | None = None,
    ):
        """
        Args:
            default_rps: Default refill rate (requests/second) for unknown providers.
            burst: Default bucket capacity for unknown providers.
            overrides: Per-provider overrides: {"anthropic": (rps, burst)}.
        """
        self._default_rps = default_rps
        self._burst = burst
        self._overrides = overrides or {}

        # Pre-create buckets for known providers to avoid races on first access
        self._buckets: dict[str, Bucket] = {}
        all_providers = set(DEFAULT_RPS) | set(self._overrides)
        for p in all_providers:
            rps, cap = self._overrides.get(p, (
                DEFAULT_RPS.get(p, default_rps),
                DEFAULT_BURST.get(p, burst),
            ))
            self._buckets[p] = Bucket(capacity=cap, tokens=cap, refill_rate=rps)

        self._lock = asyncio.Lock()

    def _get_bucket(self, provider: str) -> Bucket:
        if provider not in self._buckets:
            self._buckets[provider] = Bucket(
                capacity=self._burst,
                tokens=self._burst,
                refill_rate=self._default_rps,
            )
        return self._buckets[provider]

    async def acquire(self, provider: str, tokens: float = 1.0) -> None:
        """Wait until tokens are available for the given provider."""
        while True:
            bucket = self._get_bucket(provider)
            async with self._lock:
                wait = bucket.consume(tokens)
            if wait == 0:
                return
            await asyncio.sleep(wait)

    def stats(self) -> dict[str, dict[str, float]]:
        """Return current state of all buckets."""
        return {
            name: {
                "available": round(b.available, 2),
                "capacity": b.capacity,
                "rps": b.refill_rate,
            }
            for name, b in self._buckets.items()
        }
