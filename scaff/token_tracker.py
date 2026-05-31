"""Token tracker for tracking usage and costs."""

import json
import hashlib
from pathlib import Path
from datetime import datetime
from typing import Dict, Any, Optional

from .config import ScaffConfig


class TokenTracker:
    """
    Track token usage and costs per session and monthly.
    Persistent storage with JSON backend.
    """
    
    def __init__(self, data_file: Optional[Path] = None):
        """Initialize tracker"""
        self.tracker_file = data_file or (ScaffConfig.CONFIG_DIR / "token_usage.json")
        self.current_month = datetime.now().strftime("%Y-%m")
        self.session_id = None
        self.session_data = {
            "start_time": datetime.now().isoformat(),
            "tokens_used": 0,
            "cost": 0.0,
            "requests": [],
        }
    
    def start_session(self, model: str = "gpt-4o") -> str:
        """Start a new tracking session"""
        import uuid
        self.session_id = str(uuid.uuid4())[:8]
        self.session_data = {
            "start_time": datetime.now().isoformat(),
            "model": model,
            "tokens_used": 0,
            "cost": 0.0,
            "requests": [],
        }
        return self.session_id
    
    def record_request(
        self,
        session_id: str,
        model: str,
        input_tokens: int,
        output_tokens: int,
        cost: float,
        description: Optional[str] = None,
    ) -> None:
        """Record a single API request"""
        self.session_data["requests"].append({
            "timestamp": datetime.now().isoformat(),
            "description_hash": self._hash_description(description or ""),
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens,
            "model": model,
            "cost": cost,
        })
        
        self.session_data["tokens_used"] += input_tokens + output_tokens
        self.session_data["cost"] += cost
    
    def end_session(self) -> Dict[str, Any]:
        """End session and save data"""
        self.session_data["end_time"] = datetime.now().isoformat()
        
        # Save to persistent storage
        data = self._load_tracker_file()
        
        if self.current_month not in data:
            data[self.current_month] = {
                "total_tokens": 0,
                "total_cost": 0.0,
                "sessions": [],
            }
        
        month_data = data[self.current_month]
        month_data["total_tokens"] += self.session_data["tokens_used"]
        month_data["total_cost"] += self.session_data["cost"]
        month_data["sessions"].append({
            "session_id": self.session_id,
            **self.session_data,
        })
        
        self._save_tracker_file(data)
        
        return self.session_data
    
    def get_monthly_stats(self, month: Optional[str] = None) -> Dict[str, Any]:
        """Get stats for a specific month"""
        if month is None:
            month = self.current_month
        
        data = self._load_tracker_file()
        month_data = data.get(month, {})
        
        return {
            "month": month,
            "total_tokens": month_data.get("total_tokens", 0),
            "total_cost": month_data.get("total_cost", 0.0),
            "session_count": len(month_data.get("sessions", [])),
            "estimated_requests": month_data.get("total_tokens", 0) // 500,
        }
    
    def get_all_time_stats(self) -> Dict[str, Any]:
        """Get all-time statistics"""
        data = self._load_tracker_file()
        
        total_tokens = sum(m.get("total_tokens", 0) for m in data.values())
        total_cost = sum(m.get("total_cost", 0.0) for m in data.values())
        total_sessions = sum(len(m.get("sessions", [])) for m in data.values())
        
        return {
            "total_tokens": total_tokens,
            "total_cost": total_cost,
            "total_sessions": total_sessions,
            "average_cost_per_session": (
                total_cost / total_sessions if total_sessions > 0 else 0
            ),
            "months_tracked": len(data),
        }
    
    @staticmethod
    def _hash_description(description: str) -> str:
        """Hash description for privacy"""
        return hashlib.sha256(description.encode()).hexdigest()[:16]
    
    def _load_tracker_file(self) -> Dict[str, Any]:
        """Load tracker data from file"""
        self.tracker_file.parent.mkdir(parents=True, exist_ok=True)
        
        if self.tracker_file.exists():
            try:
                return json.loads(self.tracker_file.read_text())
            except json.JSONDecodeError:
                return {}
        return {}
    
    def _save_tracker_file(self, data: Dict[str, Any]) -> None:
        """Save tracker data to file"""
        self.tracker_file.parent.mkdir(parents=True, exist_ok=True)
        self.tracker_file.write_text(
            json.dumps(data, indent=2),
            encoding="utf-8",
        )
