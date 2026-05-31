"""File writer that renders Jinja2 templates and writes generated files."""

import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any

from jinja2 import Environment, PackageLoader, select_autoescape

from . import __version__

BASE_GENERATED_DEPENDENCIES: dict[str, list[str]] = {
    "openai": ["openai>=1.0.0"],
    "anthropic": ["anthropic>=0.49.0"],
    "gemini": ["google-generativeai>=0.8.0"],
    "ollama": ["ollama>=0.4.0"],
}


def _merge_dependencies(
    dependencies: list[str] | str,
    provider: str = "openai",
) -> list[str]:
    """Ensure generated agents include their required runtime dependency."""
    if isinstance(dependencies, str):
        dependencies = [dependencies]

    base = BASE_GENERATED_DEPENDENCIES.get(provider, BASE_GENERATED_DEPENDENCIES["openai"])
    merged: list[str] = []
    seen: set[str] = set()

    for dependency in [*base, *dependencies]:
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


PATTERN_NAMES: dict[str, str] = {
    "http_fetch": "HTTP/API fetch",
    "github": "GitHub API",
    "weather": "Weather API",
    "file_read": "File reader",
    "file_write": "File writer",
    "send": "Notification",
    "transform": "Data transform",
    "analyze": "Data analysis",
    "datetime": "Date/time",
    "list": "Enumeration",
    "default": "Default (structured JSON)",
}


def _get_pattern_name(name: str, desc: str) -> str:
    """Determine which implementation pattern a tool will match."""
    name_lower = name.lower()
    desc_lower = desc.lower()
    if any(kw in name_lower or kw in desc_lower for kw in ["search", "fetch", "lookup", "find", "query"]):
        return "http_fetch"
    if any(kw in desc_lower for kw in ["github", "issue", "repo", "pull request"]):
        return "github"
    if any(kw in desc_lower for kw in ["weather", "forecast", "temperature"]):
        return "weather"
    if any(kw in name_lower for kw in ["read", "load", "open", "parse", "import"]):
        return "file_read"
    if any(kw in name_lower for kw in ["write", "save", "store", "log", "export"]):
        return "file_write"
    if any(kw in name_lower for kw in ["send", "email", "notify", "alert", "message"]):
        return "send"
    if any(kw in name_lower for kw in ["format", "convert", "transform", "translate"]):
        return "transform"
    if any(kw in name_lower for kw in ["analyze", "count", "summarize", "compute", "calculate"]):
        return "analyze"
    if any(kw in name_lower for kw in ["date", "time", "schedule", "remind"]):
        return "datetime"
    if any(kw in name_lower for kw in ["list", "enum", "show", "display", "get_"]):
        return "list"
    return "default"


def _validate_tool_patterns(tools: list[dict[str, Any]]) -> list[str]:
    """Validate tool names/descriptions against patterns. Returns warnings."""
    warnings = []
    for tool in tools:
        name = tool.get("name", "")
        desc = tool.get("description", "")
        pattern = _get_pattern_name(name, desc)
        if pattern == "default":
            warnings.append(
                f"Tool '{name}' didn't match any specific pattern - "
                f"using default JSON response. Rename it to match a pattern like "
                f"search_, read_, write_, etc. for a real implementation."
            )
    return warnings


def _generate_tool_implementations(
    tools: list[dict[str, Any]],
    verbose: bool = False,
) -> tuple[list[str], dict[str, str]]:
    """Generate working Python code for each tool based on name/description patterns.
    
    Returns (list of code strings, dict mapping tool name to pattern name).
    """
    results = []
    pattern_map: dict[str, str] = {}
    for tool in tools:
        name = tool.get("name", "")
        desc = tool.get("description", "")
        params = tool.get("parameters", {})
        name_lower = name.lower()
        desc_lower = desc.lower()

        code = _match_implementation(name, name_lower, desc_lower, params)
        pattern = _get_pattern_name(name, desc)
        results.append(code)
        pattern_map[name] = pattern
    return results, pattern_map


