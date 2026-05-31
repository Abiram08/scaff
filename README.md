<p align="center">
  <img src="https://img.shields.io/badge/python-3.11%2B-blue?logo=python" alt="Python 3.11+">
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange?logo=rust" alt="Rust 1.75+">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License">
  <img src="https://img.shields.io/badge/platform-windows%20%7C%20macos%20%7C%20linux-lightgrey" alt="Platform">
</p>

<h1 align="center">scaff</h1>
<p align="center">
  <i>From plain English to production-ready AI agents. One command.</i>
</p>

<p align="center">
  <code>scaff run "summarize my emails and flag urgent ones"</code>
</p>

---

## Overview

**scaff** is a hybrid Rust+Python CLI that transforms a plain English description into a complete, self-contained AI agent. It generates idiomatic Python code with tool-calling loops, error handling, dependency management, and optional web UIs — all without writing a single line of code.

```
┌────────────────────────────────────────────────────────────────┐
│                    scaff Architecture                           │
│                                                                │
│  ┌──────────┐     JSON/stdin      ┌─────────────────────────┐  │
│  │ Rust CLI │ ──────────────────► │  Python Generation       │  │
│  │ (clap)   │ ◄────────────────── │  Engine                  │  │
│  │          │     JSON/stdout     │  (OpenAI / Anthropic /   │  │
│  │ • 22 commands                  │   Gemini / Ollama)       │  │
│  │ • Shell completions            │                          │  │
│  │ • TUI monitor (ratatui)        │  • Token bucket limiter  │  │
│  │ • Token tracker                │  • Session tracking      │  │
│  │ • Response cache               │  • Cost estimation       │  │
│  │ • Config management            │  • Retry logic           │  │
│  └────────────────────────────────┘  └─────────────────────────┘  │
└────────────────────────────────────────────────────────────────┘
```

---

## Features

- **Multi-provider**: OpenAI (GPT-4o/mini), Anthropic (Claude), Google (Gemini), Ollama (local)
- **Interactive REPL**: `scaff shell` — iteratively build agents conversationally
- **TUI Dashboard**: `scaff monitor` — real-time usage, budget, and rate limit visualization
- **Budget Controls**: MIN/MEDIUM/MAX modes with hard monthly caps, burn rate alerts, and trend projections
- **Response Cache**: Automatic deduplication — same description never costs you twice
- **Export**: `scaff export` — CSV/JSON token usage data for external analysis
- **Shell Completions**: `scaff completions bash|zsh|fish|powershell|elvish`
- **Generated agents are self-contained**: `python agent.py` — no heavy frameworks

---

## Quick Start

### Option A: Install the Rust CLI (recommended)

