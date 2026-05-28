<p align="center">
  <img src="https://img.shields.io/badge/python-3.11%2B-blue" alt="Python 3.11+">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License">
  <img src="https://img.shields.io/badge/status-beta-yellow" alt="Beta">
</p>

<h1 align="center">scaff</h1>
<p align="center"><i>Turn plain English into ready-to-run AI agents.</i></p>

---

## What is scaff?

scaff is a CLI tool that takes a plain English description of an AI agent and generates a complete, runnable project. No boilerplate. No framework lock-in. Just describe what you want and get working code.

```
scaff create "summarize my emails and flag urgent ones"
```
↓ *OpenAI designs the agent* ↓  
↓ *scaff generates the project* ↓  

```
agent-output/
├── agent.py              # Runnable agent
├── tools.py              # Tool stubs (fill in the TODOs)
├── mcp.json              # MCP configuration
├── requirements.txt      # Dependencies
├── README.md             # Documentation
├── .gitignore            # Git rules
├── .env.example          # Secrets template
└── MANIFEST.json         # Metadata
```

Then: `cd agent-output && pip install -r requirements.txt && python agent.py`

---

## Quick Start

```bash
# Install
pip install -e .

# Set your API key
export OPENAI_API_KEY='sk-...'

# Create your first agent
scaff create "monitor my github issues and auto-label them by priority"

# Run it
cd agent-output && pip install -r requirements.txt && python agent.py
```

---

## Why scaff?

| Problem | scaff solution |
|---------|---------------|
| Writing the tool-calling loop from scratch | ✅ Auto-generated agent loop |
| Designing tool signatures | ✅ GPT-4o figures them out |
| Setting up project structure | ✅ 8 files generated |
| Writing documentation | ✅ Auto-generated README |
| MCP configuration | ✅ mcp.json included |
| Dependencies | ✅ requirements.txt |
| Error handling & retries | ✅ Built into generated agent |
| Starting from zero ideas | ✅ 6 example agents built in |

---

## CLI Commands

| Command | What it does |
|---------|-------------|
| `scaff create "..."` | Generate an agent project from a description |
| `scaff examples` | Show 6 pre-built example agents |
| `scaff validate "..."` | Test a description without writing files |
| `scaff config` | Manage defaults (model, output dir, etc.) |
| `scaff info` | Show version and system info |

### scaff create options

```
-o, --output    Output directory (default: ./agent-output)
-m, --model     OpenAI model (default: gpt-4o)
-v, --verbose   Show generation steps
```

---

## How It Works

```
You describe → OpenAI designs → scaff generates → You run
```

1. **You write** what you want in plain English
2. **scaff sends** it to OpenAI GPT-4o with a structured prompt
3. **OpenAI returns** a JSON spec: agent name, system prompt, tools, dependencies, MCP config
4. **scaff renders** Jinja2 templates into files
5. **You get** a complete project — install deps and run

The generated `agent.py` contains a full tool-calling loop:

```python
# Simplified: what's inside agent.py
def run_agent():
    while True:
        response = client.chat.completions.create(
            messages=[sys_prompt, user_msg],
            tools=get_tool_definitions()
        )
        if message.tool_calls:
            for tool in message.tool_calls:
                execute_tool(tool.name, tool.args)
                # feed result back to OpenAI
        else:
            print(message.content)  # final answer
            break
```

---

## Built-in Example Agents

| Agent | Command |
|-------|---------|
| **Email Summarizer** | `scaff create "summarize emails and flag urgent ones"` |
| **GitHub Labeler** | `scaff create "monitor issues and auto-label them"` |
| **Code Analyzer** | `scaff create "analyze code for security issues"` |
| **Weather Bot** | `scaff create "fetch weather data and format it nicely"` |
| **News Curator** | `scaff create "aggregate news from multiple sources"` |
| **Slack Assistant** | `scaff create "manage slack channels and respond to messages"` |

Run `scaff examples` to see them all.

---

## Customization

Generated agents are designed to be edited. Common changes:

| Change | Where |
|--------|-------|
| Implement a tool | `tools.py` — replace `# TODO` with real logic |
| Change the model | `agent.py` — edit `MODEL = "gpt-4o"` |
| Tweak behavior | `agent.py` — edit `SYSTEM_PROMPT` |
| Add dependencies | `requirements.txt` |

---

## Technology

- **Python 3.11+** — Core language
- **Typer** — CLI framework
- **OpenAI SDK** — GPT-4o access
- **Rich** — Terminal formatting
- **Jinja2** — Template rendering

Deliberately no LangChain, no CrewAI, no heavy frameworks. The generated agents are self-contained — just `python agent.py`.

---

## Project Structure

```
scaff/
├── scaff/
│   ├── __init__.py        # Package init
│   ├── main.py            # CLI: 5 commands
│   ├── generator.py       # OpenAI integration
│   ├── writer.py          # File generation
│   ├── config.py          # User config
│   ├── examples.py        # 6 example agents
│   └── templates/         # Jinja2 templates
│       ├── agent.py.j2    # → agent.py
│       ├── tools.py.j2    # → tools.py
│       └── mcp.json.j2    # → mcp.json
├── pyproject.toml
├── LICENSE
└── README.md
```

---

## Installation

### From source (recommended for now)

```bash
git clone https://github.com/your-repo/scaff.git
cd scaff
pip install -e .
```

### From PyPI (coming soon)

```bash
pip install scaff
```

---

## Requirements

- Python 3.11+
- An OpenAI API key (`OPENAI_API_KEY` environment variable)

---

## Troubleshooting

| Problem | Fix |
|---------|-----|
| `OPENAI_API_KEY not set` | `export OPENAI_API_KEY='sk-...'` |
| JSON parse error | Use `--verbose` to see raw response |
| Tool not found | Implement all `# TODO` in `tools.py` |
| Rate limited | Agent retries automatically; wait and retry |

---

## License

MIT

---

<p align="center">
  <i>scaff — from idea to agent, in one command.</i>
</p>