def _match_implementation(
    name: str, name_lower: str, desc_lower: str, params: dict,
) -> str:
    """Match a tool name/description to an implementation pattern."""
    # HTTP/API fetch tools
    if any(kw in name_lower or kw in desc_lower for kw in
           ["search", "fetch", "lookup", "find", "query"]):
        return _impl_http_fetch(name, params)

    # GitHub tools
    if any(kw in desc_lower for kw in ["github", "issue", "repo", "pull request"]):
        return _impl_github(name, params)

    # Weather tools
    if any(kw in desc_lower for kw in ["weather", "forecast", "temperature"]):
        return _impl_weather(name, params)

    # File read tools
    if any(kw in name_lower for kw in ["read", "load", "open", "parse", "import"]):
        return _impl_file_read(name, params)

    # File write tools
    if any(kw in name_lower for kw in ["write", "save", "store", "log", "export"]):
        return _impl_file_write(name, params)

    # Send/notify tools
    if any(kw in name_lower for kw in ["send", "email", "notify", "alert", "message"]):
        return _impl_send(name, params)

    # Transform tools
    if any(kw in name_lower for kw in ["format", "convert", "transform", "translate"]):
        return _impl_transform(name, params)

    # Analyze tools
    if any(kw in name_lower for kw in ["analyze", "count", "summarize", "compute", "calculate"]):
        return _impl_analyze(name, params)

    # Date/time tools
    if any(kw in name_lower for kw in ["date", "time", "schedule", "remind"]):
        return _impl_datetime(name, params)

    # List/enumerate tools
    if any(kw in name_lower for kw in ["list", "enum", "show", "display", "get_"]):
        return _impl_list(name, params)

    return _impl_default(name, params)


def _impl_http_fetch(name: str, params: dict) -> str:
    """Generate HTTP GET implementation."""
    param_names = list(params.keys())
    url_param = param_names[0] if param_names else "query"
    lines = [
        '    """Fetch data from an external source.',
        "    Auto-generated implementation uses requests library.",
        '    """',
        "    import requests",
        f'    url = f"https://api.example.com/{name}?{url_param}={{{url_param}}}"',
        "    try:",
        "        resp = requests.get(url, timeout=10)",
        "        resp.raise_for_status()",
        f'        return json.dumps({{"tool": "{name}", "data": resp.json(), "status": "ok"}})',
        "    except Exception as e:",
        f'        return json.dumps({{"tool": "{name}", "error": str(e), "status": "error"}})',
    ]
    return "\n".join(lines)


def _impl_github(name: str, params: dict) -> str:
    """Generate GitHub API implementation."""
    lines = [
        '    """Interact with GitHub API.',
        "    Auto-generated - requires GITHUB_TOKEN env var for authenticated requests.",
        '    """',
        "    import os",
        "    import requests",
        '    gh_token = os.environ.get("GITHUB_TOKEN", "")',
        '    gh_headers = {"Authorization": f"Bearer {gh_token}"} if gh_token else {}',
        "    gh_params = {}",
        "    for k, v in locals().items():",
        '        if k not in ("os", "requests", "gh_token", "gh_headers", "gh_params"):',
        '            gh_params[k] = v',
        "    try:",
        '        resp = requests.get("https://api.github.com/",',
        "                    headers=gh_headers,",
        "                    params=gh_params,",
        "                    timeout=10)",
        "        resp.raise_for_status()",
        f'        return json.dumps({{"tool": "{name}", "data": resp.json(), "status": "ok"}})',
        "    except Exception as e:",
        f'        return json.dumps({{"tool": "{name}", "error": str(e), "status": "error"}})',
    ]
    return "\n".join(lines)


