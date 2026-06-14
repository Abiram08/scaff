"""Multi-provider LLM client with rate limiting and token tracking."""
from __future__ import annotations

import json
import logging
import time
from dataclasses import dataclass
from typing import AsyncIterator, Optional

import httpx
from tenacity import retry, stop_after_attempt, wait_exponential

from .config import RelayConfig
from .rate_limit import RateLimiter
from .token_tracker import TokenTracker

logger = logging.getLogger(__name__)


@dataclass
class ChatMessage:
    role: str
    content: str

    @classmethod
    def system(cls, content: str) -> "ChatMessage":
        return cls(role="system", content=content)

    @classmethod
    def user(cls, content: str) -> "ChatMessage":
        return cls(role="user", content=content)


@dataclass
class ChatUsage:
    input_tokens: int = 0
    output_tokens: int = 0


@dataclass
class ChatResponse:
    content: str
    usage: ChatUsage
    model: str


class LLMClient:
    def __init__(self, config: RelayConfig, rate_limiter: RateLimiter | None = None,
                 token_tracker: TokenTracker | None = None):
        self.config = config
        self._http = httpx.AsyncClient(timeout=120.0)
        self._limiter = rate_limiter or RateLimiter()
        self._token_tracker = token_tracker or TokenTracker()
        self._session_id: str = ""
        self._stage: str = ""

    def set_context(self, session_id: str, stage: str = ""):
        self._session_id = session_id
        self._stage = stage

    async def close(self):
        await self._http.aclose()

    @retry(stop=stop_after_attempt(3), wait=wait_exponential(min=1, max=10))
    async def chat(
        self,
        messages: list[ChatMessage],
        model: Optional[str] = None,
        temperature: float = 0.3,
        max_tokens: int = 4096,
        api_key: Optional[str] = None,
    ) -> ChatResponse:
        model = model or self.config.llm.model
        provider = self.config.llm.provider

        provider_map = {"openai": "openai", "anthropic": "anthropic",
                        "gemini": "gemini", "groq": "groq", "ollama": "ollama"}
        provider_check = provider_map.get(provider, "openai")

        if api_key is None:
            api_key = self._get_api_key(provider_check)
        await self._limiter.acquire(provider)

        start = time.monotonic()
        try:
            if provider == "anthropic":
                resp = await self._chat_anthropic(messages, model, temperature, max_tokens, api_key)
            else:
                resp = await self._chat_openai_compat(messages, model, temperature, max_tokens, api_key)
        except Exception as e:
            error_str = str(e).lower()
            if "401" in error_str or "unauthorized" in error_str or "auth" in error_str or "invalid" in error_str:
                raise RuntimeError(
                    f"Invalid or missing API key for {provider}. Open the Settings (⚙️) to set your key."
                ) from e
            if "402" in error_str or "insufficient" in error_str or "quota" in error_str or "rate limit" in error_str:
                raise RuntimeError(
                    f"API rate limit or quota exceeded for {provider}. Check your billing."
                ) from e
            raise

        elapsed = int((time.monotonic() - start) * 1000)

        await self._token_tracker.record(
            provider=provider,
            model=resp.model,
            input_tokens=resp.usage.input_tokens,
            output_tokens=resp.usage.output_tokens,
            session_id=self._session_id,
            stage=self._stage,
            duration_ms=elapsed,
        )
        return resp

    def _get_api_key(self, provider: str) -> Optional[str]:
        """Get API key from the per-request store, fallback to config (env vars)."""
        if hasattr(self, '_key_override') and self._key_override:
            return self._key_override
        return self.config.get_api_key(provider)

    def set_api_key(self, api_key: str) -> None:
        """Set a per-request API key override (called from pipeline)."""
        self._key_override = api_key

    async def _chat_anthropic(
        self,
        messages: list[ChatMessage],
        model: str,
        temperature: float,
        max_tokens: int,
        api_key: Optional[str],
    ) -> ChatResponse:
        if not api_key:
            raise ValueError("ANTHROPIC_API_KEY not set")

        system_msg = ""
        user_msgs = []
        for m in messages:
            if m.role == "system":
                system_msg = m.content
            else:
                user_msgs.append({"role": m.role, "content": m.content})

        resp = await self._http.post(
            "https://api.anthropic.com/v1/messages",
            headers={
                "x-api-key": api_key,
                "anthropic-version": "2023-06-01",
                "content-type": "application/json",
            },
            json={
                "model": model,
                "max_tokens": max_tokens,
                "temperature": temperature,
                "system": system_msg,
                "messages": user_msgs,
            },
        )
        resp.raise_for_status()
        data = resp.json()

        content = ""
        for block in data.get("content", []):
            if block.get("type") == "text":
                content += block["text"]

        usage = data.get("usage", {})
        return ChatResponse(
            content=content,
            usage=ChatUsage(
                input_tokens=usage.get("input_tokens", 0),
                output_tokens=usage.get("output_tokens", 0),
            ),
            model=data.get("model", model),
        )

    async def _chat_openai_compat(
        self,
        messages: list[ChatMessage],
        model: str,
        temperature: float,
        max_tokens: int,
        api_key: Optional[str],
    ) -> ChatResponse:
        provider = self.config.llm.provider
        base_urls = {
            "openai": "https://api.openai.com/v1",
            "gemini": "https://generativelanguage.googleapis.com/v1beta/openai",
            "groq": "https://api.groq.com/openai/v1",
        }
        base_url = base_urls.get(provider, "https://api.openai.com/v1")

        if not api_key and provider != "ollama":
            raise ValueError(f"{provider.upper()}_API_KEY not set")

        headers = {"Content-Type": "application/json"}
        if api_key:
            headers["Authorization"] = f"Bearer {api_key}"

        msgs = [{"role": m.role, "content": m.content} for m in messages]

        resp = await self._http.post(
            f"{base_url}/chat/completions",
            headers=headers,
            json={
                "model": model,
                "messages": msgs,
                "temperature": temperature,
                "max_tokens": max_tokens,
            },
        )
        resp.raise_for_status()
        data = resp.json()

        content = data["choices"][0]["message"]["content"]
        usage = data.get("usage", {})
        return ChatResponse(
            content=content,
            usage=ChatUsage(
                input_tokens=usage.get("prompt_tokens", 0),
                output_tokens=usage.get("completion_tokens", 0),
            ),
            model=data.get("model", model),
        )

    async def stream(
        self,
        messages: list[ChatMessage],
        model: Optional[str] = None,
        temperature: float = 0.3,
        max_tokens: int = 4096,
    ) -> AsyncIterator[str]:
        model = model or self.config.llm.model
        provider = self.config.llm.provider
        api_key = self.config.get_api_key()

        await self._limiter.acquire(provider)

        if provider == "anthropic":
            async for chunk in self._stream_anthropic(messages, model, temperature, max_tokens, api_key):
                yield chunk
        else:
            async for chunk in self._stream_openai_compat(messages, model, temperature, max_tokens, api_key):
                yield chunk

    async def _stream_anthropic(
        self,
        messages: list[ChatMessage],
        model: str,
        temperature: float,
        max_tokens: int,
        api_key: Optional[str],
    ) -> AsyncIterator[str]:
        if not api_key:
            raise ValueError("ANTHROPIC_API_KEY not set")

        system_msg = ""
        user_msgs = []
        for m in messages:
            if m.role == "system":
                system_msg = m.content
            else:
                user_msgs.append({"role": m.role, "content": m.content})

        async with self._http.stream(
            "POST",
            "https://api.anthropic.com/v1/messages",
            headers={
                "x-api-key": api_key,
                "anthropic-version": "2023-06-01",
                "content-type": "application/json",
            },
            json={
                "model": model,
                "max_tokens": max_tokens,
                "temperature": temperature,
                "system": system_msg,
                "messages": user_msgs,
                "stream": True,
            },
        ) as resp:
            resp.raise_for_status()
            async for line in resp.aiter_lines():
                if line.startswith("data: "):
                    data = line[6:]
                    if data == "[DONE]":
                        break
                    try:
                        event = json.loads(data)
                        if event.get("type") == "content_block_delta":
                            delta = event.get("delta", {})
                            if delta.get("type") == "text_delta":
                                yield delta["text"]
                    except json.JSONDecodeError:
                        continue

    async def _stream_openai_compat(
        self,
        messages: list[ChatMessage],
        model: str,
        temperature: float,
        max_tokens: int,
        api_key: Optional[str],
    ) -> AsyncIterator[str]:
        provider = self.config.llm.provider
        base_urls = {
            "openai": "https://api.openai.com/v1",
            "gemini": "https://generativelanguage.googleapis.com/v1beta/openai",
            "groq": "https://api.groq.com/openai/v1",
        }
        base_url = base_urls.get(provider, "https://api.openai.com/v1")

        headers = {"Content-Type": "application/json"}
        if api_key:
            headers["Authorization"] = f"Bearer {api_key}"

        msgs = [{"role": m.role, "content": m.content} for m in messages]

        async with self._http.stream(
            "POST",
            f"{base_url}/chat/completions",
            headers=headers,
            json={
                "model": model,
                "messages": msgs,
                "temperature": temperature,
                "max_tokens": max_tokens,
                "stream": True,
            },
        ) as resp:
            resp.raise_for_status()
            async for line in resp.aiter_lines():
                if line.startswith("data: "):
                    data = line[6:]
                    if data == "[DONE]":
                        break
                    try:
                        event = json.loads(data)
                        delta = event["choices"][0].get("delta", {})
                        if "content" in delta:
                            yield delta["content"]
                    except (json.JSONDecodeError, KeyError, IndexError):
                        continue
