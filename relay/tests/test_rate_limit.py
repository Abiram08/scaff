"""Tests for the rate limiter."""
from __future__ import annotations

import asyncio
import time
import pytest

from relay.rate_limit import Bucket, RateLimiter


def test_bucket_consume_within_capacity():
    b = Bucket(capacity=5, tokens=5, refill_rate=1.0)
    assert b.consume(1.0) == 0.0
    assert b.consume(1.0) == 0.0
    assert b.available == 3.0


def test_bucket_consume_returns_wait_time():
    b = Bucket(capacity=5, tokens=1.0, refill_rate=10.0)
    wait = b.consume(3.0)
    assert wait > 0.0
    assert b.available == 0.0


def test_bucket_refill():
    b = Bucket(capacity=5, tokens=0.0, refill_rate=5.0)
    b.last_refill = time.monotonic() - 1.0  # simulate 1 second passed
    b._refill()
    assert b.available == 5.0  # fully refilled


def test_bucket_refill_clamped_to_capacity():
    b = Bucket(capacity=5, tokens=4.0, refill_rate=10.0)
    b.last_refill = time.monotonic() - 10.0
    b._refill()
    assert b.available == 5.0  # capped at capacity


@pytest.mark.asyncio
async def test_rate_limiter_acquire():
    limiter = RateLimiter(default_rps=100.0, burst=100)
    t0 = time.monotonic()
    await limiter.acquire("test_provider")
    elapsed = time.monotonic() - t0
    assert elapsed < 0.1  # should not block


@pytest.mark.asyncio
async def test_rate_limiter_burst():
    limiter = RateLimiter(default_rps=10.0, burst=3)
    # First 3 should be instant
    for _ in range(3):
        await limiter.acquire("burst_test")
    # Fourth should wait
    t0 = time.monotonic()
    await limiter.acquire("burst_test")
    elapsed = time.monotonic() - t0
    assert elapsed >= 0.05  # should have waited briefly


def test_rate_limiter_stats():
    limiter = RateLimiter(default_rps=5.0, burst=10, overrides={"custom": (1.0, 3)})
    stats = limiter.stats()
    # Known providers are pre-created
    assert "custom" in stats
    assert stats["custom"]["rps"] == 1.0
    assert stats["custom"]["capacity"] == 3.0
    assert stats["custom"]["available"] <= 3.0


def test_rate_limiter_known_providers():
    limiter = RateLimiter()
    # Anthropic should have pre-configured defaults
    bucket = limiter._get_bucket("anthropic")
    assert bucket.refill_rate == 2.0
    assert bucket.capacity == 5.0

    bucket = limiter._get_bucket("openai")
    assert bucket.refill_rate == 3.0
    assert bucket.capacity == 8.0
