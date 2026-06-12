# AGENTS.md — Relay: Harness-First Deep Research Agent

> **Relay** is a deep research agent that applies Harness engineering methodology — Continuous Verification, canary delivery, and pipeline-as-code — to knowledge discovery. It doesn't just retrieve; it plans, searches in parallel, verifies claims, and streams progressive findings the user can steer mid-flight.

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Design Philosophy](#2-design-philosophy)
3. [Pipeline Architecture](#3-pipeline-architecture)
4. [Agent Roles](#4-agent-roles)
5. [Corpus Strategy](#5-corpus-strategy)
6. [Verification & Confidence Scoring](#6-verification--confidence-scoring)
7. [Canary Streaming & User Controls](#7-canary-streaming--user-controls)
8. [API Surface](#8-api-surface)
9. [Data Flow Diagram](#9-data-flow-diagram)
10. [Tech Stack](#10-tech-stack)
11. [Directory Structure](#11-directory-structure)
12. [Configuration](#12-configuration)
13. [Extending the Agent](#13-extending-the-agent)
14. [Failure Modes & Guardrails](#14-failure-modes--guardrails)
15. [Roadmap](#15-roadmap)

---

## 1. Project Overview

**Name:** Relay
**Type:** Agentic deep research system
**Primary audience:** Developers, DevOps engineers, platform teams
**Built on:** Harness APIs, Anthropic Claude, web search

### What It Does

Relay takes a research query — "how does Harness handle canary rollback vs Argo Rollouts?" or "what's the best strategy for secret rotation in multi-cloud deployments?" — and runs it through a structured, verifiable pipeline that:

- Decomposes the query into sub-questions
- Searches the Harness corpus and the open web **in parallel**
- Cross-checks claims across sources and scores confidence
- Streams progressive findings so users can redirect before synthesis completes
- Produces a final answer annotated by confidence tier, not just a wall of text

---

## 2. Design Philosophy

Relay is not a RAG wrapper. It is an agent that thinks the way Harness ships software.

**Why we build this way** — Every decision in scaff starts from one question: *does this make the first interaction useful or instructional?* If the answer is "instructional," we redesign.
- First run auto-configures itself — because asking users to configure before they get value is asking them to care before they've seen why.
- Pipeline stages and confidence are visible — because trust comes from seeing how an answer was built, not from a citation list.
- The setup wizard exists — because configuration is a tax, not a feature.
- Reports show confidence first — because a cited answer you can't assess quickly is just a wall of text.

| Harness Concept | How Relay Applies It |
|---|---|
| **Pipeline-as-code** | Research flow is an explicit, inspectable pipeline: `plan → search → verify → synthesize → render` |
| **Continuous Verification** | Claims are scored and cross-checked before surfacing, not just retrieved |
| **Canary / Progressive Delivery** | Findings stream early; user steers with `/deepen` or `/redirect` before full synthesis |
| **Parallel execution** | All sub-queries fire simultaneously — no sequential bottleneck |
| **Corpus-first** | Harness docs and engineering content take precedence for Harness topics; web fills gaps |

The agent's internal architecture mirrors the engineering practices it was built to serve.

---

## 3. Pipeline Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      USER QUERY                         │
└────────────────────────┬────────────────────────────────┘
                         │
                    ┌────▼────┐
                    │  PLAN   │  Decompose query → sub-questions
                    │  AGENT  │  Tag each: harness-corpus | web | both
                    └────┬────┘
                         │
          ┌──────────────▼──────────────┐
          │       BROAD SEARCH          │  Parallel execution
          │  ┌──────────┐ ┌──────────┐  │
          │  │ Harness  │ │   Web    │  │
          │  │  Corpus  │ │ Search   │  │
          │  │  Agent   │ │  Agent   │  │
          │  └──────────┘ └──────────┘  │
          └──────────────┬──────────────┘
                         │ raw results + source metadata
                    ┌────▼────┐
                    │ VERIFY  │  Cross-check claims
                    │  AGENT  │  Score confidence: low / medium / high
                    └────┬────┘
                         │ verified findings
                    ┌────▼──────┐
                    │ SYNTHESIZE│  Merge by confidence tier
                    │   AGENT   │  Structure final answer
                    └────┬──────┘
                         │
                    ┌────▼────┐
                    │ RENDER  │  Stream output
                    │  AGENT  │  Expose /deepen, /redirect controls
                    └─────────┘
```

Each stage is a discrete agent with its own system prompt, tools, and output schema. Stages communicate via structured JSON handoffs — no implicit shared state.

---

## 4. Agent Roles

### 4.1 Plan Agent

**Responsibility:** Query decomposition and routing

**Input:** Raw user query (string)

**Output:**
```json
{
  "sub_questions": [
    {
      "id": "sq_001",
      "question": "How does Harness CV integrate with Prometheus?",
      "source_tag": "harness-corpus",
      "priority": 1
    },
    {
      "id": "sq_002",
      "question": "What are common Prometheus alerting patterns for deployment gates?",
      "source_tag": "web",
      "priority": 2
    }
  ],
  "query_intent": "technical-comparison",
  "domain": "harness-cv"
}
```

**Source tagging rules:**
- `harness-corpus` — questions about Harness products, features, APIs, concepts
- `web` — general DevOps, cloud, tooling questions not specific to Harness
- `both` — questions where Harness has a position AND general context helps

---

### 4.2 Harness Corpus Agent

**Responsibility:** Query the Harness knowledge base (docs, blog, API reference, changelog)

**Tools:**
- `search_harness_docs(query, top_k)` — vector search over Harness documentation
- `search_harness_blog(query, top_k)` — search Harness engineering blog posts
- `fetch_harness_api_ref(endpoint)` — pull specific API reference sections

**Output per sub-question:**
```json
{
  "sq_id": "sq_001",
  "source": "harness-corpus",
  "results": [
    {
      "chunk_id": "docs_cv_prometheus_001",
      "content": "...",
      "url": "https://developer.harness.io/docs/...",
      "relevance_score": 0.91
    }
  ]
}
```

---

### 4.3 Web Search Agent

**Responsibility:** Fill gaps with open web content

**Tools:**
- `web_search(query)` — general web search
- `web_fetch(url)` — fetch full content of a specific page

**Behavior:**
- Deduplicates against Harness corpus results (same claim from two sources → merged, not doubled)
- Flags when web content *contradicts* Harness docs (surfaces as a verification flag)

---

### 4.4 Verify Agent

**Responsibility:** Cross-check claims, score confidence

This is the core differentiator. Most research agents skip this step.

**Input:** All raw results from corpus + web agents, grouped by sub-question

**Process:**
1. Extract atomic claims from each result
2. For each claim, count supporting sources and contradicting sources
3. Score confidence:

| Score | Condition |
|---|---|
| `high` | Claim appears in ≥2 independent sources with no contradiction |
| `medium` | Claim appears in 1 source, or appears in 2+ with minor variation |
| `low` | Claim appears in only 1 source with limited corroboration |
| `contested` | Claim appears in multiple sources with direct contradiction |

4. Flag contested claims with a `conflict_note` explaining the disagreement

**Output:**
```json
{
  "verified_claims": [
    {
      "claim": "Harness CV supports Prometheus as a health source natively",
      "confidence": "high",
      "sources": ["docs_cv_prometheus_001", "blog_cv_deep_dive_003"],
      "conflict_note": null
    },
    {
      "claim": "CV rollback threshold defaults to 50% failure rate",
      "confidence": "medium",
      "sources": ["docs_cv_thresholds_002"],
      "conflict_note": null
    },
    {
      "claim": "CV gates block deployment for up to 30 minutes by default",
      "confidence": "contested",
      "sources": ["docs_cv_gates_001", "community_post_441"],
      "conflict_note": "Harness docs say 30 min; community post reports 15 min after recent update"
    }
  ]
}
```

---

### 4.5 Synthesize Agent

**Responsibility:** Build the final structured answer from verified claims

**Ordering logic:**
1. `high` confidence claims form the core answer
2. `medium` confidence claims are included with a qualifier ("based on limited sources...")
3. `low` confidence claims are surfaced in a "Unverified / needs confirmation" section
4. `contested` claims get their own "Conflicts found" section with both sides

**Output:** Structured markdown with inline confidence annotations

---

### 4.6 Render Agent

**Responsibility:** Progressive streaming + user interaction

**Streaming behavior:**
- Emits findings as soon as Verify agent clears each sub-question (don't wait for all)
- Each streamed chunk is labeled: `[Finding 1/4 — high confidence]`
- After all chunks: emits synthesis block

**User controls (mid-stream):**
- `/deepen <topic>` — spawns a new Plan → Search → Verify mini-pipeline on that subtopic
- `/redirect <new angle>` — discards pending synthesis, re-runs from Plan with new framing
- `/sources` — dumps raw source list with relevance scores

---

## 5. Corpus Strategy

### Harness Corpus Sources

| Source | Type | Update frequency |
|---|---|---|
| `developer.harness.io/docs` | Product documentation | Weekly crawl |
| Harness engineering blog | Long-form technical posts | Weekly crawl |
| Harness API reference | OpenAPI spec + narrative | On release |
| Harness changelog | Release notes | On release |
| Harness community forum | Q&A, community answers | Daily crawl |

### Corpus Priority Rules

```
IF query.domain == "harness" AND harness_corpus.relevance >= 0.75:
    use harness_corpus as primary
    use web as supplementary (fill gaps only)

IF query.domain == "harness" AND harness_corpus.relevance < 0.75:
    use both equally
    flag: "limited official Harness coverage found"

IF query.domain == "general":
    use web as primary
    check harness_corpus for any relevant positioning
```

### Embedding Model

- Harness corpus indexed with `text-embedding-3-small` (OpenAI) or equivalent
- Stored in SQLite with `sqlite-vss` for local-first deployment (mirrors Memora's approach)
- Full-text fallback via FTS5 for keyword-heavy queries

---

## 6. Verification & Confidence Scoring

### Why It Matters

Unverified research agents hallucinate confidently. Relay treats *all* retrieved content as untrusted until cross-checked. This is the same principle Harness Continuous Verification applies to deployments — "trust, but verify with real signal."

### Scoring Algorithm

```python
def score_confidence(claim, sources):
    supporting = [s for s in sources if s.stance == "supports"]
    contradicting = [s for s in sources if s.stance == "contradicts"]
    
    if len(contradicting) > 0:
        return "contested"
    if len(supporting) >= 2:
        return "high"
    if len(supporting) == 1:
        return "medium"
    return "low"
```

### Confidence in the UI

Every claim in the final output is tagged:

```
✅ High confidence  — Harness docs + 2 sources agree
⚠️  Medium confidence — Single source; verify before acting
🔴 Low confidence   — Limited corroboration; treat as hypothesis
⚡ Contested        — Sources disagree; see conflict note
```

---

## 7. Canary Streaming & User Controls

### Canary Principle

Relay doesn't wait for a perfect answer. It delivers findings progressively — like a canary deployment that goes to 10% traffic first, then 50%, then 100%. Users see signal early and can course-correct before the full synthesis runs.

### Stream Protocol

```
[RELAY] Planning query...
[RELAY] Running 4 parallel searches...
[RELAY] Finding 1/4 — high confidence
  Harness CV natively supports Prometheus as a health source via the 
  "Prometheus Health Source" integration in Verify step configuration.
  Sources: developer.harness.io/docs/cv/prometheus, harness-blog-cv-2024

[RELAY] Finding 2/4 — medium confidence
  ...

> /deepen Prometheus threshold configuration

[RELAY] Deepening on: Prometheus threshold configuration
[RELAY] Running targeted search...
...
```

### User Commands

| Command | Behavior |
|---|---|
| `/deepen <topic>` | Spawns a focused mini-pipeline on the given subtopic |
| `/redirect <framing>` | Abandons pending synthesis, replans from scratch |
| `/sources` | Lists all sources with relevance scores and confidence |
| `/confidence` | Shows full confidence breakdown for all claims |
| `/export` | Exports the full session as markdown |

---

## 8. API Surface

### REST Endpoints

```
POST   /api/v1/research          Start a new research session
GET    /api/v1/research/:id      Get session status + findings so far
POST   /api/v1/research/:id/cmd  Send a /deepen or /redirect command
GET    /api/v1/research/:id/export  Export session as markdown
DELETE /api/v1/research/:id      Cancel in-progress session
```

### WebSocket

```
WS /api/v1/research/:id/stream
```

Emits events:
```json
{ "event": "finding", "data": { "index": 1, "total": 4, "confidence": "high", "content": "..." } }
{ "event": "synthesis", "data": { "content": "..." } }
{ "event": "done", "data": { "session_id": "...", "duration_ms": 4200 } }
```

### Research Session Schema

```typescript
interface ResearchSession {
  id: string;
  query: string;
  status: "planning" | "searching" | "verifying" | "synthesizing" | "done" | "error";
  sub_questions: SubQuestion[];
  findings: Finding[];
  synthesis: string | null;
  created_at: string;
  updated_at: string;
}

interface Finding {
  sub_question_id: string;
  content: string;
  confidence: "high" | "medium" | "low" | "contested";
  sources: Source[];
  conflict_note: string | null;
  streamed_at: string;
}
```

---

## 9. Data Flow Diagram

```
User Query
    │
    ▼
Plan Agent
    │ sub_questions[]
    ├─────────────────────────────────┐
    ▼                                 ▼
Harness Corpus Agent            Web Search Agent
(parallel)                      (parallel)
    │                                 │
    └──────────────┬──────────────────┘
                   ▼
            raw_results[]
                   │
                   ▼
            Verify Agent
                   │ verified_claims[] with confidence scores
                   │
                   ├──── stream Finding 1 ──► User (can /deepen or /redirect)
                   ├──── stream Finding 2 ──► User
                   ├──── stream Finding 3 ──► User
                   │
                   ▼
           Synthesize Agent
                   │ structured markdown
                   ▼
            Render Agent
                   │
                   ▼
              Final Output
    (confidence-tiered, source-annotated)
```

---

## 10. Tech Stack

### Core

| Layer | Technology | Rationale |
|---|---|---|
| LLM backbone | Anthropic Claude (claude-sonnet-4) | Tool use, structured output, long context |
| Web search | Brave Search API / Tavily | Clean API, no login walls |
| Corpus storage | SQLite + FTS5 + sqlite-vss | Local-first, zero infra, mirrors Memora approach |
| Embeddings | text-embedding-3-small | Fast, cheap, good quality |
| Backend | Python (FastAPI) | Async, good LLM ecosystem |
| CLI frontend | Rust (clap) | Fast, portable, consistent with scaff |
| Web UI | React + Tailwind | Streaming-friendly, component-based |
| Stream protocol | WebSocket (native FastAPI) | Low-latency progressive delivery |

### Harness Integration

| Integration | Purpose |
|---|---|
| Harness Delegate API | Optional: trigger real pipeline actions from research output |
| Harness Docs crawler | Weekly corpus refresh |
| Harness OpenAPI spec | Structured API reference ingestion |

---

## 11. Directory Structure

```
relay/
├── AGENTS.md                  # This file
├── README.md
├── pyproject.toml
│
├── relay/                     # Python package
│   ├── agents/
│   │   ├── plan.py            # Plan Agent
│   │   ├── corpus.py          # Harness Corpus Agent
│   │   ├── web.py             # Web Search Agent
│   │   ├── verify.py          # Verify Agent
│   │   ├── synthesize.py      # Synthesize Agent
│   │   └── render.py          # Render + streaming Agent
│   │
│   ├── pipeline.py            # Orchestrates agent handoffs
│   ├── corpus/
│   │   ├── crawler.py         # Harness docs crawler
│   │   ├── embedder.py        # Chunking + embedding
│   │   └── store.py           # SQLite + FTS5 + vss
│   │
│   ├── api/
│   │   ├── main.py            # FastAPI app
│   │   ├── routes.py          # REST endpoints
│   │   └── websocket.py       # WS stream handler
│   │
│   └── config.py              # Config loading
│
├── cli/                       # Rust CLI
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands/
│   │   │   ├── research.rs    # relay research "query"
│   │   │   ├── deepen.rs      # relay deepen <topic>
│   │   │   └── export.rs      # relay export <session_id>
│   │   └── client.rs          # HTTP + WS client
│   └── Cargo.toml
│
├── web/                       # React UI (optional)
│   ├── src/
│   │   ├── App.tsx
│   │   ├── components/
│   │   │   ├── ResearchStream.tsx
│   │   │   ├── FindingCard.tsx
│   │   │   ├── ConfidenceBadge.tsx
│   │   │   └── CommandBar.tsx
│   │   └── hooks/
│   │       └── useResearchStream.ts
│   └── package.json
│
├── corpus/                    # Corpus data (gitignored)
│   └── harness.db
│
└── tests/
    ├── test_plan.py
    ├── test_verify.py
    └── test_pipeline.py
```

---

## 12. Configuration

```toml
# relay.toml

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 4096

[search]
web_provider = "tavily"           # or "brave"
harness_corpus_top_k = 8
web_top_k = 5

[corpus]
db_path = "./corpus/harness.db"
refresh_interval_hours = 168      # weekly

[verification]
min_sources_for_high = 2
flag_contradictions = true

[streaming]
chunk_on = "sub_question_complete" # emit finding as each sub-question verifies
enable_user_commands = true

[api]
host = "0.0.0.0"
port = 8000
```

---

## 13. Extending the Agent

### Adding a New Corpus Source

1. Create a crawler in `relay/corpus/crawlers/<source>.py`
2. Implement `crawl() -> List[Document]`
3. Register in `corpus/embedder.py` under `SOURCES`
4. Add source tag in Plan Agent's routing rules

### Adding a New User Command

1. Define command in `relay/agents/render.py` under `COMMANDS`
2. Add handler: `async def handle_<command>(session, args) -> StreamEvent`
3. Register in CLI: `cli/src/commands/<command>.rs`

### Swapping the LLM

All agents use a common `LLMClient` interface in `relay/llm.py`. Swap provider by implementing:
```python
class MyLLMClient(LLMClient):
    async def complete(self, messages, tools=None) -> str: ...
    async def stream(self, messages, tools=None) -> AsyncIterator[str]: ...
```

---

## 14. Failure Modes & Guardrails

| Failure | Detection | Handling |
|---|---|---|
| Corpus returns no results | `relevance_score < 0.4` for all chunks | Fall back to web-only; surface warning |
| Web search rate limited | HTTP 429 from search API | Retry with backoff; continue with corpus-only |
| LLM returns malformed JSON | Parse error in any agent | Re-prompt once with strict schema reminder; fail gracefully |
| Verify finds all claims low-confidence | All scores = `low` | Surface explicit warning: "Limited corroboration found — treat findings as preliminary" |
| User sends `/redirect` mid-synthesis | Command received | Cancel pending synthesis task; re-run from Plan with new framing |
| Session timeout | No activity for 10 minutes | Mark session `expired`; preserve findings so far |

---

## 15. Roadmap

### v0.1 — Core Pipeline (MVP)
- [ ] Plan Agent with sub-question decomposition
- [ ] Web Search Agent (Tavily)
- [ ] Basic Verify Agent (source count scoring)
- [ ] Synthesize Agent
- [ ] CLI interface (`relay research "query"`)

### v0.2 — Corpus Integration
- [ ] Harness docs crawler + embedder
- [ ] SQLite + FTS5 + vss store
- [ ] Corpus Agent with hybrid search
- [ ] Source priority routing

### v0.3 — Canary Streaming
- [ ] WebSocket streaming
- [ ] Progressive finding emission
- [ ] `/deepen` and `/redirect` commands

### v0.4 — Web UI
- [ ] React interface with real-time stream rendering
- [ ] Confidence badge system
- [ ] Source panel

### v0.5 — Harness Integration
- [ ] Harness Delegate API hooks
- [ ] Pipeline trigger from research output
- [ ] Harness AIDA compatibility layer

---

*Relay — research that ships like software.*
