"""Request enforcer for API limiting and rate limiting with MIN/MEDIUM/MAX modes."""

import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, Any, Optional, List, Tuple


class Mode(Enum):
    """API limiting modes"""
    MIN = "min"      # Cost-optimized
    MEDIUM = "medium"  # Balanced
    MAX = "max"      # Quality-optimized


@dataclass
class ModeConfig:
    """Configuration for each mode"""
    mode: Mode
    monthly_tokens: int
    monthly_cost: float
    rate_limit_per_minute: int
    concurrent_limit: int
    cache_ttl_days: int
    model: str
    temperature: float
    max_tokens: int
    refill_rate: float = 0.0
    burst_capacity: int = 0


class ModeRegistry:
    """Registry of mode configurations"""

    CONFIGS = {
        Mode.MIN: ModeConfig(
            mode=Mode.MIN,
            monthly_tokens=1_000_000,
            monthly_cost=0.25,
            rate_limit_per_minute=6,
            concurrent_limit=1,
            cache_ttl_days=30,
            model="gpt-4o-mini",
            temperature=0.3,
            max_tokens=2048,
            refill_rate=0.1,
            burst_capacity=6,
        ),
        Mode.MEDIUM: ModeConfig(
            mode=Mode.MEDIUM,
            monthly_tokens=5_000_000,
            monthly_cost=1.25,
            rate_limit_per_minute=12,
            concurrent_limit=2,
            cache_ttl_days=14,
            model="gpt-4o",
            temperature=0.7,
            max_tokens=4096,
            refill_rate=0.2,
            burst_capacity=12,
        ),
        Mode.MAX: ModeConfig(
            mode=Mode.MAX,
            monthly_tokens=20_000_000,
            monthly_cost=5.00,
            rate_limit_per_minute=30,
            concurrent_limit=5,
            cache_ttl_days=7,
            model="gpt-4o",
            temperature=1.0,
            max_tokens=4096,
            refill_rate=0.5,
            burst_capacity=30,
        ),
    }

    @classmethod
    def get_config(cls, mode: Mode) -> ModeConfig:
        """Get configuration for a mode"""
        return cls.CONFIGS[mode]

    @classmethod
    def get_all_configs(cls) -> Dict[Mode, ModeConfig]:
        """Get all mode configurations"""
        return cls.CONFIGS


@dataclass
class RateLimitHeaders:
    """Parsed rate limit headers from OpenAI API response"""
    limit_requests: int = 0
    limit_tokens: int = 0
    remaining_requests: int = 0
    remaining_tokens: int = 0
    reset_requests: float = 0.0
    reset_tokens: float = 0.0


@dataclass
class QueueItem:
    """An item in the request queue"""
    id: str
    enqueued_at: float
    estimated_tokens: int


class TokenBucket:
    """
    Token bucket rate limiter.

    Tokens are added at `refill_rate` per second up to `capacity`.
    Each request consumes one token. Supports burst traffic up to
    capacity, then smooths to refill_rate.
    """

    def __init__(self, capacity: int, refill_rate: float):
        self.capacity = capacity
        self.refill_rate = refill_rate
        self.tokens = float(capacity)
        self.last_refill = time.time()

    def _refill(self) -> None:
        now = time.time()
        elapsed = now - self.last_refill
        self.tokens = min(self.capacity, self.tokens + elapsed * self.refill_rate)
        self.last_refill = now

    def acquire(self, tokens: float = 1.0) -> bool:
        self._refill()
        if self.tokens >= tokens:
            self.tokens -= tokens
            return True
        return False

    def peek(self, tokens: float = 1.0) -> bool:
        self._refill()
        return self.tokens >= tokens

    def estimated_wait(self, tokens: float = 1.0) -> float:
        self._refill()
        if self.tokens >= tokens:
            return 0.0
        needed = tokens - self.tokens
        if self.refill_rate <= 0:
            return float('inf')
        return needed / self.refill_rate

    @property
    def available_tokens(self) -> float:
        self._refill()
        return self.tokens

    @property
    def utilization(self) -> float:
        cap = self.capacity if self.capacity > 0 else 1
        return 1.0 - (self.available_tokens / cap)


