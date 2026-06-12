"""Harness docs crawler."""
from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Optional
from urllib.parse import urljoin, urlparse

import httpx
from bs4 import BeautifulSoup

from .store import CorpusChunk


@dataclass
class CrawlResult:
    chunks: list[CorpusChunk]
    urls_crawled: int
    errors: list[str]


class HarnessCrawler:
    BASE_URL = "https://developer.harness.io"
    DOCS_PATH = "/docs"

    def __init__(self):
        self._http = httpx.AsyncClient(
            timeout=30.0,
            follow_redirects=True,
            headers={"User-Agent": "relay-crawler/1.0"},
        )

    async def close(self):
        await self._http.aclose()

    async def crawl_docs(self, max_pages: int = 50) -> CrawlResult:
        chunks = []
        errors = []
        urls_crawled = 0
        visited = set()

        to_visit = [
            f"{self.BASE_URL}{self.DOCS_PATH}",
            f"{self.BASE_URL}{self.DOCS_PATH}/continuous-delivery",
            f"{self.BASE_URL}{self.DOCS_PATH}/continuous-integration",
            f"{self.BASE_URL}{self.DOCS_PATH}/feature-flags",
            f"{self.BASE_URL}{self.DOCS_PATH}/cloud-cost-management",
            f"{self.BASE_URL}{self.DOCS_PATH}/security-testing-orchestration",
            f"{self.BASE_URL}{self.DOCS_PATH}/service-reliability-management",
        ]

        while to_visit and urls_crawled < max_pages:
            url = to_visit.pop(0)
            if url in visited:
                continue
            visited.add(url)

            try:
                page_chunks = await self._crawl_page(url)
                chunks.extend(page_chunks)
                urls_crawled += 1

                links = await self._extract_links(url)
                for link in links:
                    if link not in visited and self._is_docs_url(link):
                        to_visit.append(link)

            except Exception as e:
                errors.append(f"{url}: {e}")

        return CrawlResult(chunks=chunks, urls_crawled=urls_crawled, errors=errors)

    async def _crawl_page(self, url: str) -> list[CorpusChunk]:
        resp = await self._http.get(url)
        resp.raise_for_status()

        soup = BeautifulSoup(resp.text, "html.parser")

        title = self._extract_title(soup)
        content = self._extract_content(soup)

        if not content or len(content) < 50:
            return []

        chunks = []
        sections = self._split_into_sections(content)

        for i, (section_title, section_content) in enumerate(sections):
            if len(section_content.strip()) < 20:
                continue

            chunk_id = self._make_id(url, i)
            chunks.append(CorpusChunk(
                id=chunk_id,
                url=url,
                title=title,
                section=section_title,
                content=section_content.strip(),
                source="harness-docs",
            ))

        if not chunks and content:
            chunks.append(CorpusChunk(
                id=self._make_id(url, 0),
                url=url,
                title=title,
                section=None,
                content=content[:2000],
                source="harness-docs",
            ))

        return chunks

    async def _extract_links(self, url: str) -> list[str]:
        try:
            resp = await self._http.get(url)
            resp.raise_for_status()
            soup = BeautifulSoup(resp.text, "html.parser")
            links = []
            for a in soup.find_all("a", href=True):
                href = a["href"]
                full_url = urljoin(url, href)
                if full_url.startswith(self.BASE_URL):
                    links.append(full_url.split("#")[0])
            return list(set(links))
        except Exception:
            return []

    def _is_docs_url(self, url: str) -> bool:
        return url.startswith(f"{self.BASE_URL}{self.DOCS_PATH}")

    def _extract_title(self, soup: BeautifulSoup) -> str:
        h1 = soup.find("h1")
        if h1:
            return h1.get_text(strip=True)
        title = soup.find("title")
        if title:
            return title.get_text(strip=True)
        return "Untitled"

    def _extract_content(self, soup: BeautifulSoup) -> str:
        for tag in soup.find_all(["script", "style", "nav", "footer", "header"]):
            tag.decompose()

        main = soup.find("main") or soup.find("article") or soup.find("body")
        if not main:
            return ""

        text = main.get_text(separator="\n", strip=True)
        text = re.sub(r'\n{3,}', '\n\n', text)
        return text

    def _split_into_sections(self, content: str) -> list[tuple[Optional[str], str]]:
        lines = content.split("\n")
        sections = []
        current_title = None
        current_content = []

        for line in lines:
            if re.match(r'^#{1,3}\s+', line):
                if current_content:
                    sections.append((current_title, "\n".join(current_content)))
                current_title = re.sub(r'^#{1,3}\s+', '', line).strip()
                current_content = []
            else:
                current_content.append(line)

        if current_content:
            sections.append((current_title, "\n".join(current_content)))

        return sections

    def _make_id(self, url: str, index: int) -> str:
        url_hash = abs(hash(url)) % 100000
        return f"harness_{url_hash}_{index:03d}"
