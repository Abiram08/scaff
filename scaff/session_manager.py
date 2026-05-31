"""Session and memory management for scaff."""

import logging
import tracemalloc
import psutil
import uuid
from dataclasses import dataclass
from enum import Enum
from datetime import datetime
from typing import Dict, Any, Optional

logger = logging.getLogger(__name__)


class MemoryProfile(Enum):
    """Memory usage profiles"""
    CONSERVATIVE = "conservative"  # 256 MB max
    BALANCED = "balanced"         # 512 MB max
    AGGRESSIVE = "aggressive"     # 2 GB max


@dataclass
class MemoryLimits:
    """Memory limits for a profile"""
    profile: MemoryProfile
    max_memory_mb: int
    session_timeout_minutes: int
    cache_ttl_hours: int
    
    @staticmethod
    def get_for_profile(profile: MemoryProfile) -> "MemoryLimits":
        """Get limits for a profile"""
        limits = {
            MemoryProfile.CONSERVATIVE: MemoryLimits(
                profile=MemoryProfile.CONSERVATIVE,
                max_memory_mb=256,
                session_timeout_minutes=30,
                cache_ttl_hours=24,
            ),
            MemoryProfile.BALANCED: MemoryLimits(
                profile=MemoryProfile.BALANCED,
                max_memory_mb=512,
                session_timeout_minutes=120,
                cache_ttl_hours=72,
            ),
            MemoryProfile.AGGRESSIVE: MemoryLimits(
                profile=MemoryProfile.AGGRESSIVE,
                max_memory_mb=2048,
                session_timeout_minutes=480,
                cache_ttl_hours=168,
            ),
        }
        return limits[profile]


class MemoryMonitor:
    """Monitor memory usage in real-time with tracemalloc leak detection"""

    BASELINE_MB = 50
    _tracemalloc_started = False

    @classmethod
    def start_tracemalloc(cls) -> None:
        """Start tracemalloc for memory leak detection."""
        if not cls._tracemalloc_started:
            tracemalloc.start(25)
            cls._tracemalloc_started = True

    @classmethod
    def get_tracemalloc_stats(cls) -> Dict[str, Any]:
        """Get tracemalloc snapshot stats for leak analysis."""
        if not tracemalloc.is_tracing():
            return {"size": 0, "peak": 0, "traceback_filtered_count": 0}
        snapshot = tracemalloc.take_snapshot()
        stats = snapshot.statistics("lineno")
        total_size = sum(s.size for s in stats)
        total_count = sum(s.count for s in stats)
        return {
            "size": total_size,
            "peak": tracemalloc.get_tracemalloc_memory(),
            "traceback_filtered_count": total_count,
        }

    @staticmethod
    def get_process_memory_mb() -> float:
        """Get current process memory in MB"""
        process = psutil.Process()
        return process.memory_info().rss / 1024 / 1024

    @staticmethod
    def get_available_memory_mb() -> float:
        """Get available system memory in MB"""
        return psutil.virtual_memory().available / 1024 / 1024

    @staticmethod
    def get_memory_percent() -> float:
        """Get memory usage as % of total"""
        return psutil.virtual_memory().percent


class SessionManager:
    """
    Manage agent generation sessions with memory tracking
    and resource enforcement.
    """
    
    def __init__(self, profile: MemoryProfile = MemoryProfile.BALANCED):
        """Initialize session manager"""
        self.profile = profile
        self.limits = MemoryLimits.get_for_profile(profile)
        self.session_id = str(uuid.uuid4())[:8]
        self.start_time = datetime.now()
        self.initial_memory_mb = MemoryMonitor.get_process_memory_mb()
        self.peak_memory_mb = self.initial_memory_mb
        self.request_count = 0
    
    def check_memory_limit(self) -> Dict[str, Any]:
        """Check if memory usage is within limits"""
        current_memory = MemoryMonitor.get_process_memory_mb()
        self.peak_memory_mb = max(self.peak_memory_mb, current_memory)
        
        delta_mb = current_memory - MemoryMonitor.BASELINE_MB
        percent_of_limit = (delta_mb / self.limits.max_memory_mb) * 100 if self.limits.max_memory_mb > 0 else 0
        
        exceeds_limit = delta_mb > self.limits.max_memory_mb
        
        return {
            "current_mb": current_memory,
            "delta_mb": delta_mb,
            "limit_mb": self.limits.max_memory_mb,
            "percent_of_limit": percent_of_limit,
            "exceeds_limit": exceeds_limit,
            "warning_level": self._get_warning_level(percent_of_limit),
        }
    
    def check_session_timeout(self) -> bool:
        """Check if session has timed out"""
        elapsed_minutes = (datetime.now() - self.start_time).total_seconds() / 60
        return elapsed_minutes > self.limits.session_timeout_minutes
    
    def get_session_stats(self) -> Dict[str, Any]:
        """Get session statistics with memory leak metrics"""
        elapsed = (datetime.now() - self.start_time).total_seconds()
        current_memory = MemoryMonitor.get_process_memory_mb()
        memory_growth = current_memory - self.initial_memory_mb
        tracemalloc_stats = MemoryMonitor.get_tracemalloc_stats()
        
        return {
            "session_id": self.session_id,
            "profile": self.profile.value,
            "elapsed_seconds": elapsed,
            "initial_memory_mb": self.initial_memory_mb,
            "current_memory_mb": current_memory,
            "peak_memory_mb": self.peak_memory_mb,
            "memory_growth_mb": round(memory_growth, 2),
            "request_count": self.request_count,
            "leak_detection": {
                "tracemalloc_size_bytes": tracemalloc_stats["size"],
                "tracemalloc_peak_bytes": tracemalloc_stats["peak"],
                "allocated_objects": tracemalloc_stats["traceback_filtered_count"],
            },
        }
    
    @staticmethod
    def _get_warning_level(percent_of_limit: float) -> str:
        """Get warning level based on memory usage"""
        if percent_of_limit < 50:
            return "normal"
        elif percent_of_limit < 70:
            return "info"
        elif percent_of_limit < 85:
            return "warning"
        elif percent_of_limit < 95:
            return "critical"
        else:
            return "fatal"
