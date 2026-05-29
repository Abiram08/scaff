<p align="center">
  <img src="https://img.shields.io/badge/python-3.11%2B-blue" alt="Python 3.11+">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License">
</p>

<h1 align="center">scaff</h1>
<p align="center">
  <i>Describe what you want. Get a working AI agent. No coding required.</i>
</p>

---

## What is scaff?

**scaff** is a tool that turns everyday English into a working AI agent.

Tell it what you want in plain words:

```
scaff run "summarize my emails and flag urgent ones"
```

One command. It generates the agent, installs everything, and starts it up.

---

## Quick Start (2 minutes)

### 1. Install Python

You need **Python 3.11 or newer** on your computer.

<details>
<summary><b>How do I check if I have Python?</b></summary>

Open a **terminal** (command prompt) and type:

```bash
python --version
```

If you see `Python 3.11.x` or higher, you're good.

**Don't have Python?**
- **Windows:** Download from [python.org](https://www.python.org/downloads/) - check "Add Python to PATH" during install
- **Mac:** Download from [python.org](https://www.python.org/downloads/) or run `brew install python@3.12`
- **Linux:** `sudo apt install python3 python3-pip` (Ubuntu/Debian)

</details>

<details>
<summary><b>What is a "terminal" and how do I open it?</b></summary>

- **Windows:** Press `Windows + R`, type `cmd`, press Enter. Or search for "PowerShell".
- **Mac:** Press `Cmd + Space`, type "Terminal", press Enter.
- **Linux:** Press `Ctrl + Alt + T`.

All commands below are typed into this window and run by pressing Enter.
</details>

### 2. Install scaff

Open a terminal and run:

```bash
pip install -e .
```

Make sure you're in the `scaff` folder (where this README is).

### 3. Get an API key

scaff uses OpenAI's AI to design your agent. You need a free API key.

1. Go to [https://platform.openai.com/api-keys](https://platform.openai.com/api-keys)
2. Click **"Create new secret key"**
3. Copy the key - it looks like `sk-...`

### 4. Set your API key

Paste the key in your terminal (one time only):

```bash
export OPENAI_API_KEY="sk-..."
```
<sub>Windows: use `$env:OPENAI_API_KEY = "sk-..."` instead</sub>

### 5. Create your first agent

```bash
scaff run "check the weather and tell me if I need an umbrella"
```

This will:
1. Generate your agent
2. Install required packages
3. Start the agent - just type your question!

---

## How to use scaff

### The fastest way: `scaff run`

One command does everything:

```bash
scaff run "describe what you want your agent to do"
```

scaff generates the agent, installs dependencies, and runs it immediately.

### Generate and run later: `scaff create`

If you want to save the agent for later:

```bash
scaff create "describe what you want" -o ./my-agent
```

Then:
```bash
cd my-agent
pip install -r requirements.txt
python agent.py
```

### Step-by-step: `scaff wizard`

Prefer to be guided? This walks you through each step interactively:

```bash
scaff wizard
```

### Try examples: `scaff examples`

See 6 ready-to-use agent ideas you can create right now:

```bash
scaff examples
```

### Test a description: `scaff validate`

Check if a description works before generating:

```bash
scaff validate "summarize my emails"
```

---

## Example agents you can create

| Agent idea | Command |
|-----------|---------|
| **Email assistant** | `scaff run "summarize emails and flag urgent ones"` |
| **GitHub helper** | `scaff run "monitor issues and auto-label them"` |
| **Code reviewer** | `scaff run "analyze code for security issues"` |
| **Weather bot** | `scaff run "fetch weather and format it nicely"` |
| **News curator** | `scaff run "aggregate news from multiple sources"` |
| **Slack bot** | `scaff run "manage slack channels and respond to messages"` |

---

## How it works

```
You describe ──> OpenAI designs ──> scaff generates ──> You run
```

1. **You type** what you want (e.g., "monitor GitHub issues")
2. **scaff asks** OpenAI to design the agent's tools and behavior
3. **scaff generates** a complete project with everything included
4. **You run** the agent and talk to it

scaff handles all the setup - tool-calling loops, error handling, retries, dependency management. You just describe what you need.

---

## What a generated agent can do

A generated agent can:
- Ask you what you need
- Use tools to gather information or take actions
- Talk back to you with results
- Handle errors and retry automatically

The agent code is **self-contained** - just `python agent.py`. No heavy frameworks.

---

## Commands at a glance

| Command | What it does |
|---------|-------------|
| `scaff run "..."` | Generate, install, and run - all in one |
| `scaff create "..."` | Generate agent files (run later) |
| `scaff wizard` | Guided setup step by step |
| `scaff examples` | Show 6 example agents |
| `scaff validate "..."` | Test a description without saving |
| `scaff config` | Change settings (model, output folder) |
| `scaff info` | Show version and system info |

### Options for `create` and `run`

- `-o, --output` - Where to save the agent (default: `./agent-output`)
- `-m, --model` - OpenAI model to use (default: `gpt-4o`)
- `-v, --verbose` - Show what's happening step by step

---

## Troubleshooting

### "OPENAI_API_KEY not set"

You need to set your API key:
- **Mac/Linux:** `export OPENAI_API_KEY="sk-..."`
- **Windows:** `$env:OPENAI_API_KEY = "sk-..."`

### "pip is not recognized"

Python/pip isn't installed or isn't in your system PATH.
- Download Python from [python.org](https://www.python.org/downloads/)
- During install, check **"Add Python to PATH"**

### "ModuleNotFoundError"

Run this in your agent's folder: `pip install -r requirements.txt`

### "API error" / rate limited

OpenAI's API has rate limits. Wait a minute and try again.

---

## Requirements

- **Python 3.11+** - [Download here](https://www.python.org/downloads/)
- **OpenAI API key** - [Get one free](https://platform.openai.com/api-keys)
- Works on **Windows, Mac, and Linux**

---

## Technology

scaff is built with:
- **Python** - the language
- **OpenAI** - designs the agent
- **Jinja2** - generates files from templates

No LangChain, no CrewAI, no complicated frameworks. Just clean, readable agent code.

---

## License

MIT - free to use, modify, and share.
