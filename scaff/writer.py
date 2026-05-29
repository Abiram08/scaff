"""File writer that renders Jinja2 templates and writes generated files."""

import json
from datetime import datetime
from pathlib import Path
from typing import Any

from jinja2 import Environment, PackageLoader, select_autoescape

from . import __version__

BASE_GENERATED_DEPENDENCIES = ["openai>=1.0.0"]


def _merge_dependencies(dependencies: list[str] | str) -> list[str]:
    """Ensure generated agents include their required runtime dependency."""
    if isinstance(dependencies, str):
        dependencies = [dependencies]

    merged: list[str] = []
    seen: set[str] = set()

    for dependency in [*BASE_GENERATED_DEPENDENCIES, *dependencies]:
        name = (
            dependency.split("==")[0]
            .split(">=")[0]
            .split("<=")[0]
            .split("~=")[0]
            .strip()
            .lower()
        )
        if name and name not in seen:
            seen.add(name)
            merged.append(dependency)

    return merged


def write_generated_files(
    agent_spec: dict[str, Any],
    output_dir: str,
) -> list[str]:
    """
    Render Jinja2 templates and write files to output directory.
    
    Args:
        agent_spec: Parsed agent specification from OpenAI
        output_dir: Path to output directory
        
    Returns:
        List of created file paths
    """
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)
    
    # Set up Jinja2 environment
    env = Environment(
        loader=PackageLoader("scaff", "templates"),
        autoescape=select_autoescape(),
    )
    
    created_files = []
    timestamp = datetime.now().isoformat()
    
    deps = _merge_dependencies(agent_spec.get("dependencies", []))
    spec_with_meta = {
        **agent_spec,
        "generated_at": timestamp,
        "tool_count": len(agent_spec.get("tools", [])),
        "dependency_count": len(deps),
        "dependencies": deps,
    }
    
    # Render and write agent.py
    agent_template = env.get_template("agent.py.j2")
    agent_content = agent_template.render(spec_with_meta)
    agent_file = output_path / "agent.py"
    agent_file.write_text(agent_content, encoding="utf-8")
    created_files.append(str(agent_file))
    
    # Render and write tools.py
    tools_template = env.get_template("tools.py.j2")
    tools_content = tools_template.render(spec_with_meta)
    tools_file = output_path / "tools.py"
    tools_file.write_text(tools_content, encoding="utf-8")
    created_files.append(str(tools_file))
    
    # Render and write mcp.json
    mcp_template = env.get_template("mcp.json.j2")
    mcp_content = mcp_template.render(spec_with_meta)
    mcp_file = output_path / "mcp.json"
    mcp_file.write_text(mcp_content, encoding="utf-8")
    created_files.append(str(mcp_file))
    
    # Write requirements.txt
    deps_content = "\n".join(deps) + "\n" if deps else ""
    reqs_file = output_path / "requirements.txt"
    reqs_file.write_text(deps_content, encoding="utf-8")
    created_files.append(str(reqs_file))
    
    # Write .gitignore
    gitignore_content = """.env
__pycache__/
*.pyc
.DS_Store
"""
    gitignore_file = output_path / ".gitignore"
    gitignore_file.write_text(gitignore_content, encoding="utf-8")
    created_files.append(str(gitignore_file))
    
    # Write .env.example
    env_example = "OPENAI_API_KEY=your-key-here\n"
    env_file = output_path / ".env.example"
    env_file.write_text(env_example, encoding="utf-8")
    created_files.append(str(env_file))
    
    # Write README.md
    agent_name = agent_spec.get("agent_name", "agent")
    agent_desc = agent_spec.get("description", "An AI agent")
    display_name = agent_name.replace("-", " ").title()
    readme_content = f"""# {display_name}

> {agent_desc}

This AI agent was created by **scaff** - a tool that turns plain English into ready-to-run AI agents.

---

## Quick Start

### 1. Install dependencies

Open a terminal in this folder and run:

```bash
pip install -r requirements.txt
```

### 2. Set your API key

You need an OpenAI API key to run this agent.

1. Go to https://platform.openai.com/api-keys
2. Click **"Create new secret key"**
3. Copy the key (it starts with `sk-...`)

Then set it in your terminal:

- **Mac / Linux:** `export OPENAI_API_KEY="sk-..."`
- **Windows:** `$env:OPENAI_API_KEY = "sk-..."`

Or copy `.env.example` to `.env` and put your key there.

### 3. Run the agent

```bash
python agent.py
```

Type your request when prompted. The agent will use AI to complete it.

---

## What's in this folder

| File | What it does |
|------|-------------|
| `agent.py` | The main agent - this is what you run |
| `tools.py` | Tools the agent can use (some need implementation) |
| `requirements.txt` | Python packages needed |
| `mcp.json` | Configuration for MCP servers |
| `.env.example` | Template for your API key |
| `README.md` | This file |
| `MANIFEST.json` | Generation metadata |

---

## Agent details

- **Name:** {display_name}
- **Tools:** {len(agent_spec.get('tools', []))} tools available
- **Model:** GPT-4o (can be changed in agent.py)

### Tools

{_format_tools(agent_spec.get('tools', []))}

---

## Customizing your agent

### Change the model

Open `agent.py` and look for:

```python
MODEL = "gpt-4o"
```

Change `gpt-4o` to another model like `gpt-4-turbo`.

### Change the behavior

Open `agent.py` and look for `SYSTEM_PROMPT`. Edit the instructions inside.

### Implement a tool

Open `tools.py`. Find the `# TODO` comments and replace them with real code.

---

## Troubleshooting

**"OpenAI API key not found"**
- You haven't set your API key yet. See step 2 above.

**"pip is not recognized"** (Windows) or **"command not found: pip"** (Mac/Linux)
- Python is not installed or not in your PATH. Download Python from python.org.

**"ModuleNotFoundError"**
- Run `pip install -r requirements.txt` again.

**Rate limited**
- You've hit OpenAI's rate limit. Wait a moment and try again.

---

## Built with scaff

This agent was automatically generated with [scaff](https://github.com/your-repo/scaff).

scaff turns plain English descriptions into complete AI agent projects in seconds.
"""
    readme_file = output_path / "README.md"
    readme_file.write_text(readme_content, encoding="utf-8")
    created_files.append(str(readme_file))
    
    # Write MANIFEST.json (metadata)
    manifest = {
        "agent_name": agent_spec.get("agent_name"),
        "description": agent_spec.get("description"),
        "generated_at": timestamp,
        "scaff_version": __version__,
    }
    manifest_file = output_path / "MANIFEST.json"
    manifest_file.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    created_files.append(str(manifest_file))
    
    return created_files


def _format_tools(tools: list[dict[str, Any]]) -> str:
    """Format tools for README."""
    if not tools:
        return "No tools configured."
    
    formatted = []
    for i, tool in enumerate(tools, 1):
        name = tool.get("name", "unknown")
        desc = tool.get("description", "No description")
        formatted.append(f"{i}. **{name}**\n   - {desc}")
    
    return "\n\n".join(formatted)
