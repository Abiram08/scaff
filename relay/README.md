# Relay - Harness-First Deep Research Agent

Relay is a deep research agent that applies Harness engineering methodology — Continuous Verification, canary delivery, and pipeline-as-code — to knowledge discovery.

## Quick Start

### Python Backend

```bash
cd relay
pip install -e .
export ANTHROPIC_API_KEY=sk-...
relay
```

### React Web UI

```bash
cd web
npm install
npm run dev
```

### Rust CLI (existing)

```bash
cargo build --release
./target/release/scaff "What is Harness CD?"
```

## Architecture

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
                    │  AGENT  │  Score confidence: low / medium / high / contested
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

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/research` | Start a new research session |
| GET | `/api/v1/research/:id` | Get session status + findings |
| DELETE | `/api/v1/research/:id` | Cancel in-progress session |
| GET | `/api/v1/research/:id/export` | Export session as markdown |
| WS | `/api/v1/research/:id/stream` | WebSocket streaming |

## WebSocket Events

```json
{ "event": "status", "data": { "status": "planning", "message": "..." } }
{ "event": "plan", "data": { "sub_questions": [...], "domain": "..." } }
{ "event": "search", "data": { "corpus_results": 5, "web_results": 3 } }
{ "event": "finding", "data": { "index": 1, "confidence": "high", "content": "..." } }
{ "event": "synthesis", "data": { "tldr": "...", "body": "...", "report": "..." } }
{ "event": "done", "data": { "session_id": "...", "duration_ms": 4200 } }
```

## Configuration

Set via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `RELAY_LLM_PROVIDER` | `anthropic` | LLM provider |
| `RELAY_LLM_MODEL` | `claude-sonnet-4-20250514` | Model name |
| `RELAY_SEARCH_PROVIDER` | `tavily` | Web search provider |
| `RELAY_CORPUS_DB` | `./corpus/harness.db` | Corpus database path |
| `OPENAI_API_KEY` | - | OpenAI API key |
| `ANTHROPIC_API_KEY` | - | Anthropic API key |
| `TAVILY_API_KEY` | - | Tavily API key |
| `BRAVE_API_KEY` | - | Brave Search API key |

## Tech Stack

- **Backend**: Python 3.11+, FastAPI, WebSockets
- **Frontend**: React 18, TypeScript, Vite, Tailwind CSS
- **CLI**: Rust (existing scaff binary)
- **Corpus**: SQLite + FTS5
- **Search**: DuckDuckGo, Tavily, Brave
- **LLM**: Anthropic Claude, OpenAI, Gemini, Groq, Ollama
