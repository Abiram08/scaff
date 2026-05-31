"""Persistent state management for scaff."""

import json
from pathlib import Path
from typing import Dict, Any, Optional
from datetime import datetime
from dataclasses import dataclass, asdict


@dataclass
class ApplicationState:
    """Application-wide state"""
    last_session_id: Optional[str] = None
    last_update: str = ""
    total_agents_generated: int = 0
    total_api_calls: int = 0
    current_mode: str = "medium"
    cache_enabled: bool = True
    verbose_mode: bool = False


class StateManager:
    """Manage persistent application state"""
    
    def __init__(self, state_dir: Optional[Path] = None):
        """Initialize state manager"""
        self.state_dir = state_dir or Path.home() / ".scaff"
        self.state_dir.mkdir(parents=True, exist_ok=True)
        self.state_file = self.state_dir / "app_state.json"
        self._load_state()
    
    def _load_state(self) -> None:
        """Load state from file"""
        if self.state_file.exists():
            try:
                data = json.loads(self.state_file.read_text())
                self.state = ApplicationState(**data)
            except (json.JSONDecodeError, TypeError):
                self.state = ApplicationState()
        else:
            self.state = ApplicationState()
    
    def save(self) -> None:
        """Save state to file"""
        self.state.last_update = datetime.now().isoformat()
        try:
            self.state_file.write_text(json.dumps(asdict(self.state), indent=2))
        except (IOError, OSError):
            pass
    
    def get(self, key: str, default: Any = None) -> Any:
        """Get state value"""
        return getattr(self.state, key, default)
    
    def set(self, key: str, value: Any) -> None:
        """Set state value"""
        if hasattr(self.state, key):
            setattr(self.state, key, value)
            self.save()
    
    def increment(self, key: str, amount: int = 1) -> None:
        """Increment an integer counter"""
        if hasattr(self.state, key):
            current = getattr(self.state, key, 0)
            if isinstance(current, int):
                setattr(self.state, key, current + amount)
                self.save()
    
    def get_all(self) -> Dict[str, Any]:
        """Get all state"""
        return asdict(self.state)
    
    def reset(self) -> None:
        """Reset state to defaults"""
        self.state = ApplicationState()
        self.save()


class SessionState:
    """Manage session-specific state"""
    
    def __init__(self, session_id: str, state_dir: Optional[Path] = None):
        """Initialize session state"""
        self.session_id = session_id
        self.state_dir = state_dir or Path.home() / ".scaff" / "sessions"
        self.state_dir.mkdir(parents=True, exist_ok=True)
        self.state_file = self.state_dir / f"{session_id}.json"
        
        self.data = {
            "session_id": session_id,
            "created_at": datetime.now().isoformat(),
            "updated_at": datetime.now().isoformat(),
            "data": {},
        }
        
        self._load()
    
    def _load(self) -> None:
        """Load session state from file"""
        if self.state_file.exists():
            try:
                self.data = json.loads(self.state_file.read_text())
            except json.JSONDecodeError:
                self._save()
    
    def _save(self) -> None:
        """Save session state to file"""
        self.data["updated_at"] = datetime.now().isoformat()
        try:
            self.state_file.write_text(json.dumps(self.data, indent=2))
        except (IOError, OSError):
            pass
    
    def get(self, key: str, default: Any = None) -> Any:
        """Get value from session"""
        return self.data.get("data", {}).get(key, default)
    
    def set(self, key: str, value: Any) -> None:
        """Set value in session"""
        if "data" not in self.data:
            self.data["data"] = {}
        self.data["data"][key] = value
        self._save()
    
    def get_all(self) -> Dict[str, Any]:
        """Get all session data"""
        return self.data.get("data", {})
    
    def delete(self) -> None:
        """Delete session state file"""
        try:
            if self.state_file.exists():
                self.state_file.unlink()
        except (IOError, OSError):
            pass
    
    def exists(self) -> bool:
        """Check if session state exists"""
        return self.state_file.exists()
