"""Synthesize Agent - Build final structured answer from verified claims."""
from __future__ import annotations

from ..llm import LLMClient, ChatMessage
from ..types import Claim, Source


class SynthesizeAgent:
    def __init__(self, llm: LLMClient):
        self.llm = llm

    async def synthesize(
        self,
        question: str,
        sub_questions: list[str],
        sources: list[Source],
        claims: list[Claim],
        model: str | None = None,
    ) -> tuple[str, str]:
        sources_text = self._format_sources(sources)
        claims_text = self._format_claims(claims)
        sub_list = "\n".join(f"- {sq}" for sq in sub_questions)

        system = ChatMessage.system(
            "You are a research synthesizer for a deep-research agent. You will be given a "
            "research question, a list of sub-questions, and a set of numbered sources. You will "
            "also receive verified claims with confidence scores from the verification stage.\n\n"
            "Rules:\n"
            "- Every non-trivial claim must have an inline [N] citation that matches a source.\n"
            "- If a source does not actually support a claim, do not cite it.\n"
            "- Use the verified claims to guide your confidence: HIGH claims can be stated firmly, "
            "MEDIUM claims should note the evidence, LOW claims should be hedged or omitted.\n"
            "- Contested claims should be presented with both sides.\n"
            "- If sources do not cover part of the question, say so in 'Known Unknowns'.\n"
            "- Keep the report focused and well-organized.\n"
            "- Use ## headings only.\n\n"
            "Output exactly:\n"
            "## TL;DR\n<1-3 sentences>\n\n"
            "## Confidence Summary\n<bullet list of key findings with confidence levels>\n\n"
            "## Findings\n<body with [N] inline citations>\n\n"
            "## Known Unknowns\n<bulleted list, or 'None'>"
        )

        user = ChatMessage.user(
            f"Question: {question}\n\n"
            f"Sub-questions to address:\n{sub_list}\n\n"
            f"Sources:\n{sources_text}\n\n"
            f"Verified Claims:\n{claims_text}\n\n"
            f"Produce the Markdown report now."
        )

        resp = await self.llm.chat(
            [system, user],
            model=model,
            temperature=0.3,
            max_tokens=4096,
        )

        tldr, body = self._parse_report(resp.content)
        return tldr, body

    def _format_sources(self, sources: list[Source]) -> str:
        parts = []
        for src in sources:
            parts.append(
                f"[{src.id}] {src.title}\n"
                f"URL: {src.url}\n"
                f"Content: {src.content[:1500] if src.content else '(no content)'}\n"
            )
        return "\n".join(parts)

    def _format_claims(self, claims: list[Claim]) -> str:
        parts = []
        for claim in claims:
            status = {
                "high": "HIGH CONFIDENCE",
                "medium": "MEDIUM CONFIDENCE",
                "low": "LOW CONFIDENCE",
                "contested": "CONTESTED",
            }.get(claim.confidence.value, "UNKNOWN")

            note = f" — {claim.conflict_note}" if claim.conflict_note else ""
            parts.append(
                f"- [{claim.id}] {claim.text} "
                f"(confidence: {claim.confidence.value}, sources: {len(claim.source_ids)}, "
                f"status: {status}{note})"
            )
        return "\n".join(parts)

    def _parse_report(self, text: str) -> tuple[str, str]:
        tldr = ""
        body = ""
        known = ""

        section = "none"
        for line in text.split("\n"):
            trimmed = line.rstrip()
            lower = trimmed.lower()

            if lower.startswith("## tldr") or lower.startswith("# tldr"):
                section = "tldr"
                continue
            if lower.startswith("## findings") or lower.startswith("# findings"):
                section = "body"
                continue
            if lower.startswith("## known unknowns") or lower.startswith("# known unknowns"):
                section = "known"
                continue
            if lower.startswith("## confidence") or lower.startswith("# confidence"):
                section = "body"
                continue
            if lower.startswith("# ") or lower.startswith("## "):
                section = "body"

            if section == "tldr":
                if trimmed.strip():
                    tldr += (" " if tldr else "") + trimmed.strip()
            elif section == "known":
                known += trimmed + "\n"
            else:
                body += trimmed + "\n"

        if known.strip() and not known.strip().lower().eq("none"):
            body += "\n\n## Known Unknowns\n\n" + known.strip()

        return tldr.strip(), body.strip()