Download the pre-built binary from [Releases](https://github.com/your-org/scaff/releases) or build from source:

```bash
git clone https://github.com/your-org/scaff.git
cd scaff
cargo build --release
# Binary: ./target/release/scaff
```

### Option B: Install with Python

```bash
pip install -e .
```

### Set your API key

```bash
export OPENAI_API_KEY="sk-..."
# Windows: $env:OPENAI_API_KEY = "sk-..."
```

### Create your first agent

```bash
scaff run "check the weather and tell me if I need an umbrella"
```

---

## Command Reference

### Core Commands

| Command | Description |
|---------|-------------|
| `scaff run "..."` | Generate, install dependencies, and run — all in one |
| `scaff create "..."` | Generate agent files for later execution |
| `scaff wizard` | Interactive guided setup for beginners |
| `scaff validate "..."` | Dry-run: test a description without generating files |
| `scaff shell` | Interactive REPL for iteratively building agents |

### Management

| Command | Description |
|---------|-------------|
| `scaff config [show\|set\|reset]` | View or modify settings |
| `scaff mode [min\|medium\|max]` | Set API limiting mode |
| `scaff stats [-m YYYY-MM]` | Token usage statistics with burn rate projections |
| `scaff analytics [-p daily\|weekly\|monthly]` | Performance metrics and usage breakdown |
| `scaff state` | Application state overview |
| `scaff export [-f json\|csv] [-o FILE]` | Export token usage data |
| `scaff errors [-r]` | Error reports and diagnostics |
| `scaff cache [status\|clear]` | Manage response cache |

### Advanced

| Command | Description |
|---------|-------------|
| `scaff examples` | Show 6 ready-to-use example agents |
| `scaff info` | Version, features, and configuration overview |
| `scaff estimate "..."` | Preview cost across all modes before generating |
| `scaff monitor [-r SECS] [--once]` | Live TUI dashboard with budget gauges |
| `scaff completions <shell>` | Generate shell completion scripts |
| `scaff edit` | Modify existing generated agents |
| `scaff deploy` | Generate deployment configs (Docker, Railway, Render) |
| `scaff test` | Run agent against test prompts |
| `scaff upgrade` | Regenerate agent preserving user changes |

### Common Flags

| Flag | Description |
|------|-------------|
| `--cheap, -c` | Use `gpt-4o-mini` — save ~90% on API costs |
| `--show-cost` | Print estimated token count and cost before calling |
| `--no-cache` | Force fresh API call, bypassing cache |
| `-v, --verbose` | Show detailed generation steps |
| `--tokens, -t` | Show detailed token usage and remaining budget |
| `-o, --output` | Output directory (default: `./agent-output`) |
| `-m, --model` | Model override (default: from config) |
| `-p, --provider` | AI provider (openai, anthropic, gemini, ollama) |

---

## API Modes & Budget Management

scaff provides three API limiting modes with hard monthly budgets:

| Mode | Monthly Budget | Monthly Tokens | Model | Rate Limit |
|------|---------------|----------------|-------|------------|
| **MIN** | $0.25 | 1,000,000 | `gpt-4o-mini` | 6 req/min |
| **MEDIUM** | $1.25 | 5,000,000 | `gpt-4o` | 12 req/min |
| **MAX** | $5.00 | 20,000,000 | `gpt-4o` | 30 req/min |

```
scaff mode min       # Cost-optimized mode
scaff mode medium    # Balanced mode (default)
scaff mode max       # Quality-optimized mode
scaff stats          # View burn rate, projections, and remaining budget
```

Budget warnings are displayed at **50%**, **75%**, and **90%** of the monthly limit. The monitor dashboard shows a real-time gauge:

```
┌─ Budget — MIN mode ────────────────────────────────────┐
│ ▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░ 62%                              │
└────────────────────────────────────────────────────────┘
```

---

## Token Tracking & Analytics

Every API request is tracked per-session and per-month with persistent JSON storage at `~/.scaff/token_tracker.json`:

```bash
scaff stats                    # Monthly usage + burn rate + projections
scaff stats -m 2026-05         # Historical month
scaff export                   # CSV of all token records
scaff export -f json -o data.json  # JSON export
scaff analytics -p monthly     # Aggregate performance metrics
scaff monitor                  # Live TUI dashboard
```

---

## Example Agents

| Agent | Command |
|-------|---------|
| **Email Assistant** | `scaff run "summarize my emails and flag urgent ones"` |
| **GitHub Helper** | `scaff run "monitor issues and auto-label them by priority"` |
| **Code Reviewer** | `scaff run "analyze code for security issues"` |
| **Weather Bot** | `scaff run "fetch weather and format it nicely"` |
| **News Curator** | `scaff run "aggregate news from multiple sources"` |
| **Meeting Assistant** | `scaff run "summarize meeting notes and extract action items"` |

---

## Architecture

scaff uses a **hybrid architecture** — a Rust CLI frontend paired with a Python generation backend:

```
┌─────────────────────────────────────────────────────┐
│                 Rust CLI (frontend)                   │
│                                                       │
│  clap (argument parsing)                              │
│  ratatui + crossterm (TUI dashboard)                  │
│  dialoguer (interactive prompts)                      │
│  indicatif (progress spinners)                        │
│  clap_complete (shell completions)                    │
│  serde (config + token tracking persistence)          │
└──────────────────────┬──────────────────────────────┘
                       │ JSON stdin/stdout
                       ▼
┌─────────────────────────────────────────────────────┐
│             Python Generation Engine (backend)        │
│                                                       │
│  OpenAI / Anthropic / Gemini / Ollama API clients     │
│  Token bucket rate limiter (MIN/MEDIUM/MAX)           │
│  Token counter (hybrid, ±5% accuracy)                 │
│  Session manager + memory monitoring                  │
│  Jinja2 template rendering                            │
│  Retry logic + exponential backoff                    │
└─────────────────────────────────────────────────────┘
```

The Rust CLI handles all user-facing UX — argument parsing, tab completion, TUI monitoring, token tracking — while the Python backend handles AI generation. Communication occurs via a JSON protocol over stdin/stdout.

---

## Benchmarks

| Metric | MIN (gpt-4o-mini) | MEDIUM (gpt-4o) | MAX (gpt-4o) |
|--------|-------------------|------------------|---------------|
| Generation time | ~8s | ~12s | ~15s |
| Cost per agent | ~$0.0003 | ~$0.002 | ~$0.005 |
| Tokens per agent | ~2,000 | ~5,000 | ~8,000 |
| Cache TTL | 30 days | 14 days | 7 days |

---

## Development

### Prerequisites

- **Rust 1.75+** — [rustup.rs](https://rustup.rs)
- **Python 3.11+**
- **OpenAI API key** (or alternative provider key)

### Build & Test

```bash
# Rust CLI
cargo build              # debug
cargo build --release    # release (~2MB binary)
cargo test

# Python backend
pip install -e .
pytest

# Full build (Python + Rust)
make all
```

### Project Structure

```
scaff/
├── src/                    # Rust CLI source
│   ├── main.rs             # Entry point
│   ├── cli.rs              # 22 clap commands
│   ├── config.rs           # ~/.scaff/config.json
│   ├── token_tracker.rs    # Token usage persistence
│   ├── rate_limiter.rs     # Token bucket + mode config
│   ├── cache.rs            # MD5-keyed response cache
│   ├── python_bridge.rs    # JSON stdin/stdout protocol
│   ├── shell.rs            # Interactive REPL
│   ├── monitor.rs          # ratatui TUI dashboard
│   └── display.rs          # Console formatting helpers
├── scaff/                   # Python generation backend
│   ├── json_mode.py        # JSON bridge entry point
│   ├── generator.py        # OpenAI interaction + agent design
│   ├── writer.py           # File generation from templates
│   ├── request_enforcer.py # Token bucket rate limiter
│   ├── token_tracker.py    # Session + monthly tracking
│   ├── token_counter.py    # Hybrid token estimation
│   ├── pricing.py          # Cost calculation
│   └── config.py           # Configuration management
├── Cargo.toml
├── pyproject.toml
├── Makefile
└── README.md
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `OPENAI_API_KEY not set` | `export OPENAI_API_KEY="sk-..."` |
| Rate limited | Wait or switch mode: `scaff mode min` |
| `ModuleNotFoundError` | `pip install -r requirements.txt` |
| Budget exceeded | `scaff mode min` to switch to cheapest mode |

---

## License

MIT — free to use, modify, share. See [LICENSE](LICENSE) for details.