def _impl_weather(name: str, params: dict) -> str:
    """Generate weather API implementation."""
    param_names = list(params.keys())
    location = param_names[0] if param_names else "location"
    lines = [
        '    """Fetch weather data.',
        "    Auto-generated - uses wttr.in for weather data (no API key needed).",
        '    """',
        "    import requests",
        "    try:",
        f'        resp = requests.get(f"https://wttr.in/{{{location}}}?format=j1",',
        "                    timeout=10)",
        "        resp.raise_for_status()",
        "        data = resp.json()",
        '        current = data.get("current_condition", [{}])[0]',
        '        return json.dumps({',
        f'            "tool": "{name}",',
        '            "temperature": current.get("temp_C", "unknown"),',
        '            "condition": current.get("weatherDesc", [{}])[0].get("value", ""),',
        '            "humidity": current.get("humidity", ""),',
        '            "status": "ok",',
        "        })",
        "    except Exception as e:",
        f'        return json.dumps({{"tool": "{name}", "error": str(e), "status": "error"}})',
    ]
    return "\n".join(lines)


def _impl_file_read(name: str, params: dict) -> str:
    """Generate file reading implementation."""
    param_names = list(params.keys())
    path_param = param_names[0] if param_names else "path"
    lines = [
        '    """Read data from a file.',
        "    Auto-generated implementation using pathlib.",
        '    """',
        "    from pathlib import Path",
        "    try:",
        f"        p = Path({path_param})",
        "        if not p.exists():",
        f'            return json.dumps({{"tool": "{name}", "error": f"File not found: {{{path_param}}}", "status": "error"}})',
        '        content = p.read_text(encoding="utf-8")',
        f'        return json.dumps({{"tool": "{name}", "content": content, "size": len(content), "status": "ok"}})',
        "    except Exception as e:",
        f'        return json.dumps({{"tool": "{name}", "error": str(e), "status": "error"}})',
    ]
    return "\n".join(lines)


def _impl_file_write(name: str, params: dict) -> str:
    """Generate file writing implementation."""
    param_names = list(params.keys())
    path_param = param_names[0] if param_names else "path"
    content_param = param_names[1] if len(param_names) > 1 else "content"
    lines = [
        '    """Write data to a file.',
        "    Auto-generated implementation using pathlib.",
        '    """',
        "    from pathlib import Path",
        "    try:",
        f"        p = Path({path_param})",
        "        p.parent.mkdir(parents=True, exist_ok=True)",
        f'        p.write_text(str({content_param}), encoding="utf-8")',
        f'        return json.dumps({{"tool": "{name}", "path": str(p), "size": len(str({content_param})), "status": "ok"}})',
        "    except Exception as e:",
        f'        return json.dumps({{"tool": "{name}", "error": str(e), "status": "error"}})',
    ]
    return "\n".join(lines)


def _impl_send(name: str, params: dict) -> str:
    """Generate send/notify implementation (prints to stdout)."""
    lines = [
        '    """Send a message or notification.',
        "    Auto-generated - prints to stdout (replace with real send logic).",
        '    """',
        f'    print(f"[{name}] Parameters received: {{{{locals()}}}}")',
        f'    return json.dumps({{"tool": "{name}", "message": "Sent successfully", "status": "ok"}})',
    ]
    return "\n".join(lines)


def _impl_transform(name: str, params: dict) -> str:
    """Generate text/data transform implementation."""
    param_names = list(params.keys())
    input_param = param_names[0] if param_names else "text"
    lines = [
        '    """Transform or convert data.',
        "    Auto-generated implementation.",
        '    """',
        "    try:",
        f"        result = str({input_param})",
        f'        return json.dumps({{"tool": "{name}", "result": result, "length": len(result), "status": "ok"}})',
        "    except Exception as e:",
        f'        return json.dumps({{"tool": "{name}", "error": str(e), "status": "error"}})',
    ]
    return "\n".join(lines)


def _impl_analyze(name: str, params: dict) -> str:
    """Generate data analysis implementation."""
    lines = [
        '    """Analyze data and return insights.',
        "    Auto-generated analysis implementation.",
        '    """',
        "    import json",
        "    data = dict(locals())",
        '    data.pop("json", None)',
        "    word_count = sum(len(str(v).split()) for v in data.values())",
        "    char_count = sum(len(str(v)) for v in data.values())",
        "    return json.dumps({",
        f'        "tool": "{name}",',
        '        "analysis": "Auto-analyzed",',
        '        "input_fields": list(data.keys()),',
        '        "total_words": word_count,',
        '        "total_chars": char_count,',
        '        "status": "ok",',
        "    })",
    ]
    return "\n".join(lines)


