"""Plan Agent - Query decomposition and routing."""
from __future__ import annotations

import json
from dataclasses import dataclass

from ..llm import LLMClient, ChatMessage
from ..types import SubQuestion, SourceTag


@dataclass
class PlanResult:
    sub_questions: list[SubQuestion]
    query_intent: str
    domain: str


class PlanAgent:
    def __init__(self, llm: LLMClient):
        self.llm = llm

    async def plan(
        self,
        question: str,
        max_sub_questions: int = 4,
        model: str | None = None,
    ) -> PlanResult:
        system = ChatMessage.system(
            f"You are a research planner for a deep-research agent. "
            f"Decompose the user's research question into {max_sub_questions} or fewer specific, "
            f"self-contained sub-questions that, when answered individually, would let you "
            f"synthesize a complete answer to the original question. Each sub-question should "
            f"be search-friendly and specific. Also determine the query intent and domain.\n\n"
            f"Return ONLY valid JSON:\n"
            f'{{"sub_questions": [{{"question": "...", "source_tag": "harness-corpus|web|both"}}], '
            f'"query_intent": "...", "domain": "..."}}\n\n'
            f"Source tagging rules:\n"
            f"- harness-corpus: questions about Harness products, features, APIs\n"
            f"- web: general DevOps, cloud, tooling questions not specific to Harness\n"
            f"- both: questions where Harness has a position AND general context helps"
        )

        user = ChatMessage.user(f"Research question: {question}\n\nReturn JSON only.")

        resp = await self.llm.chat(
            [system, user],
            model=model,
            temperature=0.2,
            max_tokens=500,
        )

        parsed = self._extract_json(resp.content)

        sub_questions = []
        for i, sq in enumerate(parsed.get("sub_questions", [])):
            source_tag_str = sq.get("source_tag", "both")
            try:
                source_tag = SourceTag(source_tag_str)
            except ValueError:
                source_tag = SourceTag.BOTH

            sub_questions.append(SubQuestion(
                id=f"sq_{i+1:03d}",
                question=sq.get("question", question),
                source_tag=source_tag,
                priority=i + 1,
            ))

        if not sub_questions:
            sub_questions.append(SubQuestion(
                id="sq_001",
                question=question,
                source_tag=SourceTag.BOTH,
            ))

        return PlanResult(
            sub_questions=sub_questions,
            query_intent=parsed.get("query_intent", "general"),
            domain=parsed.get("domain", "general"),
        )

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
