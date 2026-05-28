"""Configuration management for scaff."""

import json
import os
from pathlib import Path
from typing import Any, Optional


class ScaffConfig:
    """Manage scaff configuration."""
    
    CONFIG_DIR = Path.home() / ".scaff"
    CONFIG_FILE = CONFIG_DIR / "config.json"
    
    DEFAULT_CONFIG = {
        "model": "gpt-4o",
        "output_dir": "./agent-output",
        "default_dependencies": ["requests", "python-dotenv"],
        "template_style": "modern",
    }
    
    @classmethod
    def ensure_config_dir(cls) -> None:
        """Ensure config directory exists."""
        cls.CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    
    @classmethod
    def load_config(cls) -> dict[str, Any]:
        """Load configuration from file or return defaults."""
        cls.ensure_config_dir()
        
        if cls.CONFIG_FILE.exists():
            try:
                with open(cls.CONFIG_FILE, "r", encoding="utf-8") as f:
                    config = json.load(f)
                return {**cls.DEFAULT_CONFIG, **config}
            except (json.JSONDecodeError, IOError):
                return cls.DEFAULT_CONFIG.copy()
        
        return cls.DEFAULT_CONFIG.copy()
    
    @classmethod
    def save_config(cls, config: dict[str, Any]) -> None:
        """Save configuration to file."""
        cls.ensure_config_dir()
        
        with open(cls.CONFIG_FILE, "w", encoding="utf-8") as f:
            json.dump(config, f, indent=2)
    
    @classmethod
    def get(cls, key: str, default: Optional[Any] = None) -> Any:
        """Get a configuration value."""
        config = cls.load_config()
        return config.get(key, default)
    
    @classmethod
    def set(cls, key: str, value: Any) -> None:
        """Set a configuration value."""
        config = cls.load_config()
        config[key] = value
        cls.save_config(config)
    
    @classmethod
    def reset(cls) -> None:
        """Reset configuration to defaults."""
        cls.save_config(cls.DEFAULT_CONFIG.copy())
