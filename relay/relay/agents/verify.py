"""Verify Agent - Cross-check claims and score confidence."""
from __future__ import annotations

import json
from dataclasses import dataclass

from ..llm import LLMClient, ChatMessage
from ..types import Claim, Confidence, VerificationResult, Stance, Source


@dataclass
class VerifyResult:
    claims: list[Claim]
    verifications: list[VerificationResult]


class VerifyAgent:
    def __init__(self, llm: LLMClient):
        self.llm = llm

    async def verify(
        self,
        question: str,
        sub_questions: list[str],
        sources: list[Source],
        model: str | None = None,
    ) -> VerifyResult:
        sources_text = self._format_sources(sources)

        system = ChatMessage.system(
            "You are a claim extractor and verifier for a deep-research agent. "
            "Given a research question and sources, extract the key factual claims and "
            "identify which sources support each claim. Detect contradictions between sources.\n\n"
            "Return ONLY valid JSON:\n"
            "{\n"
            '  "claims": [\n'
            "    {\n"
            '      "text": "claim text",\n'
            '      "source_ids": [0, 1],\n'
            '      "confidence": 0.9,\n'
            '      "conflict_note": null or "description of conflict"\n'
            "    }\n"
            "  ]\n"
            "}\n\n"
            "Confidence rules:\n"
            "- 0.9-1.0: Explicitly stated in 2+ independent sources with no contradiction\n"
            "- 0.7-0.89: Stated in 1 source or implied by multiple sources\n"
            "- 0.5-0.69: Partially supported, some ambiguity\n"
            "- Below 0.5: Speculative or contradicted\n\n"
            "If sources contradict each other, set confidence below 0.5 and add a conflict_note."
        )

        sub_list = "\n".join(f"- {sq}" for sq in sub_questions)
        user = ChatMessage.user(
            f"Question: {question}\n\n"
            f"Sub-questions:\n{sub_list}\n\n"
            f"Sources:\n{sources_text}\n\n"
            f"Extract claims with confidence scores. Return JSON only."
        )

        resp = await self.llm.chat(
            [system, user],
            model=model,
            temperature=0.1,
            max_tokens=1500,
        )

        parsed = self._extract_json(resp.content)
        claims = self._parse_claims(parsed)

        verifications = self._create_verifications(claims)

        return VerifyResult(claims=claims, verifications=verifications)

    def _format_sources(self, sources: list[Source]) -> str:
        parts = []
        for src in sources:
            parts.append(
                f"[{src.id}] {src.title}\n"
                f"URL: {src.url}\n"
                f"Content: {src.content[:1500] if src.content else '(no content)'}\n"
            )
        return "\n".join(parts)

    def _parse_claims(self, parsed: dict) -> list[Claim]:
        claims = []
        for i, cj in enumerate(parsed.get("claims", [])):
            confidence_score = cj.get("confidence", 0.5)
            conflict_note = cj.get("conflict_note")
            has_contradiction = conflict_note is not None or confidence_score < 0.5

            claims.append(Claim(
                id=i,
                text=cj.get("text", ""),
                source_ids=cj.get("source_ids", []),
                confidence=Confidence.from_score(confidence_score, has_contradiction),
                conflict_note=conflict_note,
            ))
        return claims

    def _create_verifications(self, claims: list[Claim]) -> list[VerificationResult]:
        verifications = []
        for claim in claims:
            for sid in claim.source_ids:
                supported = claim.confidence not in (Confidence.LOW,)
                stance = Stance.SUPPORTS if supported else (
                    Stance.CONTRADICTS if claim.conflict_note else Stance.NEUTRAL
                )
                verifications.append(VerificationResult(
                    claim_id=claim.id,
                    source_id=sid,
                    supported=supported,
                    evidence="Source supports this claim" if supported else "Source does not clearly support this claim",
                    stance=stance,
                ))
        return verifications

    def _extract_json(self, text: str) -> dict:
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            pass

        if "```" in text:
            start = text.find("```")
            rest = text[start + 3:]
            end = rest.find("```")
            if end != -1:
                block = rest[:end].strip()
                if block.startswith("json"):
                    block = block[4:].strip()
                try:
                    return json.loads(block)
                except json.JSONDecodeError:
                    pass

        start = text.find("{")
        end = text.rfind("}")
        if start != -1 and end != -1:
            try:
                return json.loads(text[start:end + 1])
            except json.JSONDecodeError:
                pass

        return {}
