"""Response caching for OpenAI requests to save API tokens."""

import hashlib
import json
import time
from pathlib import Path
from typing import Any

from .config import ScaffConfig


CACHE_DIR = ScaffConfig.CONFIG_DIR / "cache"
CACHE_TTL = 7 * 24 * 3600  # 7 days


def _cache_key(description: str, model: str) -> str:
    """Generate a deterministic cache key for a request."""
    raw = f"{description}||{model}".encode("utf-8")
    return hashlib.md5(raw).hexdigest()


def _cache_path(key: str) -> Path:
    """Get the file path for a cache entry."""
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    return CACHE_DIR / f"{key}.json"


def get(description: str, model: str) -> str | None:
    """Return cached response if valid, None otherwise."""
    enabled = ScaffConfig.get("cache_enabled", True)
    if not enabled:
        return None

    path = _cache_path(_cache_key(description, model))
    if not path.exists():
        return None

    try:
        entry: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
        age = time.time() - entry.get("cached_at", 0)
        if age > CACHE_TTL:
            path.unlink(missing_ok=True)
            return None
        return entry.get("response")
    except (json.JSONDecodeError, OSError):
        path.unlink(missing_ok=True)
        return None


def set(description: str, model: str, response: str) -> None:
    """Cache an OpenAI response."""
    enabled = ScaffConfig.get("cache_enabled", True)
    if not enabled:
        return

    path = _cache_path(_cache_key(description, model))
    entry = {
        "description": description,
        "model": model,
        "response": response,
        "cached_at": time.time(),
    }
    path.write_text(json.dumps(entry, indent=2), encoding="utf-8")


def clear() -> int:
    """Delete all cached responses. Returns number of files removed."""
    if not CACHE_DIR.exists():
        return 0
    count = 0
    for p in CACHE_DIR.glob("*.json"):
        p.unlink()
        count += 1
    return count


def status() -> dict[str, Any]:
    """Return cache stats: entry count, size, oldest age."""
    if not CACHE_DIR.exists():
        return {"entries": 0, "size_bytes": 0, "oldest_hours": 0}

    entries = list(CACHE_DIR.glob("*.json"))
    if not entries:
        return {"entries": 0, "size_bytes": 0, "oldest_hours": 0}

    total_size = sum(p.stat().st_size for p in entries)
    now = time.time()
    oldest = now
    for p in entries:
        try:
            entry: dict[str, Any] = json.loads(p.read_text(encoding="utf-8"))
            cached_at = entry.get("cached_at", 0)
            if cached_at < oldest:
                oldest = cached_at
        except (json.JSONDecodeError, OSError):
            pass

    oldest_hours = round((now - oldest) / 3600, 1) if oldest < now else 0
    return {
        "entries": len(entries),
        "size_bytes": total_size,
        "oldest_hours": oldest_hours,
    }