def _impl_datetime(name: str, params: dict) -> str:
    """Generate date/time implementation."""
    lines = [
        '    """Get or manipulate date/time information.',
        "    Auto-generated using datetime module.",
        '    """',
        "    from datetime import datetime",
        "    now = datetime.now()",
        "    return json.dumps({",
        f'        "tool": "{name}",',
        '        "current_time": now.isoformat(),',
        '        "current_date": now.strftime("%Y-%m-%d"),',
        '        "weekday": now.strftime("%A"),',
        '        "status": "ok",',
        "    })",
    ]
    return "\n".join(lines)


def _impl_list(name: str, params: dict) -> str:
    """Generate list/enumeration implementation."""
    lines = [
        '    """List or enumerate items.',
        "    Auto-generated listing implementation.",
        '    """',
        "    data = dict(locals())",
        f'    return json.dumps({{"tool": "{name}", "items": [{{"id": i, "value": v}} for i, v in enumerate(data.values())], "count": len(data), "status": "ok"}})',
    ]
    return "\n".join(lines)


def _impl_default(name: str, params: dict) -> str:
    """Default implementation - returns structured info about what the tool would do."""
    lines = [
        f'    """{name} - implement your tool logic here.',
        "    Replace this with real implementation.",
        '    """',
        "    return json.dumps({",
        f'        "tool": "{name}",',
        '        "result": f"Tool {name} executed with args: {locals()}",',
        '        "status": "ok",',
        "    })",
    ]
    return "\n".join(lines)


def _collect_tool_imports(tools: list[dict[str, Any]]) -> list[str]:
    """Collect all Python imports needed by generated tool implementations."""
    imports = []
    all_code = " ".join(t.get("_implementation_code", "") for t in tools)
    if "import requests" in all_code:
        imports.append("requests")
    return imports


def _add_extra_deps(
    deps: list[str],
    ui_mode: str | None,
    schedule: str | None,
) -> list[str]:
    """Add dependencies needed for UI/schedule modes."""
    extra_deps = []
    if ui_mode == "fastapi":
        extra_deps.extend(["fastapi>=0.115.0", "uvicorn>=0.34.0", "pydantic>=2.0.0"])
    elif ui_mode == "streamlit":
        extra_deps.append("streamlit>=1.40.0")
    if schedule:
        extra_deps.append("apscheduler>=3.10.0")
    for dep in extra_deps:
        if dep.split(">=")[0].strip().lower() not in str(deps).lower():
            deps.append(dep)
    return deps


