"""Relay configuration."""
from __future__ import annotations

import os
from pathlib import Path
from dataclasses import dataclass, field
from typing import Optional


@dataclass
class LLMConfig:
    provider: str = "anthropic"
    model: str = "claude-sonnet-4-20250514"
    max_tokens: int = 4096
    temperature: float = 0.3


@dataclass
class SearchConfig:
    web_provider: str = "tavily"
    harness_corpus_top_k: int = 8
    web_top_k: int = 5
    max_search_queries: int = 4


@dataclass
class EmbeddingConfig:
    model: str = "text-embedding-3-small"
    provider: str = "openai"
    chunk_size: int = 512
    chunk_overlap: int = 64


@dataclass
class CorpusConfig:
    db_path: str = "./corpus/harness.db"
    refresh_interval_hours: int = 168
    embedding: EmbedingConfig = field(default_factory=EmbedingConfig)


@dataclass
class VerificationConfig:
    min_sources_for_high: int = 2
    flag_contradictions: bool = True


@dataclass
class StreamingConfig:
    chunk_on: str = "sub_question_complete"
    enable_user_commands: bool = True


@dataclass
class APIConfig:
    host: str = "0.0.0.0"
    port: int = 8000


@dataclass
class RelayConfig:
    llm: LLMConfig = field(default_factory=LLMConfig)
    search: SearchConfig = field(default_factory=SearchConfig)
    corpus: CorpusConfig = field(default_factory=CorpusConfig)
    verification: VerificationConfig = field(default_factory=VerificationConfig)
    streaming: StreamingConfig = field(default_factory=StreamingConfig)
    api: APIConfig = field(default_factory=APIConfig)

    @classmethod
    def from_env(cls) -> "RelayConfig":
        return cls(
            llm=LLMConfig(
                provider=os.getenv("RELAY_LLM_PROVIDER", "anthropic"),
                model=os.getenv("RELAY_LLM_MODEL", "claude-sonnet-4-20250514"),
            ),
            search=SearchConfig(
                web_provider=os.getenv("RELAY_SEARCH_PROVIDER", "tavily"),
            ),
            corpus=CorpusConfig(
                db_path=os.getenv("RELAY_CORPUS_DB", "./corpus/harness.db"),
            ),
        )

    def get_api_key(self, provider: Optional[str] = None) -> Optional[str]:
        provider = provider or self.llm.provider
        env_map = {
            "openai": "OPENAI_API_KEY",
            "anthropic": "ANTHROPIC_API_KEY",
            "gemini": "GEMINI_API_KEY",
            "groq": "GROQ_API_KEY",
        }
        key_name = env_map.get(provider)
        if key_name:
            return os.getenv(key_name)
        return None
