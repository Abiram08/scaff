"""Analytics module for tracking scaff usage metrics and performance."""

import json
import time
from pathlib import Path
from datetime import datetime, timedelta
from typing import Dict, Any, Optional, List
from dataclasses import dataclass, asdict


@dataclass
class APICallMetric:
    """Single API call metric"""
    timestamp: str
    model: str
    input_tokens: int
    output_tokens: int
    latency_ms: float
    cost_usd: float
    success: bool
    error_code: Optional[str] = None


@dataclass
class SessionMetric:
    """Session-level metric"""
    session_id: str
    start_time: str
    end_time: Optional[str]
    total_calls: int
    total_tokens: int
    total_cost: float
    peak_memory_mb: float
    status: str  # "active", "completed", "failed"


class Analytics:
    """Track and analyze scaff usage metrics"""
    
    def __init__(self, analytics_dir: Optional[Path] = None):
        """Initialize analytics"""
        self.analytics_dir = analytics_dir or Path.home() / ".scaff" / "analytics"
        self.analytics_dir.mkdir(parents=True, exist_ok=True)
        
        self.api_calls_file = self.analytics_dir / "api_calls.jsonl"
        self.sessions_file = self.analytics_dir / "sessions.jsonl"
        self.daily_stats_file = self.analytics_dir / "daily_stats.json"
    
    def log_api_call(
        self,
        model: str,
        input_tokens: int,
        output_tokens: int,
        latency_ms: float,
        cost_usd: float,
        success: bool = True,
        error_code: Optional[str] = None,
    ) -> None:
        """Log an API call"""
        metric = APICallMetric(
            timestamp=datetime.now().isoformat(),
            model=model,
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            latency_ms=latency_ms,
            cost_usd=cost_usd,
            success=success,
            error_code=error_code,
        )
        
        try:
            with open(self.api_calls_file, "a") as f:
                f.write(json.dumps(asdict(metric)) + "\n")
        except (IOError, OSError):
            pass
    
    def log_session(self, session_metric: SessionMetric) -> None:
        """Log a session"""
        try:
            with open(self.sessions_file, "a") as f:
                f.write(json.dumps(asdict(session_metric)) + "\n")
        except (IOError, OSError):
            pass
    
    def get_api_calls(
        self,
        hours: int = 24,
        model: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        """Get API calls from last N hours"""
        if not self.api_calls_file.exists():
            return []
        
        cutoff = datetime.now() - timedelta(hours=hours)
        calls = []
        
        try:
            with open(self.api_calls_file, "r") as f:
                for line in f:
                    try:
                        call = json.loads(line)
                        ts = datetime.fromisoformat(call["timestamp"])
                        
                        if ts > cutoff:
                            if model is None or call["model"] == model:
                                calls.append(call)
                    except (json.JSONDecodeError, ValueError):
                        continue
        except (IOError, OSError):
            pass
        
        return calls
    
    def get_sessions(self, days: int = 30) -> List[Dict[str, Any]]:
        """Get sessions from last N days"""
        if not self.sessions_file.exists():
            return []
        
        cutoff = datetime.now() - timedelta(days=days)
        sessions = []
        
        try:
            with open(self.sessions_file, "r") as f:
                for line in f:
                    try:
                        session = json.loads(line)
                        ts = datetime.fromisoformat(session["start_time"])
                        
                        if ts > cutoff:
                            sessions.append(session)
                    except (json.JSONDecodeError, ValueError):
                        continue
        except (IOError, OSError):
            pass
        
        return sessions
    
    def get_daily_stats(self) -> Dict[str, Any]:
        """Get aggregated daily statistics"""
        calls = self.get_api_calls(hours=24)
        sessions = self.get_sessions(days=1)
        
        total_calls = len(calls)
        successful_calls = sum(1 for c in calls if c.get("success", True))
        failed_calls = total_calls - successful_calls
        total_tokens = sum(c.get("input_tokens", 0) + c.get("output_tokens", 0) for c in calls)
        total_cost = sum(c.get("cost_usd", 0) for c in calls)
        avg_latency = (sum(c.get("latency_ms", 0) for c in calls) / total_calls) if total_calls > 0 else 0
        
        return {
            "date": datetime.now().strftime("%Y-%m-%d"),
            "api_calls": total_calls,
            "successful_calls": successful_calls,
            "failed_calls": failed_calls,
            "total_tokens": total_tokens,
            "total_cost_usd": round(total_cost, 4),
            "avg_latency_ms": round(avg_latency, 2),
            "active_sessions": len([s for s in sessions if s.get("status") == "active"]),
            "completed_sessions": len([s for s in sessions if s.get("status") == "completed"]),
        }
    
    def get_monthly_stats(self, month: Optional[str] = None) -> Dict[str, Any]:
        """Get aggregated monthly statistics"""
        if month is None:
            month = datetime.now().strftime("%Y-%m")
        
        calls = self.get_api_calls(hours=24*31)  # Approximate month
        
        month_calls = [
            c for c in calls 
            if c["timestamp"].startswith(month)
        ]
        
        total_calls = len(month_calls)
        successful = sum(1 for c in month_calls if c.get("success", True))
        total_tokens = sum(c.get("input_tokens", 0) + c.get("output_tokens", 0) for c in month_calls)
        total_cost = sum(c.get("cost_usd", 0) for c in month_calls)
        
        return {
            "month": month,
            "api_calls": total_calls,
            "successful_calls": successful,
            "failed_calls": total_calls - successful,
            "total_tokens": total_tokens,
            "total_cost_usd": round(total_cost, 4),
            "avg_cost_per_call": round(total_cost / total_calls, 4) if total_calls > 0 else 0,
        }
    
    def get_model_stats(self) -> Dict[str, Dict[str, Any]]:
        """Get statistics by model"""
        calls = self.get_api_calls(hours=24*7)  # Last week
        
        stats_by_model: Dict[str, Dict[str, Any]] = {}
        
        for call in calls:
            model = call.get("model", "unknown")
            
            if model not in stats_by_model:
                stats_by_model[model] = {
                    "calls": 0,
                    "tokens": 0,
                    "cost": 0.0,
                    "latency_ms": [],
                }
            
            stats = stats_by_model[model]
            stats["calls"] += 1
            stats["tokens"] += call.get("input_tokens", 0) + call.get("output_tokens", 0)
            stats["cost"] += call.get("cost_usd", 0)
            stats["latency_ms"].append(call.get("latency_ms", 0))
        
        # Calculate averages
        for model, stats in stats_by_model.items():
            latencies = stats.pop("latency_ms")
            stats["avg_latency_ms"] = round(sum(latencies) / len(latencies), 2) if latencies else 0
            stats["cost"] = round(stats["cost"], 4)
        
        return stats_by_model
