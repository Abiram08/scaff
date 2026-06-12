"""Web search with multiple providers."""
from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Optional
from urllib.parse import quote_plus

import httpx
from tenacity import retry, stop_after_attempt, wait_exponential


@dataclass
class WebResult:
    title: str
    url: str
    snippet: str
    provider: str = "duckduckgo"


class WebSearcher:
    def __init__(self, provider: str = "tavily", api_key: Optional[str] = None):
        self.provider = provider
        self.api_key = api_key
        self._http = httpx.AsyncClient(timeout=30.0)

    async def close(self):
        await self._http.aclose()

    @retry(stop=stop_after_attempt(3), wait=wait_exponential(min=1, max=5))
    async def search(self, query: str, max_results: int = 5) -> list[WebResult]:
        if self.provider == "tavily" and self.api_key:
            return await self._search_tavily(query, max_results)
        elif self.provider == "brave" and self.api_key:
            return await self._search_brave(query, max_results)
        else:
            return await self._search_duckduckgo(query, max_results)

    async def _search_tavily(self, query: str, max_results: int) -> list[WebResult]:
        resp = await self._http.post(
            "https://api.tavily.com/search",
            json={
                "api_key": self.api_key,
                "query": query,
                "max_results": max_results,
                "include_answer": False,
            },
        )
        resp.raise_for_status()
        data = resp.json()

        results = []
        for r in data.get("results", []):
            results.append(WebResult(
                title=r.get("title", ""),
                url=r.get("url", ""),
                snippet=r.get("content", ""),
                provider="tavily",
            ))
        return results

    async def _search_brave(self, query: str, max_results: int) -> list[WebResult]:
        resp = await self._http.get(
            "https://api.search.brave.com/res/v1/web/search",
            headers={"X-Subscription-Token": self.api_key},
            params={"q": query, "count": max_results},
        )
        resp.raise_for_status()
        data = resp.json()

        results = []
        for r in data.get("web", {}).get("results", []):
            results.append(WebResult(
                title=r.get("title", ""),
                url=r.get("url", ""),
                snippet=r.get("description", ""),
                provider="brave",
            ))
        return results

    async def _search_duckduckgo(self, query: str, max_results: int) -> list[WebResult]:
        url = f"https://html.duckduckgo.com/html/?q={quote_plus(query)}"
        resp = await self._http.get(
            url,
            headers={"User-Agent": "relay-research/1.0"},
        )
        resp.raise_for_status()
        html = resp.text

        results = []
        result_pattern = re.compile(
            r'<a[^>]*class="result__a"[^>]*href="([^"]*)"[^>]*>(.*?)</a>.*?'
            r'<a[^>]*class="result__snippet"[^>]*>(.*?)</a>',
            re.DOTALL,
        )

        for match in result_pattern.finditer(html):
            if len(results) >= max_results:
                break
            url, title, snippet = match.groups()
            title = re.sub(r'<[^>]+>', '', title).strip()
            snippet = re.sub(r'<[^>]+>', '', snippet).strip()
            if title and url:
                results.append(WebResult(
                    title=title,
                    url=url,
                    snippet=snippet,
                    provider="duckduckgo",
                ))

        return results

    async def fetch_page(self, url: str, max_chars: int = 5000) -> str:
        try:
            resp = await self._http.get(
                url,
                headers={"User-Agent": "relay-research/1.0"},
                follow_redirects=True,
            )
            resp.raise_for_status()
            html = resp.text

            text = re.sub(r'<script[^>]*>.*?</script>', '', html, flags=re.DOTALL)
            text = re.sub(r'<style[^>]*>.*?</style>', '', text, flags=re.DOTALL)
            text = re.sub(r'<[^>]+>', ' ', text)
            text = re.sub(r'\s+', ' ', text).strip()

            if len(text) > max_chars:
                text = text[:max_chars] + "..."

            return text
        except Exception:
            return ""


def generate_search_queries(question: str, sub_questions: list[str], max_queries: int = 6) -> list[str]:
    queries = [question]
    for sq in sub_questions[:max_queries - 1]:
        if sq not in queries:
            queries.append(sq)

    stop_words = {
        "what", "how", "why", "when", "where", "which", "who", "is", "are", "was", "were",
        "do", "does", "did", "can", "could", "should", "would", "will", "shall", "may",
        "might", "the", "a", "an", "in", "on", "at", "to", "for", "of", "with", "by",
    }

    words = question.split()
    current = []
    for word in words:
        clean = re.sub(r'[^a-zA-Z0-9-]', '', word).lower()
        if clean in stop_words or len(clean) < 3:
            if current:
                term = " ".join(current)
                if term not in queries:
                    queries.append(term)
                current = []
        else:
            current.append(word)

    if current:
        term = " ".join(current)
        if term not in queries:
            queries.append(term)

    return queries[:max_queries]