class RequestQueue:
    """
    Request queue with position feedback and estimated wait times.
    Tracks pending requests and calculates wait based on token bucket state.
    """

    def __init__(self, bucket: TokenBucket):
        self.bucket = bucket
        self.queue: List[QueueItem] = []
        self._counter = 0

    def enqueue(self, estimated_tokens: int = 1000) -> Tuple[int, float]:
        self._counter += 1
        item = QueueItem(
            id=str(self._counter),
            enqueued_at=time.time(),
            estimated_tokens=estimated_tokens,
        )
        self.queue.append(item)
        return self._position_and_wait(item)

    def dequeue(self) -> Optional[QueueItem]:
        if not self.queue:
            return None
        return self.queue.pop(0)

    def _position_and_wait(self, item: QueueItem) -> Tuple[int, float]:
        pos = self.queue.index(item) + 1
        ahead = self.queue[:pos]
        total_tokens = sum(i.estimated_tokens for i in ahead)
        wait = self.bucket.estimated_wait(total_tokens)
        return pos, wait

    def position(self, item_id: str) -> Optional[Tuple[int, float]]:
        for item in self.queue:
            if item.id == item_id:
                return self._position_and_wait(item)
        return None

    def clear(self) -> None:
        self.queue.clear()
        self._counter = 0

    @property
    def size(self) -> int:
        return len(self.queue)

    @property
    def is_empty(self) -> bool:
        return len(self.queue) == 0


class RequestEnforcer:
    """
    Enforce API limits and rate limiting.
    Uses token bucket algorithm for smooth rate limiting.
    Supports request queuing and real-time rate limit header parsing.
    """

    def __init__(self, mode: Mode = Mode.MEDIUM):
        self.mode = mode
        self.config = ModeRegistry.get_config(mode)
        self.bucket = TokenBucket(
            capacity=self.config.burst_capacity or self.config.rate_limit_per_minute,
            refill_rate=self.config.refill_rate or (self.config.rate_limit_per_minute / 60.0),
        )
        self.queue = RequestQueue(self.bucket)
        self.monthly_tokens_used = 0
        self.tokens_used = 0
        self.last_rate_limit_headers: Optional[RateLimitHeaders] = None

    def validate_request(self, tokens_estimate: int) -> Dict[str, Any]:
        """
        Validate a request can proceed within limits.

        Returns:
            {
                "allowed": bool,
                "reason": str,
                "remaining_tokens": int,
                "requests_remaining_this_minute": int,
                "estimated_wait": float,
                "queue_position": int,
            }
        """
        available = int(self.bucket.available_tokens)
        estimated_wait = self.bucket.estimated_wait()
        remaining_budget = self.config.monthly_tokens - self.monthly_tokens_used

        if self.monthly_tokens_used + tokens_estimate > self.config.monthly_tokens:
            return {
                "allowed": False,
                "reason": f"Would exceed monthly budget. {remaining_budget} tokens left.",
                "remaining_tokens": remaining_budget,
                "requests_remaining_this_minute": available,
                "estimated_wait": estimated_wait,
                "queue_position": self.queue.size + 1,
            }

        if available <= 0:
            return {
                "allowed": False,
                "reason": f"Rate limited. Try again in {estimated_wait:.1f}s",
                "remaining_tokens": remaining_budget,
                "requests_remaining_this_minute": 0,
                "estimated_wait": estimated_wait,
                "queue_position": self.queue.size + 1,
            }

        return {
            "allowed": True,
            "reason": "Request allowed",
            "remaining_tokens": remaining_budget - tokens_estimate,
            "requests_remaining_this_minute": available - 1,
            "estimated_wait": 0.0,
            "queue_position": 0,
        }

    def check_budget(self, tokens: int) -> None:
        result = self.validate_request(tokens)
        if not result["allowed"]:
            raise ValueError(result["reason"])

    def check_rate_limit(self) -> None:
        if not self.bucket.peek():
            wait = self.bucket.estimated_wait()
            raise ValueError(f"Rate limited. Try again in {wait:.1f}s")

    def record_request(self, tokens_used: int) -> None:
        self.monthly_tokens_used += tokens_used
        self.tokens_used = self.monthly_tokens_used
        self.bucket.acquire()

    def update_from_headers(self, headers: Dict[str, str]) -> None:
        """
        Update token bucket state from OpenAI API response headers.
        Parses x-ratelimit-remaining-requests, x-ratelimit-reset-requests, etc.
        """
        rh = RateLimitHeaders()
        try:
            if 'x-ratelimit-remaining-requests' in headers:
                rh.remaining_requests = int(headers['x-ratelimit-remaining-requests'])
                self.bucket.tokens = float(rh.remaining_requests)
            if 'x-ratelimit-limit-requests' in headers:
                rh.limit_requests = int(headers['x-ratelimit-limit-requests'])
            if 'x-ratelimit-remaining-tokens' in headers:
                rh.remaining_tokens = int(headers['x-ratelimit-remaining-tokens'])
            if 'x-ratelimit-reset-requests' in headers:
                rh.reset_requests = float(headers['x-ratelimit-reset-requests'])
        except (ValueError, KeyError, TypeError):
            pass
        self.last_rate_limit_headers = rh

    def get_remaining_budget(self) -> int:
        return self.config.monthly_tokens - self.monthly_tokens_used