def write_generated_files(
    agent_spec: dict[str, Any],
    output_dir: str,
    provider: str = "openai",
    ui_mode: str | None = None,
    schedule: str | None = None,
    memory: str | None = None,
    verbose: bool = False,
) -> list[str]:
    """
    Render Jinja2 templates and write files to output directory.
    
    Args:
        agent_spec: Parsed agent specification from OpenAI
        output_dir: Path to output directory
        provider: AI provider (openai, anthropic, gemini)
        ui_mode: Web UI mode (None, "fastapi", "streamlit")
        schedule: Cron expression for scheduled runs (None to disable)
        memory: Memory backend (None, "sqlite")
        verbose: Print pattern match info
        
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
    
    deps = _merge_dependencies(agent_spec.get("dependencies", []), provider)
    
    # Generate real tool implementations
    tools = agent_spec.get("tools", [])
    impl_codes, pattern_map = _generate_tool_implementations(tools)
    for tool, code in zip(tools, impl_codes):
        tool["_implementation_code"] = code

    # Add requests to deps if any tool uses HTTP
    extra_imports = _collect_tool_imports(tools)
    if "requests" in extra_imports and "requests" not in str(deps):
        deps.insert(0, "requests")
    
    # Add UI/schedule dependencies
    deps = _add_extra_deps(deps, ui_mode, schedule)
    
    spec_with_meta = {
        **agent_spec,
        "generated_at": timestamp,
        "tool_count": len(tools),
        "dependency_count": len(deps),
        "dependencies": deps,
        "uses_requests": "requests" in extra_imports,
        "provider": provider,
        "pattern_map": pattern_map,
        "use_memory": memory == "sqlite",
    }

    if verbose:
        print("[*] Tool implementation pattern mapping:")
        for t in tools:
            tname = t.get("name", "?")
            pat = pattern_map.get(tname, "default")
            pname = PATTERN_NAMES.get(pat, pat)
            print(f"    {tname} -> {pname}")
        warnings = _validate_tool_patterns(tools)
        if warnings:
            for w in warnings:
                print(f"    [WARN] {w}")
    
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
    
    # Render and write UI app if requested
    if ui_mode == "fastapi":
        ui_template = env.get_template("fastapi_app.py.j2")
        ui_content = ui_template.render(spec_with_meta)
        ui_file = output_path / "app.py"
        ui_file.write_text(ui_content, encoding="utf-8")
        created_files.append(str(ui_file))
    elif ui_mode == "streamlit":
        ui_template = env.get_template("streamlit_app.py.j2")
        ui_content = ui_template.render(spec_with_meta)
        ui_file = output_path / "app.py"
        ui_file.write_text(ui_content, encoding="utf-8")
        created_files.append(str(ui_file))
    
    # Render and write schedule.py if requested
    if schedule:
        schedule_spec = {**spec_with_meta, "schedule_cron": schedule}
        sched_template = env.get_template("schedule.py.j2")
        sched_content = sched_template.render(schedule_spec)
        sched_file = output_path / "schedule.py"
        sched_file.write_text(sched_content, encoding="utf-8")
        created_files.append(str(sched_file))
    
    # Render and write memory.py if requested
    if memory == "sqlite":
        mem_template = env.get_template("memory.py.j2")
        mem_content = mem_template.render(spec_with_meta)
        mem_file = output_path / "memory.py"
        mem_file.write_text(mem_content, encoding="utf-8")
        created_files.append(str(mem_file))
    
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
    env_example = (
        "# AI Provider API Keys\n"
        "# Uncomment and fill in the key for your provider:\n\n"
        "# OpenAI:\n"
        "# OPENAI_API_KEY=sk-...\n\n"
        "# Anthropic:\n"
        "# ANTHROPIC_API_KEY=sk-ant-...\n\n"
        "# Google (Gemini):\n"
        "# GOOGLE_API_KEY=AIza...\n\n"
        "# Ollama (local):\n"
        "# OLLAMA_HOST=http://localhost:11434\n"
    )
    env_file = output_path / ".env.example"
    env_file.write_text(env_example, encoding="utf-8")
    created_files.append(str(env_file))
    
    # Write README.md
    agent_name = agent_spec.get("agent_name", "agent")
    agent_desc = agent_spec.get("description", "An AI agent")
    display_name = agent_name.replace("-", " ").title()
    has_http_tools = agent_spec.get("uses_requests", False)
    tool_count = len(agent_spec.get("tools", []))
    
    # Extra README sections for UI/schedule modes
    extra_run_instructions = ""
    extra_file_table_rows = ""
    if ui_mode == "fastapi":
        extra_run_instructions = """
### 4. Run the web server (alternative)

```bash
uvicorn app:app --reload
```

Then visit http://localhost:8000 in your browser. The API docs are at
http://localhost:8000/docs.

Or use the API:
```bash
curl -X POST http://localhost:8000/api/chat \\
  -H "Content-Type: application/json" \\
  -d '{"message": "Hello!"}'
```
"""
        extra_file_table_rows = "\n| `app.py` | FastAPI web server with REST API |"
    elif ui_mode == "streamlit":
        extra_run_instructions = """
### 4. Run the web UI (alternative)

```bash
streamlit run app.py
```

