<p align="center">
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange?logo=rust" alt="Rust 1.75+">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License">
  <img src="https://img.shields.io/badge/platform-windows%20%7C%20macos%20%7C%20linux-lightgrey" alt="Platform">
</p>

<h1 align="center">scaff</h1>
<p align="center">
  <i>Harness Research Agent. Cited answers about the Harness platform. One command.</i>
</p>

<p align="center">
  <code>scaff research "What's the difference between Harness CD canary and blue/green?"</code>
</p>

---

## What it is

**scaff** is a verticalized deep-research CLI for the [Harness](https://harness.io) platform. You ask a question, it searches a curated local corpus of Harness docs, blogs, and release notes (with an optional web fallback), and returns a **cited Markdown report**. No code is generated. No agent is scaffolded. The deliverable is knowledge, not artifacts.

```
$ scaff research "How does Harness GitOps relate to Argo CD?"

## TL;DR

Harness GitOps uses Argo CD as the reconciliation engine under the hood. A GitOps
Application defines the source (a Git repo) and the destination (a cluster +
namespace); Harness treats the cluster as the source of truth...

## Findings

Harness GitOps uses Argo CD under the hood [1]. ...

## Sources

[1] **GitOps with Harness** — https://developer.harness.io/docs/continuous-delivery/gitops/... (corpus)
[2] **Kubernetes Integration** — https://developer.harness.io/docs/.../cd-kubernetes-basics (corpus)
```

## Why

- **No more code gen.** scaff doesn't generate a project for you. It *is* the product.
- **Citations on every claim.** Inline `[N]` markers, sources list at the bottom.
- **No account permissions required.** Read-only by design; safe to share in a team.
- **Local-first corpus.** Works offline for everything bundled in the seed.
- **Composable.** Drop it into Claude, Cursor, or any MCP client: `scaff mcp`.

## Install

```bash
# Build from source
git clone <repo-url> scaff
cd scaff
cargo build --release
./target/release/scaff --version
```

## Quick start

```bash
# 1. Configure your provider + API key
scaff config set-key openai sk-proj-...
# or
export OPENAI_API_KEY=sk-proj-...

# 2. Bootstrap the local corpus (downloads ~30 curated entries)
scaff corpus update

# 3. Ask a question (one-shot, no subcommand)
scaff "What is Continuous Verification in Harness CD?"

# 4. Or pipe a longer question from a file
scaff "$(cat question.md)" -o report.md

# 5. Or drop into the interactive REPL
scaff chat
```

## Commands

| Command | What it does |
|---|---|
| `scaff "..."` | Run a research query, print a cited Markdown report. |
| `scaff ask "..."` | Alias for the default form. |
| `scaff research "..."` | Same as `ask`, but explicit. |
| `scaff "..." -o r.md` | Write the report to a file. |
| `scaff "..." --stages` | Show pipeline stages (plan → broad_search → verify → synthesize → render) as they run. |
| `scaff "..." --show-sources` | Show the full source text after the report. |
| `scaff "..." --show-cost` | Print token usage and estimated cost. |
| `scaff "..." -k 12` | Retrieve 12 corpus chunks per sub-question. |
| `scaff chat` | Start the interactive REPL with multi-turn research + slash commands. |
| `scaff history` | List past research executions. |
| `scaff show <id>` | Reprint a saved report (also accepts `:n` index from history). |
| `scaff init` | Write a `.scaff.yaml` project file in the current directory. |
| `scaff connector list` | Show configured data sources (corpus, web, local). |
| `scaff connector test [name]` | Health-check a connector. |
| `scaff corpus status` | Show corpus stats. |
| `scaff corpus update` | Download the latest seed from the remote source. |
| `scaff corpus add file.jsonl` | Add chunks from a local JSONL file. |
| `scaff corpus add-dir ./docs` | Ingest a local directory of markdown/text into the corpus. |
| `scaff corpus crawl <url>` | Fetch and chunk a single URL. |
| `scaff corpus search "..."` | Run a raw BM25 search and print hits. |
| `scaff corpus clear` | Wipe the local corpus. |
| `scaff config show` | Show current configuration. |
| `scaff config set <key> <value>` | Set a config value (model, default_provider, retrieval_k, ...). |
| `scaff config set-key <provider> <key>` | Store an API key for a provider. |
| `scaff doctor` | Health check: API keys, corpus, connectors, dependencies. |
| `scaff mcp` | Start an MCP stdio server exposing the `research` tool. |
| `scaff completions <shell>` | Generate shell completions. |

## REPL (`scaff chat`)

`scaff chat` (or just `scaff` with no arguments) drops you into a multi-turn research
session. Every query runs the full pipeline (`plan → broad_search → verify → synthesize → render`)
and is auto-saved as an execution record plus a cited Markdown report under `~/.scaff/reports/`.

Slash commands:

| Command | Effect |
|---|---|
| `/help` | Show the slash-command help. |
| `/stages` | Toggle pipeline-stage display. |
| `/sources-toggle` | Toggle full source-text display after each report. |
| `/sources` | List all sources with relevance scores and confidence. |
| `/web` | Toggle web-search fallback for the rest of the session. |
| `/model <name>` | Switch model for the rest of the session. |
| `/provider <name>` | Switch provider (openai, anthropic, gemini, groq, ollama). |
| `/deepen <topic>` | Spawn a focused mini-pipeline on the given subtopic. |
| `/redirect <framing>` | Abandon pending synthesis, re-run from Plan with new framing. |
| `/confidence` | Show full confidence breakdown for all claims. |
| `/history` | Show this session's past questions. |
| `/show <id\|:n>` | Reprint a saved report. |
| `/expand <n>` | Print the full text of source `[n]` from the last answer. |
| `/save` | Re-save the last report. |
| `/last` | Re-run the last question. |
| `/export` | Export the full session as markdown. |
| `/clear` | Clear the screen. |
| `/quit` | Exit the REPL. |

## Harness concept mapping

scaff is organized around the same primitives as the Harness platform:

| Harness concept | What it is in scaff |
|---|---|
| Pipeline | The research pipeline: `plan → broad_search → verify → synthesize → render`. Run with `--stages` to watch it. |
| Stage | One step of the pipeline. Each stage records its name, status, duration, and details. |
| Connector | A data source. Built-in: `corpus`, `web`, `local://path`. `scaff connector list` to inspect. |
| Trigger | A way to start a run. CLI (`scaff "..."`), REPL (`scaff chat`), or MCP (`scaff mcp`). |
| Service | The LLM provider. OpenAI, Anthropic, Gemini, Groq, Ollama. |
| Execution | A single research run. Persisted to `~/.scaff/executions/<id>.json` and listed by `scaff history`. |
| Artifact | The cited Markdown report. Persisted to `~/.scaff/reports/<id>-<slug>.md` with YAML frontmatter. |
| Project | A directory with a `.scaff.yaml` (created by `scaff init`). |
| Continuous Verification | Claims are cross-checked across sources and scored for confidence (high/medium/low/contested) before synthesis. |
| Canary / Progressive | Early findings can be streamed; user can `/deepen` or `/redirect` in REPL. |
| Confidence Tiers | ✅ High (2+ sources agree), ⚠️ Medium (1 source), 🔴 Low (limited corroboration), ⚡ Contested (sources disagree). |

## Providers

| Provider   | Default model                  | Env var               |
|------------|--------------------------------|-----------------------|
| `openai`   | `gpt-4o-mini`                  | `OPENAI_API_KEY`      |
| `anthropic`| `claude-3-5-haiku-latest`      | `ANTHROPIC_API_KEY`   |
| `gemini`   | `gemini-1.5-flash`             | `GEMINI_API_KEY`      |
| `groq`     | `llama-3.1-8b-instant`         | `GROQ_API_KEY`        |
| `ollama`   | `llama3.1`                     | _(no key required)_   |

Switch providers for one command:

```bash
scaff research "..." --provider anthropic
scaff research "..." --provider ollama --cheap
```

## MCP server

`scaff` can run as an MCP stdio server, exposing a `research` tool to Claude, Cursor, or any MCP-compatible client:

```bash
scaff mcp
```

Add to your MCP client config (e.g., `~/.config/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "scaff": {
      "command": "/absolute/path/to/scaff",
      "args": ["mcp"]
    }
  }
}
```

The `research` tool accepts `{ "question": "..." }` and returns the cited Markdown report as text content.

## How it works

```
scaff "..." (Rust CLI, single binary)
    |
    +-- 1. plan:   LLM decomposes the question into 3-5 sub-questions
    |
    +-- 2. broad_search:  Parallel multi-query search
    |    - Corpus BM25 search for each sub-question
    |    - Web search via DuckDuckGo (multiple queries)
    |    - Local files via local:// connector
    |
    +-- 3. verify:  Cross-source fact-check + confidence scoring
    |    - Extract claims from sources
    |    - Score confidence (HIGH/MEDIUM/LOW)
    |    - Flag unsupported claims
    |
    +-- 4. synthesize:  LLM produces a Markdown report with [N] citations
    |    - Uses verified claims to guide confidence
    |    - TL;DR + Confidence Summary + Findings + Known Unknowns
    |
    +-- 5. render + persist:  TL;DR + Findings + Sources
    |    - ~/.scaff/executions/<id>.json
    |    - ~/.scaff/reports/<id>-<slug>.md
```

Every run is a Harness-style **execution** with named **stages** (`plan`, `broad_search`, `verify`, `synthesize`, `render`), timing, sources, model, and estimated cost. Pass
`--stages` to watch the pipeline run live. Run `scaff history` to see past
executions, `scaff show <id>` to reprint a report.

No subprocess, no IPC, no embedded Python. The LLM client speaks OpenAI-compatible chat completions for 4 of the 5 providers, and the Anthropic Messages API for Anthropic.

## Corpus

The corpus is the moat. It's stored as a SQLite database at `~/.scaff/corpus.db`, with BM25 indexing done in memory on startup. A seed is shipped in the repo (`corpus/seed.jsonl`) and downloaded on first run.

To extend:

```bash
# Add more chunks from a local JSONL file
scaff corpus add my-team-notes.jsonl

# Crawl a public docs page
scaff corpus crawl https://developer.harness.io/docs/some-page

# Refresh from the remote seed
scaff corpus update
```

See [`corpus/README.md`](corpus/README.md) for the chunk format.

## Project structure

```
scaff/
├── src/
│   ├── main.rs         # Entry point
│   ├── lib.rs          # Library facade (re-exports modules)
│   ├── cli.rs          # Clap commands + REPL launch
│   ├── config.rs       # Config + provider registry
│   ├── llm.rs          # Multi-provider chat client
│   ├── search.rs       # BM25 indexing + retrieval
│   ├── corpus.rs       # Seed load, crawl, SQLite persistence
│   ├── web.rs          # DuckDuckGo web search
│   ├── research.rs     # Plan → retrieve → synthesize loop
│   ├── render.rs       # Markdown report rendering with citations
│   ├── pipeline.rs     # Execution / Stage / Source / timing primitives
│   ├── history.rs      # Persist executions + reports to ~/.scaff/
│   ├── connectors.rs   # Connector registry + health checks
│   ├── local_files.rs  # Local directory ingest (markdown/text)
│   ├── init.rs         # First-run wizard + `scaff init`
│   ├── repl.rs         # Interactive REPL with slash commands
│   ├── mcp.rs          # MCP stdio server
│   └── display.rs      # Console helpers
├── corpus/
│   ├── seed.jsonl      # ~35 curated Harness entries
│   └── README.md       # Corpus format + refresh
├── examples/
│   └── questions.txt   # Example research questions
├── tests/
│   └── integration.rs  # End-to-end tests
├── Cargo.toml
└── README.md
```

## Development

```bash
cargo build --release
cargo test
cargo run -- doctor
cargo run -- "What is Harness CD?"
cargo run -- chat   # REPL
```

On Windows, the MinGW `dlltool` shipped with `rustup`'s `pc-windows-gnu` target
breaks on paths with spaces. Workaround:

```bash
export CARGO_TARGET_DIR=C:/scaff-target
cargo build --release
```

## License

MIT — see [LICENSE](LICENSE).
