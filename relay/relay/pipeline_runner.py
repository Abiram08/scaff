"""Pipeline orchestrator - coordinates agent handoffs."""
from __future__ import annotations

import time
from typing import AsyncIterator, Optional

import structlog

from .config import RelayConfig
from .llm import LLMClient
from .rate_limit import RateLimiter
from .token_tracker import TokenTracker
from .memory_store import MemoryStore
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

MAX_SOURCES_BEFORE_VERIFY = 20  # cap sources to limit token usage


class ResearchPipeline:
    def __init__(self, config: RelayConfig):
        self.config = config
        self._limiter = RateLimiter()
        self._token_tracker = TokenTracker()
        self._api_keys: dict[str, str] = {}
        self.memory = MemoryStore(
            db_path=getattr(config.corpus, "memory_db_path", "./memory/relay_memory.db")
        )
        self.llm = LLMClient(config, rate_limiter=self._limiter, token_tracker=self._token_tracker)
        self.corpus = CorpusStore(config.corpus.db_path)
        self.web = WebSearcher(
            provider=config.search.web_provider,
            api_key=config.get_api_key(config.search.web_provider),
            rate_limiter=self._limiter,
        )
        self.plan_agent = PlanAgent(self.llm)
        self.verify_agent = VerifyAgent(self.llm)
        self.synthesize_agent = SynthesizeAgent(self.llm)

    def set_api_key(self, provider: str, api_key: str) -> None:
        """Override an API key for a provider. Takes priority over env vars."""
        if api_key:
            self._api_keys[provider] = api_key

    def get_api_key(self, provider: str) -> str | None:
        """Return the per-request key if set, otherwise fall back to env."""
        if provider in self._api_keys:
            return self._api_keys[provider]
        return self.config.get_api_key(provider)

    async def close(self):
        await self.llm.close()
        await self.web.close()

    async def research(
        self,
        question: str,
        session: Optional[Session] = None,
        api_key: Optional[str] = None,
        provider: Optional[str] = None,
    ) -> AsyncIterator[dict]:
        if session is None:
            session = Session.create(question)

        # Apply per-request API key and provider override
        if api_key:
            effective_provider = provider or self.config.llm.provider
            self.set_api_key(effective_provider, api_key)
            self.llm.set_api_key(api_key)
            if provider:
                self.config.llm.provider = provider
        elif not self.get_api_key(provider or self.config.llm.provider):
            # No key available anywhere
            raise RuntimeError(
                f"No API key found for {provider or self.config.llm.provider}. "
                "Set it in Settings (⚙️) or via environment variable."
            )

        start_time = time.time()

        try:
            async for event in self._run_pipeline(session):
                yield event

            duration_ms = int((time.time() - start_time) * 1000)

            # Persist session + extract memories
            await self._persist_session(session)

            yield {
                "event": "done",
                "data": {
                    "session_id": session.id,
                    "duration_ms": duration_ms,
                    "input_tokens": session.input_tokens,
                    "output_tokens": session.output_tokens,
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

        # Inject memory context: past research relevant to this query
        memory_context = await self.memory.get_relevant_context(session.query)
        if memory_context:
            logger.info("injecting_memory_context", session_id=session.id)

        self.llm.set_context(session.id, stage="plan")
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

        # Cap sources to prevent token explosion in verify/synthesize
        if len(all_sources) > MAX_SOURCES_BEFORE_VERIFY:
            all_sources.sort(key=lambda s: s.score, reverse=True)
            all_sources = all_sources[:MAX_SOURCES_BEFORE_VERIFY]

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

            self.llm.set_context(session.id, stage="verify")
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

        self.llm.set_context(session.id, stage="synthesize")
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
                for i, r in enumerate(results):
                    if not any(s.url == r.url for s in sources):
                        sources.append(Source(
                            id=len(sources),
                            title=r.title,
                            url=r.url,
                            kind=SourceKind.WEB,
                            content=r.snippet,
                            score=0.7 - (len(sources) * 0.02),
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

    async def deepen(
        self,
        session: Session,
        topic: str,
    ) -> AsyncIterator[dict]:
        """Run a focused mini-pipeline on a specific subtopic within an active session."""
        yield {
            "event": "status",
            "data": {"status": "planning", "message": f"Deepening on: {topic}..."},
        }

        # Create a focused sub-question from the deepen topic
        sub_question = topic
        yield {
            "event": "finding",
            "data": {
                "index": len(session.findings) + 1,
                "total": len(session.findings) + 1,
                "confidence": "medium",
                "content": f"**Deepening: {topic}**\n\nRunning targeted search...",
                "conflict_note": None,
            },
        }

        yield {
            "event": "status",
            "data": {"status": "searching", "message": f"Searching for: {topic}..."},
        }

        corpus_sources = await self._search_corpus([sub_question])
        web_sources = await self._search_web(generate_search_queries(session.query, [sub_question], 2))

        all_sources = corpus_sources + web_sources
        self._deduplicate_sources(all_sources)

        if not all_sources:
            yield {
                "event": "finding",
                "data": {
                    "index": len(session.findings) + 1,
                    "total": len(session.findings) + 1,
                    "confidence": "low",
                    "content": f"No new sources found for \"{topic}\". Try a more specific angle or check the web search configuration.",
                    "conflict_note": None,
                },
            }
            return

        yield {
            "event": "status",
            "data": {"status": "verifying", "message": f"Verifying claims about: {topic}..."},
        }

        self.llm.set_context(session.id, stage="deepen-verify")
        verify_result = await self.verify_agent.verify(
            topic,
            [sub_question],
            all_sources,
            model=self.config.llm.model,
        )

        for i, claim in enumerate(verify_result.claims):
            finding = Finding(
                index=len(session.findings) + i + 1,
                total=len(verify_result.claims),
                confidence=claim.confidence,
                content=f"**{topic}** — {claim.text}",
                sources=[s for s in all_sources if s.id in claim.source_ids],
                conflict_note=claim.conflict_note,
            )
            session.add_finding(finding)
            yield {
                "event": "finding",
                "data": {
                    "index": finding.index,
                    "total": finding.total,
                    "confidence": claim.confidence.value,
                    "content": finding.content,
                    "conflict_note": claim.conflict_note,
                },
            }

    async def redirect(
        self,
        session: Session,
        new_query: str,
    ) -> AsyncIterator[dict]:
        """Abandon current session state and redirect to a new query."""
        yield {
            "event": "status",
            "data": {"status": "redirecting", "message": f"Redirecting to: {new_query}..."},
        }

        # Reset session state
        session.query = new_query
        session.sub_questions = []
        session.findings = []
        session.synthesis = None
        session.update_status(SessionStatus.PLANNING)

        # Re-run full pipeline
        async for event in self._run_pipeline(session):
            yield event

    async def _persist_session(self, session: Session) -> None:
        """Save session to memory store and extract memories."""

        # Sync token usage from the global tracker onto the session
        session_usage = self._token_tracker.session_usage(session.id)
        session.input_tokens = session_usage.input_tokens
        session.output_tokens = session_usage.output_tokens

        try:
            await self.memory.save_session(
                session_id=session.id,
                query=session.query,
                status=session.status.value,
                synthesis=session.synthesis,
                input_tokens=session.input_tokens,
                output_tokens=session.output_tokens,
            )
        except Exception as e:
            logger.warning("failed_to_persist_session", error=str(e))

        # Extract key claims as memories for cross-session recall
        for finding in session.findings:
            try:
                source_urls = [s.url for s in finding.sources[:3]]
                await self.memory.save_memory(
                    session_id=session.id,
                    claim=finding.content[:500],
                    confidence=finding.confidence.value,
                    source_urls=source_urls,
                )
            except Exception as e:
                logger.warning("failed_to_save_memory", error=str(e))
