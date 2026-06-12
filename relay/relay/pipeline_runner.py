"""Pipeline orchestrator - coordinates agent handoffs."""
from __future__ import annotations

import time
from typing import AsyncIterator, Optional

import structlog

from .config import RelayConfig
from .llm import LLMClient
from .types import (
    Session, SessionStatus, Finding, Source, SourceKind,
    Stage, StageStatus, Claim, Confidence,
)
from .agents.plan import PlanAgent
from .agents.verify import VerifyAgent
from .agents.synthesize import SynthesizeAgent
from .agents.render import render_report
from .corpus.store import CorpusStore
from .web import WebSearcher, generate_search_queries

logger = structlog.get_logger()


class ResearchPipeline:
    def __init__(self, config: RelayConfig):
        self.config = config
        self.llm = LLMClient(config)
        self.corpus = CorpusStore(config.corpus.db_path)
        self.web = WebSearcher(
            provider=config.search.web_provider,
            api_key=config.get_api_key(config.search.web_provider),
        )
        self.plan_agent = PlanAgent(self.llm)
        self.verify_agent = VerifyAgent(self.llm)
        self.synthesize_agent = SynthesizeAgent(self.llm)

    async def close(self):
        await self.llm.close()
        await self.web.close()

    async def research(
        self,
        question: str,
        session: Optional[Session] = None,
    ) -> AsyncIterator[dict]:
        if session is None:
            session = Session.create(question)

        start_time = time.time()

        try:
            async for event in self._run_pipeline(session):
                yield event

            duration_ms = int((time.time() - start_time) * 1000)
            yield {
                "event": "done",
                "data": {
                    "session_id": session.id,
                    "duration_ms": duration_ms,
                },
            }

        except Exception as e:
            session.update_status(SessionStatus.ERROR)
            yield {
                "event": "error",
                "data": {
                    "session_id": session.id,
                    "error": str(e),
                },
            }

    async def _run_pipeline(self, session: Session) -> AsyncIterator[dict]:
        yield {"event": "status", "data": {"status": "planning", "message": "Planning query..."}}
        session.update_status(SessionStatus.PLANNING)

        plan_result = await self.plan_agent.plan(
            session.query,
            max_sub_questions=self.config.search.max_search_queries,
            model=self.config.llm.model,
        )
        session.sub_questions = plan_result.sub_questions

        yield {
            "event": "plan",
            "data": {
                "sub_questions": [sq.question for sq in session.sub_questions],
                "domain": plan_result.domain,
            },
        }

        yield {"event": "status", "data": {"status": "searching", "message": "Running parallel searches..."}}
        session.update_status(SessionStatus.SEARCHING)

        sub_questions = [sq.question for sq in session.sub_questions]
        search_queries = generate_search_queries(
            session.query, sub_questions, self.config.search.max_search_queries
        )

        corpus_sources = await self._search_corpus(sub_questions)
        web_sources = await self._search_web(search_queries)

        all_sources = corpus_sources + web_sources
        self._deduplicate_sources(all_sources)

        yield {
            "event": "search",
            "data": {
                "corpus_results": len(corpus_sources),
                "web_results": len(web_sources),
                "total_sources": len(all_sources),
            },
        }

        claims = []
        verifications = []
        if all_sources:
            yield {"event": "status", "data": {"status": "verifying", "message": "Verifying claims..."}}
            session.update_status(SessionStatus.VERIFYING)

            verify_result = await self.verify_agent.verify(
                session.query,
                sub_questions,
                all_sources,
                model=self.config.llm.model,
            )
            claims = verify_result.claims
            verifications = verify_result.verifications

            for i, claim in enumerate(claims):
                finding = Finding(
                    index=i + 1,
                    total=len(claims),
                    confidence=claim.confidence,
                    content=claim.text,
                    sources=[s for s in all_sources if s.id in claim.source_ids],
                    conflict_note=claim.conflict_note,
                )
                session.add_finding(finding)

                yield {
                    "event": "finding",
                    "data": {
                        "index": i + 1,
                        "total": len(claims),
                        "confidence": claim.confidence.value,
                        "content": claim.text,
                        "conflict_note": claim.conflict_note,
                    },
                }

        yield {"event": "status", "data": {"status": "synthesizing", "message": "Synthesizing report..."}}
        session.update_status(SessionStatus.SYNTHESIZING)

        tldr, body = await self.synthesize_agent.synthesize(
            session.query,
            sub_questions,
            all_sources,
            claims,
            model=self.config.llm.model,
        )

        report = render_report(session.query, tldr, body, corpus_sources, web_sources)

        session.synthesis = report
        session.update_status(SessionStatus.DONE)

        yield {
            "event": "synthesis",
            "data": {
                "tldr": tldr,
                "body": body,
                "report": report,
            },
        }

    async def _search_corpus(self, sub_questions: list[str]) -> list[Source]:
        sources = []
        try:
            await self.corpus.ensure_seeded()
            for sq in sub_questions:
                chunks = await self.corpus.search_fts(sq, limit=self.config.search.harness_corpus_top_k)
                for i, chunk in enumerate(chunks):
                    sources.append(Source(
                        id=len(sources),
                        title=chunk.title,
                        url=chunk.url,
                        kind=SourceKind.CORPUS,
                        score=0.8 - (i * 0.05),
                        content=chunk.content,
                    ))
        except Exception as e:
            logger.warning("corpus_search_failed", error=str(e))
        return sources

    async def _search_web(self, queries: list[str]) -> list[Source]:
        sources = []
        try:
            for query in queries:
                results = await self.web.search(query, max_results=3)
                for r in results:
                    if not any(s.url == r.url for s in sources):
                        sources.append(Source(
                            id=len(sources),
                            title=r.title,
                            url=r.url,
                            kind=SourceKind.WEB,
                            content=r.snippet,
                        ))
        except Exception as e:
            logger.warning("web_search_failed", error=str(e))
        return sources

    def _deduplicate_sources(self, sources: list[Source]):
        seen_urls = set()
        unique = []
        for src in sources:
            if src.url not in seen_urls:
                seen_urls.add(src.url)
                unique.append(src)
        sources.clear()
        sources.extend(unique)