Then visit the URL shown in your terminal (usually http://localhost:8501).
"""
        extra_file_table_rows = "\n| `app.py` | Streamlit chat UI |"
    
    if schedule:
        extra_run_instructions += f"""
### 5. Run on a schedule (alternative)

```bash
python schedule.py
```

The agent runs on the cron schedule: `{schedule}`.
Set `SCHEDULE_CRON` env var to change the schedule, and `AGENT_PROMPT`
to change what the agent runs each time.
"""
        extra_file_table_rows += "\n| `schedule.py` | Scheduled agent runner (APScheduler) |"
    
    run_modes_text = "### 3. Run the agent\n\n```bash\npython agent.py\n```\n\nType your request when prompted. The agent supports **multi-turn conversation**\n- keep chatting until you type `exit`. Responses stream in real-time." + extra_run_instructions
    
    provider_info = {
        "openai": ("OpenAI", "gpt-4o", "https://platform.openai.com/api-keys", "OPENAI_API_KEY", "sk-..."),
        "anthropic": ("Anthropic", "claude-sonnet-4-20250514", "https://console.anthropic.com/", "ANTHROPIC_API_KEY", "sk-ant-..."),
        "gemini": ("Google (Gemini)", "gemini-2.0-flash", "https://aistudio.google.com/apikey", "GOOGLE_API_KEY", "AIza..."),
        "ollama": ("Ollama (local)", "llama3.2", "https://ollama.ai", "OLLAMA_HOST", "http://localhost:11434"),
    }
    prov_name, default_model, prov_url, prov_env_var, prov_key_example = provider_info.get(provider, provider_info["openai"])
    
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

You need a {prov_name} API key to run this agent.

1. Go to {prov_url}
2. Create a new API key
3. Copy the key (it starts with `{prov_key_example}`)

Then set it in your terminal:

- **Mac / Linux:** `export {prov_env_var}="{prov_key_example}"`
- **Windows:** `$env:{prov_env_var} = "{prov_key_example}"`

Or copy `.env.example` to `.env` and fill in your key.

{run_modes_text}

---

## What's in this folder

| File | What it does |
|------|-------------|
| `agent.py` | The main agent with streaming + multi-turn chat |
| `tools.py` | Tools with auto-generated implementations |
| `requirements.txt` | Python packages needed |
| `mcp.json` | Configuration for MCP servers |
| `.env.example` | Template for your API key |
| `README.md` | This file |
| `MANIFEST.json` | Generation metadata |{extra_file_table_rows}

---

## Agent details

- **Name:** {display_name}
- **Provider:** {prov_name}
- **Tools:** {tool_count} tools available
- **Default Model:** {default_model} (can be changed in agent.py)
- **Features:** Streaming responses, multi-turn conversation, auto-implemented tools

### Tools

{_format_tools(agent_spec.get('tools', []))}

---

## Customizing your agent

### Change the model

Open `agent.py` and look for:

```python
MODEL = "{default_model}"
```

Change to any model supported by {prov_name}.

### Change the behavior

Open `agent.py` and look for `SYSTEM_PROMPT`. Edit the instructions inside.

### Improve a tool

Open `tools.py`. Each tool has an auto-generated implementation that
you can replace with real logic for your use case.

### Switch providers

The generated agent uses **{prov_name}** by default. To use a different provider,
regenerate with `scaff create --provider <name>`.

---

## Troubleshooting

**"API key not found"**
- You haven't set your API key yet. See step 2 above.

**"pip is not recognized"** (Windows) or **"command not found: pip"** (Mac/Linux)
- Python is not installed or not in your PATH. Download Python from python.org.

**"ModuleNotFoundError"**
- Run `pip install -r requirements.txt` again.

**Rate limited**
- You've hit the API rate limit. Wait a moment and try again.

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
        "provider": provider,
        "ui_mode": ui_mode,
        "schedule": schedule,
        "memory": memory,
        "generated_at": timestamp,
        "scaff_version": __version__,
    }
    manifest_file = output_path / "MANIFEST.json"
    manifest_file.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    created_files.append(str(manifest_file))
    
    # Write spec.json (full agent spec for scaff edit support)
    spec_file = output_path / "spec.json"
    spec_file.write_text(json.dumps(agent_spec, indent=2), encoding="utf-8")
    created_files.append(str(spec_file))
    
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
