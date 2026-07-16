<p align="center">
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange?logo=rust" alt="Rust 1.75+">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License">
  <img src="https://img.shields.io/badge/agent-pi--simple-blue" alt="Pi-simple agent">
</p>

<h1 align="center">scaff</h1>
<p align="center">
  <i>Pi-simple Harness research agent. Four tools. One loop. Cited answers.</i>
</p>

<p align="center">
  <code>scaff "What's the difference between Harness CD canary and blue/green?"</code>
</p>

---

## What it is

**scaff** is a production Rust CLI that answers questions about the [Harness](https://harness.io) platform with **citations and confidence**.

Architecture is intentionally small (like [Pi](https://pi.dev/)):

- **One agent loop** — LLM calls tools until `finish`
- **Four tools only** — `search_corpus` · `web_search` · `fetch_url` · `finish`
- **Short system prompt** — no multi-agent graph
- **Harness discipline** — corpus-first, visible steps, confidence on every finding

The deliverable is a **Markdown report**, not generated code.

```
$ scaff "How does Harness GitOps relate to Argo CD?"

## TL;DR

Harness GitOps uses Argo CD as the reconciliation engine under the hood...

## Confidence Summary

✅ **2** high · ⚠️ **1** medium · 🔴 **0** low · ⚡ **0** contested

## Findings

1. **[✅ high]** ...
   - sources: corpus:...

## Sources

- **[corpus:...]** title — url
```

## Use cases (v1)

| Command | Intent |
|---------|--------|
| `scaff "..."` / `scaff ask "..."` | Product Q&A with citations |
| `scaff compare "..."` | A vs B, Harness vs X, strategy trade-offs |
| `scaff howto "..."` | Steps, setup, troubleshooting |

## Install

```bash
git clone <repo-url> scaff
cd scaff
cargo build --release
./target/release/scaff --version
```

## Quick start

```bash
# 1. API key (auto-detected from env, or)
export OPENAI_API_KEY=sk-...
# or: scaff setup

# 2. Seed local Harness corpus (first run often does this for you)
scaff corpus update

# 3. Ask
scaff "What is Continuous Verification in Harness CD?"

# 4. Compare / how-to
scaff compare "Harness canary vs Argo Rollouts"
scaff howto "configure a CV gate for canary"

# 5. See the agent work
scaff "..." --stages --show-cost

# 6. Interactive
scaff chat
```

## How the agent works

Built like **Pi** (tiny loop, few tools, short prompt) and [Anthropic’s agent guidance](https://www.anthropic.com/engineering/building-effective-agents): *models using tools in a loop*.

```
question
   → LLM (native tool calling: OpenAI tools / Anthropic tools)
   → search_corpus  (local BM25 / SQLite)
   → web_search     (optional)
   → fetch_url      (optional detail)
   → finish         (structured report)
```

Production bounds:

- Max **8** tool steps (+ forced finish)
- Native tool calling (JSON-in-text fallback for weak models)
- LLM retries with backoff on 429 / 5xx
- Tool results truncated (~6k chars) for context control
- Clear errors → `scaff doctor` / `scaff setup`

See **`AGENTS.md`** for the architecture of record.

## Production readiness

| Ready | Not magic |
|-------|-----------|
| Single binary CLI for anyone with an API key | Answer quality depends on model + corpus coverage |
| Offline-capable corpus search | Web search is best-effort (DDG HTML) |
| Retries, timeouts, max steps | No multi-tenant SaaS auth/billing built-in |
| Stable report schema | Live LLM tests need your key in CI |

**Deploy for a team:** ship the release binary (or `cargo install --path .`), set `OPENAI_API_KEY` / `ANTHROPIC_API_KEY`, run `scaff doctor`, then `scaff "…"`.

## Commands

| Command | What it does |
|---|---|
| `scaff "..."` | Ask (default use case) |
| `scaff ask "..."` | Same as default |
| `scaff compare "..."` | Comparison-oriented prompt |
| `scaff howto "..."` | How-to / troubleshooting prompt |
| `scaff "..." --stages` | Print each tool step |
| `scaff "..." --show-cost` | Token usage + estimated cost |
| `scaff "..." --no-web` | Corpus + model only |
| `scaff "..." -o report.md` | Write report to file |
| `scaff chat` | Interactive REPL |
| `scaff history` | Past executions |
| `scaff show <id>` | Reprint a saved report |
| `scaff doctor` | Health check |
| `scaff setup` | Setup wizard |
| `scaff corpus status\|update\|search …` | Corpus ops |
| `scaff config …` | Config / API keys |
| `scaff mcp` | MCP stdio server (`research` tool) |

## Configuration

Providers: OpenAI, Anthropic, Gemini, Groq, Ollama.

```bash
export OPENAI_API_KEY=...
# or
scaff config set-key openai sk-...
scaff config set model gpt-4o-mini
```

Config lives under `~/.scaff/`. Local corpus DB is gitignored user data.

## Project layout (Rust product)

```
src/
  agent/          # loop, prompt, tools, types  ← product core
  report.rs       # finish → Markdown
  llm.rs          # multi-provider chat
  corpus.rs       # SQLite corpus
  search.rs       # BM25
  web.rs          # search + fetch
  cli.rs          # ask | compare | howto | …
```

## Develop

```bash
cargo test
cargo build --release
make doctor
```

## License

MIT
