"""Typer CLI entry point for scaff."""

import os
import subprocess
import sys
from datetime import datetime
from pathlib import Path

import typer
from rich.console import Console
from rich.table import Table
from rich.tree import Tree

from . import cache as scache
from . import __version__
from .config import ScaffConfig
from .examples import get_example_list, get_example_command
from .generator import generate_agent_spec
from .writer import write_generated_files

app = typer.Typer(
    help="scaff - Turn plain English into ready-to-run AI agents. No coding required.",
)
console = Console()


def _check_api_key() -> str | None:
    """Check if OPENAI_API_KEY is set, return helpful message if not."""
    key = os.environ.get("OPENAI_API_KEY", "").strip()
    if not key:
        return (
            "You need an OpenAI API key to use scaff.\n\n"
            "  1. Go to https://platform.openai.com/api-keys\n"
            "  2. Click 'Create new secret key'\n"
            "  3. Copy the key (it starts with 'sk-...')\n"
            "  4. Set it as an environment variable:\n\n"
            "     Mac / Linux:  export OPENAI_API_KEY='sk-...'\n"
            "     Windows:      $env:OPENAI_API_KEY = 'sk-...'\n"
        )
    return None


def _run_agent(
    output: str,
    description: str,
    model: str,
    verbose: bool,
    cheap: bool = False,
    show_cost: bool = False,
    no_cache: bool = False,
    tokens: bool = False,
    provider: str = "openai",
    ui_mode: str | None = None,
    schedule: str | None = None,
    memory: str | None = None,
) -> list[str]:
    """Generate agent and return created files list."""
    if Path(output).exists():
        overwrite = typer.confirm("Directory exists. Overwrite?", default=False)
        if not overwrite:
            console.print("[yellow]Cancelled.[/yellow]")
            raise typer.Exit(code=0)

    with console.status("[bold green]Generating your agent..."):
        agent_spec = generate_agent_spec(
            description,
            model=model,
            verbose=verbose,
            cheap=cheap,
            show_cost=show_cost,
            no_cache=no_cache,
        )

    if verbose:
        console.print("[OK] Agent specification generated successfully")

    if tokens:
        from .token_tracker import TokenTracker
        from .request_enforcer import Mode, ModeRegistry
        mode_str = ScaffConfig.get("api_mode", "medium")
        try:
            mode = Mode(mode_str)
        except ValueError:
            mode = Mode.MEDIUM
        mode_config = ModeRegistry.get_config(mode)
        tracker = TokenTracker()
        monthly = tracker.get_monthly_stats()
        remaining = max(0, mode_config.monthly_tokens - monthly['total_tokens'])
        console.print()
        console.print("[bold]Token Budget (mode: {})[/bold]".format(mode_str.upper()))
        console.print(f"  Used this month: {monthly['total_tokens']:,} / {mode_config.monthly_tokens:,}")
        console.print(f"  Remaining: {remaining:,} tokens")
        console.print(f"  Cost this month: ${monthly['total_cost']:.2f} / ${mode_config.monthly_cost:.2f}")
        console.print()

    if verbose:
        console.print("[*] Writing generated files...")

    created_files = write_generated_files(
        agent_spec, output, provider=provider,
        ui_mode=ui_mode, schedule=schedule, memory=memory,
        verbose=verbose,
    )

    if verbose:
        console.print("[OK] Files written successfully")

    return created_files


@app.command(
    short_help="Create a new AI agent from a description",
    epilog="""
    Examples:
      scaff create "summarize emails and flag urgent ones"
      scaff create "monitor github issues" -o ./github-agent -v
      scaff create "fetch weather" --cheap          # save tokens
      scaff create "chatbot" --ui streamlit         # with web UI
      scaff create "daily report" --schedule "0 9 * * *"  # daily at 9am

    See 'scaff examples' for more ideas.
    """,
)
def create(
    description: str = typer.Argument(..., help="Plain English description of the AI agent"),
    output: str = typer.Option(
        None,
        "--output",
        "-o",
        help="Output directory for generated files (default: ./agent-output)",
    ),
    model: str = typer.Option(
        None,
        "--model",
        "-m",
        help="OpenAI model to use (default: gpt-4o)",
    ),
    verbose: bool = typer.Option(
        False,
        "--verbose",
        "-v",
        help="Show detailed generation steps",
    ),
    cheap: bool = typer.Option(
        False,
        "--cheap",
        "-c",
        help="Use gpt-4o-mini to save 90% on API costs",
    ),
    show_cost: bool = typer.Option(
        False,
        "--show-cost",
        help="Print estimated token usage and cost before calling",
    ),
    no_cache: bool = typer.Option(
        False,
        "--no-cache",
        help="Bypass cache and force a fresh API call",
    ),
    tokens: bool = typer.Option(
        False,
        "--tokens",
        "-t",
        help="Show detailed token usage and remaining budget",
    ),
    provider: str = typer.Option(
        None,
        "--provider",
        "-p",
        help="AI provider: openai, anthropic, or gemini",
    ),
    ui_mode: str = typer.Option(
        None,
        "--ui",
        help="Generate web UI: fastapi or streamlit",
    ),
    schedule: str = typer.Option(
        None,
        "--schedule",
        help="Cron expression for scheduled runs (e.g. '0 * * * *' for hourly)",
    ),
    memory: str = typer.Option(
        None,
        "--memory",
        help="Conversation memory: sqlite for persistent chat history",
    ),
    archetype: str = typer.Option(
        "cli",
        "--archetype",
        help="Agent archetype: cli, web-api, chatbot, scheduler, memory",
    ),
) -> None:
    """Create a new AI agent from a plain English description.

    scaff sends your description to OpenAI, which designs the agent's
    system prompt, tools, and dependencies. The result is a complete,
    runnable project in the output directory.
    """

    if provider is None:
        provider = ScaffConfig.get("default_provider", "openai")

    if provider not in ("openai", "anthropic", "gemini", "ollama"):
        console.print(f"[bold red][ERROR] Invalid provider: {provider}[/bold red]")
        console.print("Valid providers: openai, anthropic, gemini, ollama")
        raise typer.Exit(code=1)

    if ui_mode and ui_mode not in ("fastapi", "streamlit"):
        console.print(f"[bold red][ERROR] Invalid UI mode: {ui_mode}[/bold red]")
        console.print("Valid modes: fastapi, streamlit")
        raise typer.Exit(code=1)

    if provider != "ollama":
        key_error = _check_api_key()
        if key_error:
            console.print(f"[bold red][ERROR][/bold red] {key_error}")
            raise typer.Exit(code=1)

    # Expand archetype into specific flags
    archetype_map = {
        "cli": {},
        "web-api": {"ui_mode": "fastapi"},
        "chatbot": {"ui_mode": "streamlit"},
        "scheduler": {"schedule": "0 * * * *"},
        "memory": {"memory": "sqlite"},
    }
    if archetype in archetype_map:
        arch_settings = archetype_map[archetype]
        if archetype != "cli":
            ui_mode = ui_mode or arch_settings.get("ui_mode")
            schedule = schedule or arch_settings.get("schedule")
            memory = memory or arch_settings.get("memory")
    else:
        console.print(f"[yellow]Unknown archetype: {archetype}. Using 'cli'.[/yellow]")

    # Use config defaults if not specified
    output = output or ScaffConfig.get("output_dir")
    model = model or ScaffConfig.get("model")
    cheap = cheap or ScaffConfig.get("cheap", False)
    show_cost = show_cost or ScaffConfig.get("show_cost", False)

    try:
        created_files = _run_agent(
            output, description, model, verbose,
            cheap=cheap, show_cost=show_cost, no_cache=no_cache, tokens=tokens,
            provider=provider, ui_mode=ui_mode, schedule=schedule, memory=memory,
        )

        # Display success message with tree
        console.print()
        console.print("[bold green][OK] Agent scaffolded successfully![/bold green]")
        console.print()

        tree = Tree(f"{output}")
        for file_path in sorted(created_files):
            rel_path = Path(file_path).name
            tree.add(f"{rel_path}")

        console.print(tree)
        console.print()
        console.print("[bold]Next steps:[/bold]")
        console.print(f"1. cd {output}")
        console.print("2. pip install -r requirements.txt")
        console.print("3. Set your API key in .env (or copy .env.example to .env)" if provider != "ollama" else "3. Make sure Ollama is running (ollama serve)")
        console.print("4. python agent.py")
        console.print()
        console.print("[dim]Tip: Try 'scaff run' to do all of this automatically![/dim]")
        console.print()

    except ValueError as e:
        console.print(f"[bold red][ERROR] {e}[/bold red]")
        raise typer.Exit(code=1)
    except Exception as e:
        console.print(f"[bold red][ERROR] Unexpected error: {e}[/bold red]")
        if verbose:
            console.print_exception()
        raise typer.Exit(code=1)


@app.command(
    short_help="Generate and run an agent in one command",
    epilog="""
    Examples:
      scaff run "summarize my emails"
      scaff run "monitor github issues" --cheap
      scaff run "chatbot" --ui streamlit          # with web UI
      scaff run "daily report" --schedule "0 9 * * *"

    This generates the agent, installs dependencies, and runs it
    automatically - all in one step.
    """,
)
def run(
    description: str = typer.Argument(..., help="Plain English description of the AI agent"),
    output: str = typer.Option(
        None,
        "--output",
        "-o",
        help="Output directory (default: ./scaff-output)",
    ),
    model: str = typer.Option(
        None,
        "--model",
        "-m",
        help="OpenAI model (default: gpt-4o)",
    ),
    cheap: bool = typer.Option(
        False,
        "--cheap",
        "-c",
        help="Use gpt-4o-mini to save 90% on API costs",
    ),
    show_cost: bool = typer.Option(
        False,
        "--show-cost",
        help="Print estimated token usage and cost before calling",
    ),
    no_cache: bool = typer.Option(
        False,
        "--no-cache",
        help="Bypass cache and force a fresh API call",
    ),
    verbose: bool = typer.Option(
        False,
        "--verbose",
        "-v",
        help="Show detailed generation steps",
    ),
    tokens: bool = typer.Option(
        False,
        "--tokens",
        "-t",
        help="Show detailed token usage and remaining budget",
    ),
    provider: str = typer.Option(
        None,
        "--provider",
        "-p",
        help="AI provider: openai, anthropic, or gemini",
    ),
    ui_mode: str = typer.Option(
        None,
        "--ui",
        help="Generate web UI: fastapi or streamlit",
    ),
    schedule: str = typer.Option(
        None,
        "--schedule",
        help="Cron expression for scheduled runs (e.g. '0 * * * *')",
    ),
    memory: str = typer.Option(
        None,
        "--memory",
        help="Conversation memory: sqlite for persistent history",
    ),
) -> None:
    """Generate and run an AI agent - all in one command.

    This is the quickest way to use scaff. It generates the agent,
    installs dependencies, and starts the agent automatically.
    """
    if provider is None:
        provider = ScaffConfig.get("default_provider", "openai")

    if provider not in ("openai", "anthropic", "gemini", "ollama"):
        console.print(f"[bold red][ERROR] Invalid provider: {provider}[/bold red]")
        console.print("Valid providers: openai, anthropic, gemini, ollama")
        raise typer.Exit(code=1)

    if ui_mode and ui_mode not in ("fastapi", "streamlit"):
        console.print(f"[bold red][ERROR] Invalid UI mode: {ui_mode}[/bold red]")
        console.print("Valid modes: fastapi, streamlit")
        raise typer.Exit(code=1)

    if provider != "ollama":
        key_error = _check_api_key()
        if key_error:
            console.print(f"[bold red][ERROR][/bold red] {key_error}")
            raise typer.Exit(code=1)

    output = output or ScaffConfig.get("output_dir")
    model = model or ScaffConfig.get("model")
    cheap = cheap or ScaffConfig.get("cheap", False)
    show_cost = show_cost or ScaffConfig.get("show_cost", False)

    try:
        created_files = _run_agent(
            output, description, model, verbose,
            cheap=cheap, show_cost=show_cost, no_cache=no_cache, tokens=tokens,
            provider=provider, ui_mode=ui_mode, schedule=schedule, memory=memory,
        )

        # Display generated files
        console.print()
        console.print("[bold green][OK] Agent generated![/bold green]")
        console.print()
        tree = Tree(f"{output}")
        for file_path in sorted(created_files):
            rel_path = Path(file_path).name
            tree.add(f"{rel_path}")
        console.print(tree)
        console.print()

        # Install dependencies
        req_file = Path(output) / "requirements.txt"
        if req_file.exists():
            console.print("[*] Installing dependencies...")
            result = subprocess.run(
                [sys.executable, "-m", "pip", "install", "-r", str(req_file)],
                capture_output=not verbose,
                text=True,
            )
            if result.returncode != 0:
                console.print("[yellow][!] Dependency install had issues. You may need to run:[/yellow]")
                console.print(f"  pip install -r {req_file}")
                if verbose:
                    console.print(result.stderr)
            else:
                console.print("[OK] Dependencies installed")

        console.print()
        console.print("[bold cyan]Starting your agent![/bold cyan]")
        console.print("[dim](Type your request when prompted, or press Ctrl+C to exit)[/dim]")
        console.print()

        # Run the agent
        agent_file = Path(output) / "agent.py"
        if not agent_file.exists():
            console.print("[bold red][ERROR] agent.py not found in output directory[/bold red]")
            raise typer.Exit(code=1)

        env = os.environ.copy()
        subprocess.run([sys.executable, str(agent_file)], cwd=output, env=env)

    except ValueError as e:
        console.print(f"[bold red][ERROR] {e}[/bold red]")
        raise typer.Exit(code=1)
    except typer.Exit:
        raise
    except Exception as e:
        console.print(f"[bold red][ERROR] Unexpected error: {e}[/bold red]")
        if verbose:
            console.print_exception()
        raise typer.Exit(code=1)


@app.command(
    short_help="Interactive setup wizard for beginners",
    epilog="""
    Example:
      scaff wizard

    Guides you step-by-step with prompts and explanations.
    Perfect if you are new to AI agents or the command line.
    """,
)
def wizard() -> None:
    """Guided setup wizard for beginners.

    Walks you through creating your first agent step by step.
    No experience needed - just answer the questions.
    """

    console.print()
    console.print("[bold cyan]Welcome to scaff![/bold cyan]")
    console.print()
    console.print("I will help you create your first AI agent.")
    console.print("Just answer a few questions to get started.")
    console.print()

    # Step 1: Check API key
    key_error = _check_api_key()
    if key_error:
        console.print("[bold]Step 1: Set up your OpenAI API key[/bold]")
        console.print()
        console.print(key_error)
        console.print()
        console.print("[yellow]Set your API key and run 'scaff wizard' again.[/yellow]")
        console.print("[dim]Tip: You can also create a .env file with: OPENAI_API_KEY=sk-...[/dim]")
        raise typer.Exit(code=1)

    console.print("[OK] OpenAI API key found!")
    console.print()

    # Step 2: What kind of agent?
    console.print("[bold]Step 2: What should your agent do?[/bold]")
    console.print()
    console.print("Describe what you want in plain English.")
    console.print("For example:")
    console.print('  * "summarize my emails and flag urgent ones"')
    console.print('  * "monitor github issues and auto-label them by priority"')
    console.print('  * "check the weather and send me a daily forecast"')
    console.print()

    description = typer.prompt("Describe your agent", default="", show_default=False)
    if not description.strip():
        console.print("[yellow]Let me pick a popular example for you.[/yellow]")
        description = "summarize my emails and flag urgent ones"
        console.print(f'Using: "{description}"')
        console.print()

    # Step 3: Output directory
    console.print("[bold]Step 3: Where should I save it?[/bold]")
    console.print()
    default_output = ScaffConfig.get("output_dir")
    output = typer.prompt(
        "Output directory",
        default=default_output,
        show_default=True,
    )
    console.print()

    # Step 4: Provider
    console.print("[bold]Step 4: Choose your AI provider[/bold]")
    console.print()
    console.print("  1) OpenAI (GPT-4o - default)")
    console.print("  2) Anthropic (Claude Sonnet 4)")
    console.print("  3) Google (Gemini 2.0 Flash)")
    console.print("  4) Ollama (Local - llama3.2, free, no API key needed)")
    console.print()

    provider_choice = typer.prompt("Choose (1, 2, 3, or 4)", default="1", show_default=False)
    provider_map = {"1": "openai", "2": "anthropic", "3": "gemini", "4": "ollama"}
    provider = provider_map.get(provider_choice.strip(), "openai")
    console.print(f"Using: {provider}")
    console.print()

    # Step 5: Run mode
    console.print("[bold]Step 5: How should we run it?[/bold]")
    console.print()
    console.print("  1) Generate only - save files, I will run them later")
    console.print("  2) Generate and run - do everything now")
    console.print()

    run_mode = typer.prompt("Choose (1 or 2)", default="2", show_default=False)
    console.print()

    # Execute
    try:
        with console.status("[bold green]Generating your agent..."):
            agent_spec = generate_agent_spec(description, model=ScaffConfig.get("model"))

        created_files = write_generated_files(agent_spec, output, provider=provider)

        console.print()
        console.print("[bold green][OK] Your agent is ready![/bold green]")
        console.print()

        tree = Tree(f"{output}")
        for file_path in sorted(created_files):
            rel_path = Path(file_path).name
            tree.add(f"{rel_path}")
        console.print(tree)
        console.print()

        if run_mode.strip() == "2":
            req_file = Path(output) / "requirements.txt"
            if req_file.exists():
                console.print("[*] Installing dependencies...")
                subprocess.run(
                    [sys.executable, "-m", "pip", "install", "-r", str(req_file)],
                    capture_output=True,
                )

            console.print()
            console.print("[bold cyan]Starting your agent![/bold cyan]")
            console.print("[dim](Type your request or press Ctrl+C to exit)[/dim]")
            console.print()

            agent_file = Path(output) / "agent.py"
            subprocess.run([sys.executable, str(agent_file)], cwd=output)
        else:
            console.print("[bold]Next steps:[/bold]")
            console.print(f"  1. cd {output}")
            console.print("  2. pip install -r requirements.txt")
            console.print(f"  3. python agent.py")
            console.print()
            console.print("[dim]Or just run: scaff run ...[/dim]")
            console.print()

    except ValueError as e:
        console.print(f"[bold red][ERROR] {e}[/bold red]")
        raise typer.Exit(code=1)
    except Exception as e:
        console.print(f"[bold red][ERROR] {e}[/bold red]")
        raise typer.Exit(code=1)


@app.command(
    short_help="Show ready-to-use example agents",
    epilog="""
    Examples:
      scaff examples
      scaff create "summarize emails and flag urgent ones"

    Copy any example to get started quickly - no description needed.
    """,
)
def examples() -> None:
    """Show ready-to-use example agents.

    Displays 6 pre-built agent examples you can create with a single command.
    Each example shows the description, tools, dependencies, and the exact
    'scaff create' command to run.
    """
    console.print()
    console.print("[bold cyan]Ready-to-Use Example Agents[/bold cyan]")
    console.print()
    console.print("Pick one and run the command shown:")
    console.print()
    console.print(get_example_list())


@app.command(
    short_help="Manage scaff settings",
    epilog="""
    Examples:
      scaff config show
      scaff config set --key model --value gpt-4-turbo
      scaff config set --key output_dir --value ./my-agents
      scaff config reset

    Config file: ~/.scaff/config.json
    """,
)
def config(
    action: str = typer.Argument("show", help="Action: show, set, reset"),
    key: str = typer.Option(None, "--key", "-k", help="Config key to set"),
    value: str = typer.Option(None, "--value", "-v", help="Config value to set"),
) -> None:
    """Manage scaff settings.

    ACTIONS:
    show    Display all current settings
    set     Change a setting (requires --key and --value)
    reset   Restore factory defaults

    SETTINGS:
    model              Default OpenAI model (e.g., gpt-4o, gpt-4-turbo)
    output_dir         Default output directory for new agents
    template_style     Template style (modern, minimal)
    """

    if action == "show":
        config_data = ScaffConfig.load_config()
        console.print()
        console.print("[bold cyan]Current Settings[/bold cyan]")
        console.print()

        table = Table(title="scaff Configuration")
        table.add_column("Setting", style="cyan")
        table.add_column("Value", style="green")

        for k, v in config_data.items():
            table.add_row(k, str(v))

        console.print(table)
        console.print()
        console.print(f"Config file: {ScaffConfig.CONFIG_FILE}")
        console.print()

    elif action == "set":
        if not key or not value:
            console.print("[bold red][ERROR] set requires --key and --value[/bold red]")
            raise typer.Exit(code=1)

        ScaffConfig.set(key, value)
        console.print(f"[OK] Set {key} = {value}")

    elif action == "reset":
        ScaffConfig.reset()
        console.print("[OK] Reset to defaults")

    else:
        console.print(f"[ERROR] Unknown action: {action}")
        raise typer.Exit(code=1)


@app.command(
    short_help="Manage response cache to save API tokens",
    epilog="""
    Examples:
      scaff cache status     # show cache size and entry count
      scaff cache clear      # delete all cached responses

    Cache is stored in ~/.scaff/cache/ and expires after 7 days.
    """,
)
def cache(
    action: str = typer.Argument(
        "status",
        help="Action: status or clear",
    ),
) -> None:
    """Manage the OpenAI response cache.

    scaff caches OpenAI responses so repeated descriptions don't cost
    extra tokens. Cache entries expire after 7 days.

    ACTIONS:
    status    Show cache size, entry count, and oldest entry
    clear     Delete all cached responses
    """

    if action == "status":
        stats = scache.status()
        console.print()
        console.print("[bold cyan]Response Cache[/bold cyan]")
        console.print()
        console.print(f"  Entries:    {stats['entries']}")
        console.print(f"  Size:       {_fmt_bytes(stats['size_bytes'])}")
        console.print(f"  Oldest:     {stats['oldest_hours']} hours")
        console.print()
        console.print(f"  Cache dir:  {scache.CACHE_DIR}")
        console.print()

    elif action == "clear":
        count = scache.clear()
        console.print(f"[OK] Cleared {count} cached response(s)")
        console.print()

    else:
        console.print(f"[ERROR] Unknown action: {action}")
        raise typer.Exit(code=1)


def _fmt_bytes(b: int) -> str:
    """Format byte count to human-readable size."""
    if b < 1024:
        return f"{b} B"
    if b < 1024 ** 2:
        return f"{b / 1024:.1f} KB"
    return f"{b / 1024 ** 2:.1f} MB"


@app.command(
    short_help="Test a description without creating files (dry run)",
    epilog="""
    Example:
      scaff validate "summarize emails and flag urgent ones"

    Use this to check if a description works before generating files.
    """,
)
def validate(
    description: str = typer.Argument(..., help="Agent description to test"),
) -> None:
    """Test an agent description without creating any files.

    Sends your description to OpenAI and shows what would be generated.
    A safe way to test ideas before committing to full generation.
    """

    try:
        console.print()
        console.print("[*] Testing your description...")
        console.print()

        # Try to generate spec without writing files
        agent_spec = generate_agent_spec(description, verbose=True)

        console.print()
        console.print("[bold green][OK] Description works![/bold green]")
        console.print()
        console.print(f"Agent Name: {agent_spec.get('agent_name')}")
        console.print(f"Description: {agent_spec.get('description')}")
        console.print(f"Tools: {len(agent_spec.get('tools', []))} tool(s)")
        console.print(f"Dependencies: {', '.join(agent_spec.get('dependencies', []))}")
        console.print()

    except ValueError as e:
        console.print(f"[bold red][ERROR] {e}[/bold red]")
        raise typer.Exit(code=1)


@app.command(
    short_help="Show scaff information and statistics",
    epilog="""
    For full documentation, see:
      README.md          - User guide
      FINAL_PRODUCT_DOCUMENTATION.md - Full reference
      QUICK_REFERENCE.md - Cheat sheet
    """,
)
def info() -> None:
    """Show scaff information and statistics.

    Displays version, features, technology stack, and
    configuration location. Use this to verify your
    scaff installation and learn about available features.
    """
    
    console.print()
    console.print("[bold cyan]scaff - AI Agent Project Scaffolder[/bold cyan]")
    console.print()
    console.print(f"[bold]Version:[/bold] {__version__}")
    console.print("[bold]License:[/bold] MIT")
    console.print("[bold]Repository:[/bold] https://github.com/your-repo/scaff")
    console.print()
    console.print("[bold cyan]Features:[/bold cyan]")
    console.print("  [*] scaff create - generate a complete agent project")
    console.print("  [*] scaff run - generate, install deps, and run in one command")
    console.print("  [*] scaff wizard - guided setup for beginners")
    console.print("  [*] scaff estimate - preview cost before generating")
    console.print("  [*] scaff edit - modify existing agents without regeneration")
    console.print("  [*] scaff examples - 6 pre-built agents to try")
    console.print("  [*] Web UI output (FastAPI or Streamlit --ui flag)")
    console.print("  [*] Scheduled agent runs (--schedule flag)")
    console.print("  [*] Tool pattern matching with verbose feedback")
    console.print("  [*] Streaming generation with real-time token counting")
    console.print("  [*] Token bucket rate limiter (MIN/MEDIUM/MAX modes)")
    console.print("  [*] Jittered exponential backoff with rate limit header parsing")
    console.print("  [*] Response cache to save API tokens")
    console.print("  [*] Memory leak detection (tracemalloc)")
    console.print("  [*] Multi-provider support: OpenAI, Anthropic, Gemini, Ollama (local)")
    console.print("  [*] OpenAI GPT-4o powered design")
    console.print("  [*] Self-contained agents (no heavy frameworks)")
    console.print("  [*] Cross-platform (Windows, Mac, Linux)")
    console.print()
    console.print("[bold cyan]Tech Stack:[/bold cyan]")
    console.print("  * Python 3.11+")
    console.print("  * Typer (CLI)")
    console.print("  * OpenAI SDK")
    console.print("  * Rich (UI)")
    console.print("  * Jinja2 (Templates)")
    console.print()
    console.print("[bold cyan]Configuration:[/bold cyan]")
    console.print(f"  Config file: {ScaffConfig.CONFIG_FILE}")
    console.print()


@app.command(
    short_help="Show token usage statistics",
    epilog="""
    Examples:
      scaff stats
      scaff stats --month 2024-05
    """,
)
def stats(
    month: str = typer.Option(
        None,
        "--month",
        "-m",
        help="Month to show stats for (YYYY-MM format)",
    ),
) -> None:
    """Display token usage statistics.
    
    Shows your monthly and all-time token usage, costs,
    and estimates how many agents you can still generate.
    """
    from .token_tracker import TokenTracker
    
    tracker = TokenTracker()
    
    if month:
        stats_data = tracker.get_monthly_stats(month)
        console.print()
        console.print(f"[bold]Token Usage for {month}[/bold]")
        console.print(f"  Total tokens: {stats_data['total_tokens']:,}")
        console.print(f"  Total cost: ${stats_data['total_cost']:.2f}")
        console.print(f"  Sessions: {stats_data['session_count']}")
        console.print()
    else:
        monthly = tracker.get_monthly_stats()
        all_time = tracker.get_all_time_stats()
        
        console.print()
        console.print("[bold cyan]This Month[/bold cyan]")
        console.print(f"  Tokens: {monthly['total_tokens']:,}")
        console.print(f"  Cost: ${monthly['total_cost']:.2f}")
        console.print(f"  Sessions: {monthly['session_count']}")
        # Monthly projection (based on current rate)
        day_of_month = datetime.now().day
        if day_of_month > 0 and monthly['total_cost'] > 0:
            projected = (monthly['total_cost'] / day_of_month) * 30
            console.print(f"  [dim]Projected monthly: ${projected:.2f} (based on current rate)[/dim]")
        console.print()
        
        console.print("[bold cyan]All-Time[/bold cyan]")
        console.print(f"  Total tokens: {all_time['total_tokens']:,}")
        console.print(f"  Total cost: ${all_time['total_cost']:.2f}")
        console.print(f"  Sessions: {all_time['total_sessions']}")
        console.print(f"  Avg cost/session: ${all_time['average_cost_per_session']:.4f}")
        console.print()


@app.command(
    short_help="Set API limiting mode",
    epilog="""
    Examples:
      scaff mode min      # Cost-optimized
      scaff mode medium   # Balanced (default)
      scaff mode max      # Quality-optimized
    """,
)
def mode(
    mode_name: str = typer.Argument("medium", help="Mode: min, medium, or max"),
) -> None:
    """Set the API limiting mode.
    
    MIN:    $0.25/month (1M tokens) - Cost optimized
    MEDIUM: $1.25/month (5M tokens) - Balanced (default)
    MAX:    $5.00/month (20M tokens) - Quality optimized
    """
    from .request_enforcer import Mode
    from .display import Display
    
    try:
        mode_enum = Mode(mode_name)
        ScaffConfig.set("api_mode", mode_name)
        
        console.print()
        console.print(f"[green]OK Mode set to: {mode_name.upper()}[/green]")
        console.print()
        Display.show_mode_info(mode_name)
        console.print()
        
    except ValueError:
        console.print(f"[red]Invalid mode: {mode_name}[/red]")
        console.print("Valid modes: min, medium, max")
        raise typer.Exit(code=1)


@app.command(
    short_help="View analytics and performance metrics",
    epilog="""
    Examples:
      scaff analytics              # Show daily stats
      scaff analytics --period weekly
      scaff analytics --model gpt-4o
    """,
)
def analytics(
    period: str = typer.Option(
        "daily",
        "--period",
        "-p",
        help="Time period: daily, weekly, monthly",
    ),
    model: str = typer.Option(
        None,
        "--model",
        "-m",
        help="Filter by model (e.g., gpt-4o)",
    ),
) -> None:
    """Display analytics and performance metrics.
    
    Shows API usage, costs, latency, and success rates.
    """
    from .analytics import Analytics
    
    analytics_obj = Analytics()
    
    console.print()
    console.print("[bold cyan]Analytics & Performance[/bold cyan]")
    console.print()
    
    if period == "daily":
        stats = analytics_obj.get_daily_stats()
        console.print("[bold]Today[/bold]")
        console.print(f"  API calls: {stats['api_calls']}")
        console.print(f"  Successful: {stats['successful_calls']}")
        console.print(f"  Failed: {stats['failed_calls']}")
        console.print(f"  Total tokens: {stats['total_tokens']:,}")
        console.print(f"  Total cost: ${stats['total_cost_usd']:.4f}")
        console.print(f"  Avg latency: {stats['avg_latency_ms']:.0f}ms")
        console.print()
    
    elif period == "weekly":
        stats = analytics_obj.get_daily_stats()
        console.print("[bold]This Week[/bold]")
        console.print(f"  API calls: {stats['api_calls']}")
        console.print(f"  Total cost: ${stats['total_cost_usd']:.4f}")
        console.print(f"  Avg latency: {stats['avg_latency_ms']:.0f}ms")
        console.print()
    
    elif period == "monthly":
        stats = analytics_obj.get_monthly_stats()
        console.print("[bold]This Month[/bold]")
        console.print(f"  API calls: {stats['api_calls']}")
        console.print(f"  Successful: {stats['successful_calls']}")
        console.print(f"  Failed: {stats['failed_calls']}")
        console.print(f"  Total tokens: {stats['total_tokens']:,}")
        console.print(f"  Total cost: ${stats['total_cost_usd']:.4f}")
        console.print(f"  Avg cost/call: ${stats['avg_cost_per_call']:.4f}")
        console.print()
    
    # Model breakdown
    model_stats = analytics_obj.get_model_stats()
    if model_stats:
        console.print("[bold]By Model[/bold]")
        for m, stats in model_stats.items():
            if model is None or m == model:
                console.print(f"  {m}")
                console.print(f"    Calls: {stats['calls']}")
                console.print(f"    Tokens: {stats['tokens']:,}")
                console.print(f"    Cost: ${stats['cost']:.4f}")
                console.print(f"    Avg latency: {stats['avg_latency_ms']:.0f}ms")
        console.print()


@app.command(
    short_help="Preview cost across all modes before generating",
    epilog="""
    Examples:
      scaff estimate "summarize emails and flag urgent ones"
      scaff estimate "monitor github issues" --verbose

    Shows estimated cost and token usage for MIN, MEDIUM, and MAX modes
    without making any API calls.
    """,
)
def estimate(
    description: str = typer.Argument(..., help="Agent description to estimate cost for"),
    verbose: bool = typer.Option(
        False,
        "--verbose",
        "-v",
        help="Show detailed token breakdown per mode",
    ),
    mode_filter: str = typer.Option(
        None,
        "--mode",
        "-m",
        help="Filter by mode: min, medium, max (default: all)",
    ),
) -> None:
    """Preview estimated cost across API limiting modes.

    Calculates token counts and costs for MIN, MEDIUM, and MAX modes
    without making any API calls. Use this to decide which mode fits
    your budget before generating.
    """
    from .request_enforcer import Mode, ModeRegistry
    from .token_counter import TokenCounter
    from .pricing import PricingCalculator
    from .generator import _build_system_prompt

    counter = TokenCounter()
    description_tokens = counter.count_tokens(description)

    # Account for system prompt + user message framing overhead
    system_prompt = _build_system_prompt(strict_json=False)
    user_message = f"Create an AI agent with the following description:\n\n{description}"
    input_text = system_prompt + "\n" + user_message
    full_input_tokens = counter.count_tokens(input_text)

    console.print()
    console.print("[bold cyan]Cost Estimate[/bold cyan]")
    console.print()
    console.print(f"  Description: {description}")
    console.print(f"  Full prompt: ~{full_input_tokens:,} tokens (incl. system prompt)")
    console.print()

    if mode_filter:
        try:
            modes = [Mode(mode_filter)]
        except ValueError:
            console.print(f"[red]Invalid mode: {mode_filter}[/red]")
            console.print("Valid modes: min, medium, max")
            raise typer.Exit(code=1)
    else:
        modes = [Mode.MIN, Mode.MEDIUM, Mode.MAX]

    table = Table(title="Estimated Cost by Mode")
    table.add_column("Mode", style="cyan")
    table.add_column("Model", style="green")
    table.add_column("Temperature", style="yellow")
    table.add_column("Max Tokens", style="yellow")
    table.add_column("Input Cost", style="green")
    table.add_column("Est. Total Cost", style="bold green")
    table.add_column("Monthly Budget", style="blue")

    for mode in modes:
        config = ModeRegistry.get_config(mode)
        avg_output = config.max_tokens // 2

        input_cost = PricingCalculator.calculate_request_cost(
            config.model, full_input_tokens, 0
        )
        total_cost = PricingCalculator.calculate_request_cost(
            config.model, full_input_tokens, avg_output
        )

        table.add_row(
            mode.value.upper(),
            config.model,
            f"{config.temperature:.1f}",
            f"{config.max_tokens:,}",
            f"${input_cost:.5f}",
            f"${total_cost:.5f}",
            f"${config.monthly_cost:.2f} / {config.monthly_tokens:,} tok",
        )

        if verbose:
            console.print(f"  [{mode.value.upper()}] ~{avg_output:,} output tokens (est.)")
            console.print(f"  [{mode.value.upper()}] Rate: {config.rate_limit_per_minute} req/min, {config.concurrent_limit} concurrent")
            console.print()

    console.print(table)
    console.print()
    console.print("[dim]Estimates based on ~4 chars/token. Actual costs may vary.[/dim]")
    console.print()


@app.command(
    short_help="View error reports and diagnostics",
    epilog="""
    Examples:
      scaff errors           # Show error summary
      scaff errors --recent  # Show recent errors
    """,
)
def errors(
    recent: bool = typer.Option(
        False,
        "--recent",
        "-r",
        help="Show recent errors (last 10)",
    ),
) -> None:
    """Display error reports and diagnostics.
    
    Shows error summaries, trends, and system information.
    """
    from .telemetry import Telemetry
    
    telemetry = Telemetry()
    
    console.print()
    console.print("[bold cyan]Error Reports & Diagnostics[/bold cyan]")
    console.print()
    
    summary = telemetry.get_error_summary()
    console.print("[bold]Error Summary[/bold]")
    console.print(f"  Total errors: {summary['total_errors']}")
    
    if summary['error_types']:
        console.print(f"  Most common: {summary['most_common']}")
        console.print(f"  Error types: {len(summary['error_types'])}")
        console.print()
        
        console.print("[bold]Error Breakdown[/bold]")
        for error_type, count in summary['error_types'].items():
            console.print(f"  {error_type}: {count}")
    
    console.print()
    
    sys_info = telemetry.get_system_info()
    console.print("[bold]System Information[/bold]")
    console.print(f"  Platform: {sys_info['platform']} {sys_info['platform_release']}")
    console.print(f"  Python: {sys_info['python_version']}")
    console.print(f"  Machine: {sys_info['machine']}")
    console.print()


@app.command(
    short_help="Show application state and configuration",
    epilog="""
    Examples:
      scaff state              # Show current state
      scaff state --reset      # Reset to defaults
    """,
)
def state(
    reset: bool = typer.Option(
        False,
        "--reset",
        help="Reset application state to defaults",
    ),
) -> None:
    """Display or manage application state.
    
    Shows current configuration, counters, and settings.
    """
    from .state_manager import StateManager
    
    state_mgr = StateManager()
    
    if reset:
        state_mgr.reset()
        console.print("[green]OK Application state reset to defaults[/green]")
        console.print()
        return
    
    console.print()
    console.print("[bold cyan]Application State[/bold cyan]")
    console.print()
    
    all_state = state_mgr.get_all()
    console.print("[bold]Configuration[/bold]")
    console.print(f"  Current mode: {all_state.get('current_mode', 'unknown')}")
    console.print(f"  Cache enabled: {all_state.get('cache_enabled', False)}")
    console.print(f"  Verbose mode: {all_state.get('verbose_mode', False)}")
    console.print()
    
    console.print("[bold]Counters[/bold]")
    console.print(f"  Agents generated: {all_state.get('total_agents_generated', 0)}")
    console.print(f"  API calls made: {all_state.get('total_api_calls', 0)}")
    console.print()
    
    if all_state.get('last_session_id'):
        console.print("[bold]Last Session[/bold]")
        console.print(f"  Session ID: {all_state['last_session_id']}")
        console.print(f"  Last update: {all_state['last_update']}")
        console.print()


@app.command(
    short_help="Modify an existing generated agent",
    epilog="""
    Examples:
      scaff edit --dir ./my-agent --description "New description"
      scaff edit --dir ./my-agent --provider anthropic
      scaff edit --dir ./my-agent --ui fastapi
      scaff edit --dir ./my-agent --system-prompt "Be concise."

    Reads spec.json from the agent directory, applies changes,
    and regenerates all files in place.
    """,
)
def edit(
    directory: str = typer.Option(
        ".",
        "--dir",
        "-d",
        help="Agent directory containing spec.json",
    ),
    description: str = typer.Option(
        None,
        "--description",
        "-desc",
        help="New agent description",
    ),
    system_prompt: str = typer.Option(
        None,
        "--system-prompt",
        "-sp",
        help="New system prompt for the agent",
    ),
    provider: str = typer.Option(
        None,
        "--provider",
        "-p",
        help="Change AI provider: openai, anthropic, gemini",
    ),
    ui_mode: str = typer.Option(
        None,
        "--ui",
        help="Change web UI mode: fastapi, streamlit, or none",
    ),
    schedule: str = typer.Option(
        None,
        "--schedule",
        help="Change cron schedule expression",
    ),
    verbose: bool = typer.Option(
        False,
        "--verbose",
        "-v",
        help="Show detailed regeneration steps",
    ),
) -> None:
    """Modify an existing generated agent without recreating from scratch.

    Reads the agent's spec.json, applies your changes, and regenerates
    all files in place. This is faster than running scaff create again
    because it skips the OpenAI API call.
    """
    agent_dir = Path(directory)
    spec_file = agent_dir / "spec.json"

    if not spec_file.exists():
        console.print(f"[bold red][ERROR] No spec.json found in '{directory}'[/bold red]")
        console.print("This doesn't appear to be a scaff-generated agent directory.")
        console.print("Run 'scaff create' first to generate an agent.")
        raise typer.Exit(code=1)

    try:
        with open(spec_file, "r", encoding="utf-8") as f:
            spec = json.load(f)
    except (json.JSONDecodeError, IOError) as e:
        console.print(f"[bold red][ERROR] Failed to read spec.json: {e}[/bold red]")
        raise typer.Exit(code=1)

    # Track what changed
    changes = []

    if description:
        old_desc = spec.get("description", "")
        spec["description"] = description
        changes.append(f"description: '{old_desc}' -> '{description}'")

    if system_prompt:
        spec["system_prompt"] = system_prompt
        changes.append("system_prompt updated")

    if provider:
        if provider not in ("openai", "anthropic", "gemini", "ollama"):
            console.print(f"[bold red][ERROR] Invalid provider: {provider}[/bold red]")
            console.print("Valid: openai, anthropic, gemini, ollama")
            raise typer.Exit(code=1)
        spec["provider"] = provider
        changes.append(f"provider -> {provider}")

    if ui_mode is not None:
        if ui_mode not in ("fastapi", "streamlit", "none"):
            console.print(f"[bold red][ERROR] Invalid UI mode: {ui_mode}[/bold red]")
            console.print("Valid: fastapi, streamlit, none")
            raise typer.Exit(code=1)
        spec["ui_mode"] = None if ui_mode == "none" else ui_mode
        changes.append(f"ui_mode -> {ui_mode}")

    if schedule is not None:
        spec["schedule"] = None if schedule == "none" else schedule
        changes.append(f"schedule -> {schedule}")

    if not changes:
        console.print("[yellow]No changes specified. Use --description, --system-prompt, etc.[/yellow]")
        raise typer.Exit(code=0)

    if verbose:
        console.print("[*] Applying changes:")
        for c in changes:
            console.print(f"    - {c}")

    try:
        created_files = write_generated_files(
            spec,
            str(agent_dir),
            provider=spec.get("provider", provider or "openai"),
            ui_mode=spec.get("ui_mode", ui_mode),
            schedule=spec.get("schedule", schedule),
            verbose=verbose,
        )

        console.print()
        console.print("[bold green][OK] Agent updated successfully![/bold green]")
        console.print()
        console.print(f"  {len(changes)} change(s) applied.")
        console.print(f"  {len(created_files)} file(s) regenerated.")
        console.print()

    except Exception as e:
        console.print(f"[bold red][ERROR] Failed to regenerate: {e}[/bold red]")
        if verbose:
            console.print_exception()
        raise typer.Exit(code=1)


@app.command(
    short_help="Generate deployment configs for your agent",
    epilog="""
    Examples:
      scaff deploy --dir ./my-agent           # generate all configs
      scaff deploy --dir ./my-agent --platform docker
      scaff deploy --dir ./my-agent --platform railway
      scaff deploy --dir ./my-agent --platform render

    Generates Dockerfile, Procfile, railway.json, and render.yaml
    for one-command deployment to your preferred platform.
    """,
)
def deploy(
    directory: str = typer.Option(
        ".",
        "--dir",
        "-d",
        help="Agent directory containing spec.json",
    ),
    platform: str = typer.Option(
        None,
        "--platform",
        "-p",
        help="Platform: docker, railway, render, heroku (default: all)",
    ),
    verbose: bool = typer.Option(
        False,
        "--verbose",
        "-v",
        help="Show detailed output",
    ),
) -> None:
    """Generate deployment configuration files for your agent.

    Reads the agent's spec.json and generates Dockerfile, Procfile,
    railway.json, and/or render.yaml depending on your chosen platform.
    """
    from jinja2 import Environment, PackageLoader, select_autoescape

    agent_dir = Path(directory)
    spec_file = agent_dir / "spec.json"

    if not spec_file.exists():
        console.print(f"[bold red][ERROR] No spec.json found in '{directory}'[/bold red]")
        console.print("Generate an agent first with 'scaff create'.")
        raise typer.Exit(code=1)

    try:
        with open(spec_file, "r", encoding="utf-8") as f:
            spec = json.loads(f.read())
    except (json.JSONDecodeError, IOError) as e:
        console.print(f"[bold red][ERROR] Failed to read spec.json: {e}[/bold red]")
        raise typer.Exit(code=1)

    # Read MANIFEST for metadata
    manifest_file = agent_dir / "MANIFEST.json"
    manifest = {}
    if manifest_file.exists():
        try:
            manifest = json.loads(manifest_file.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, IOError):
            pass

    agent_name = spec.get("agent_name", manifest.get("agent_name", "agent"))
    ui_mode = manifest.get("ui_mode")
    context = {
        "agent_name": agent_name,
        "description": spec.get("description", ""),
        "provider": manifest.get("provider", "openai"),
        "ui_mode": ui_mode,
    }

    env = Environment(
        loader=PackageLoader("scaff", "templates"),
        autoescape=select_autoescape(),
    )

    platforms = ["docker", "railway", "render", "heroku"] if not platform else [platform]
    generated = []

    for plat in platforms:
        template_map = {
            "docker": ("Dockerfile", "Dockerfile.j2"),
            "railway": ("railway.json", "railway.json.j2"),
            "render": ("render.yaml", "render.yaml.j2"),
            "heroku": ("Procfile", "Procfile.j2"),
        }
        if plat not in template_map:
            console.print(f"[yellow]Unknown platform: {plat}[/yellow]")
            continue

        filename, template_name = template_map[plat]
        try:
            template = env.get_template(template_name)
            content = template.render(context)
            output_file = agent_dir / filename
            output_file.write_text(content, encoding="utf-8")
            generated.append(filename)
            if verbose:
                console.print(f"  [*] Created {filename}")
        except Exception as e:
            console.print(f"[red]  [ERROR] Failed to generate {filename}: {e}[/red]")

    if generated:
        console.print()
        console.print("[bold green][OK] Deployment configs generated![/bold green]")
        console.print()
        for f in generated:
            console.print(f"  - {f}")
        console.print()
        console.print("[dim]To deploy:[/dim]")
        if "Dockerfile" in generated:
            console.print("  docker build -t my-agent . && docker run my-agent")
        if "railway.json" in generated:
            console.print("  railway up")
        if "render.yaml" in generated:
            console.print("  Push to GitHub and connect to Render")
        if "Procfile" in generated:
            console.print("  heroku create && git push heroku main")
        console.print()
    else:
        console.print("[yellow]No configs generated. Check platform name.[/yellow]")


@app.command(
    short_help="Run agent against test prompts",
    epilog="""
    Examples:
      scaff test --dir ./my-agent "What can you do?"
      scaff test --dir ./my-agent --prompts prompts.json
      scaff test --dir ./my-agent --prompts prompts.txt --verbose

    Test prompts file can be JSON (list of strings) or TXT (one per line).
    """,
)
def test(
    prompt: list[str] = typer.Argument(
        None,
        help="Test prompts (one or more)",
    ),
    directory: str = typer.Option(
        ".",
        "--dir",
        "-d",
        help="Agent directory containing agent.py",
    ),
    prompts_file: str = typer.Option(
        None,
        "--prompts",
        "-f",
        help="File containing test prompts (JSON list or TXT, one per line)",
    ),
    verbose: bool = typer.Option(
        False,
        "--verbose",
        "-v",
        help="Show full responses",
    ),
) -> None:
    """Run the agent against test prompts and report results.

    Tests your agent by feeding it prompts and checking that it responds
    without errors. Useful for validating agents after regeneration or edits.
    """
    import importlib.util
    import sys as _sys
    from pathlib import Path as _Path

    agent_dir = Path(directory)
    agent_file = agent_dir / "agent.py"

    if not agent_file.exists():
        console.print(f"[bold red][ERROR] No agent.py found in '{directory}'[/bold red]")
        raise typer.Exit(code=1)

    # Collect prompts
    prompts: list[str] = []

    if prompts_file:
        pf = _Path(prompts_file)
        if not pf.exists():
            console.print(f"[bold red][ERROR] Prompts file not found: {prompts_file}[/bold red]")
            raise typer.Exit(code=1)
        try:
            content = pf.read_text(encoding="utf-8").strip()
            if content.startswith("["):
                prompts = json.loads(content)
            else:
                prompts = [line.strip() for line in content.split("\n") if line.strip()]
        except Exception as e:
            console.print(f"[bold red][ERROR] Failed to read prompts file: {e}[/bold red]")
            raise typer.Exit(code=1)

    if prompt:
        prompts.extend(prompt)

    if not prompts:
        console.print("[yellow]No prompts provided. Pass prompts as arguments or use --prompts.[/yellow]")
        raise typer.Exit(code=0)

    # Import the agent module
    _sys.path.insert(0, str(agent_dir))
    try:
        module_spec = importlib.util.spec_from_file_location("test_agent", str(agent_file))
        if not module_spec or not module_spec.loader:
            console.print("[bold red][ERROR] Failed to load agent.py[/bold red]")
            raise typer.Exit(code=1)
        agent_module = importlib.util.module_from_spec(module_spec)
        module_spec.loader.exec_module(agent_module)
    finally:
        _sys.path.remove(str(agent_dir))

    if not hasattr(agent_module, "chat"):
        console.print("[yellow]Agent module has no 'chat' function. Using run_agent is not supported for testing.[/yellow]")
        console.print("  The 'chat()' function is needed for programmatic testing.")
        console.print("  Regenerate with: scaff edit --dir {} --description '...'".format(directory))
        raise typer.Exit(code=1)

    console.print()
    console.print("[bold cyan]Agent Test Results[/bold cyan]")
    console.print(f"  Agent: {agent_dir.name}")
    console.print(f"  Prompts: {len(prompts)}")
    console.print("-" * 55)
    console.print()

    passed = 0
    failed = 0

    for i, p in enumerate(prompts, 1):
        try:
            with console.status(f"[dim]Testing prompt {i}/{len(prompts)}...[/dim]"):
                response = agent_module.chat(p)

            if response and not response.startswith('{"error"'):
                passed += 1
                status = "[green]PASS[/green]"
            else:
                failed += 1
                status = "[red]FAIL[/red]"

            console.print(f"  {status} [{i}/{len(prompts)}] {p[:60]}{'...' if len(p) > 60 else ''}")

            if verbose:
                preview = response[:300].replace("\n", " ")
                console.print(f"       Response: {preview}{'...' if len(response) > 300 else ''}")
                console.print()

        except Exception as e:
            failed += 1
            console.print(f"  [red]FAIL[/red] [{i}/{len(prompts)}] {p[:60]}{'...' if len(p) > 60 else ''}")
            if verbose:
                console.print(f"       Error: {e}")
                console.print()

    console.print("-" * 55)
    console.print(f"  [bold]Results:[/bold] {passed} passed, {failed} failed, {len(prompts)} total")
    console.print()

    if failed > 0:
        raise typer.Exit(code=1)


@app.command(
    short_help="Regenerate agent, preserving user tool changes",
    epilog="""
    Examples:
      scaff upgrade --dir ./my-agent
      scaff upgrade --dir ./my-agent --provider anthropic
      scaff upgrade --dir ./my-agent --dry-run

    Reads spec.json and current tools.py, detects user modifications,
    preserves them during regeneration. Shows diff of changes.
    """,
)
def upgrade(
    directory: str = typer.Option(
        ".",
        "--dir",
        "-d",
        help="Agent directory containing spec.json",
    ),
    provider: str = typer.Option(
        None,
        "--provider",
        "-p",
        help="Change provider (optional)",
    ),
    dry_run: bool = typer.Option(
        False,
        "--dry-run",
        help="Show what would change without writing",
    ),
    verbose: bool = typer.Option(
        False,
        "--verbose",
        "-v",
        help="Show detailed diff output",
    ),
) -> None:
    """Regenerate an agent while preserving your custom tool implementations.

    Upgrades an existing generated agent to the latest scaff templates.
    Detects which tools you've customized and preserves those implementations,
    only regenerating tools that match the original auto-generated code.
    """
    agent_dir = Path(directory)
    spec_file = agent_dir / "spec.json"
    tools_file = agent_dir / "tools.py"
    manifest_file = agent_dir / "MANIFEST.json"

    if not spec_file.exists():
        console.print(f"[bold red][ERROR] No spec.json found in '{directory}'[/bold red]")
        console.print("This doesn't appear to be a scaff-generated agent.")
        raise typer.Exit(code=1)

    # Read existing spec and manifest
    try:
        with open(spec_file, "r", encoding="utf-8") as f:
            spec = json.loads(f.read())
    except (json.JSONDecodeError, IOError) as e:
        console.print(f"[bold red][ERROR] Failed to read spec.json: {e}[/bold red]")
        raise typer.Exit(code=1)

    manifest = {}
    if manifest_file.exists():
        try:
            manifest = json.loads(manifest_file.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, IOError):
            pass

    # Override provider if specified
    if provider:
        if provider not in ("openai", "anthropic", "gemini", "ollama"):
            console.print(f"[bold red][ERROR] Invalid provider: {provider}[/bold red]")
            raise typer.Exit(code=1)
        spec["provider"] = provider

    current_provider = manifest.get("provider", "openai")
    new_provider = provider or current_provider

    # Read current tools.py to detect user modifications
    original_impls: dict[str, str] = {}
    modified_tools: dict[str, str] = {}

    if tools_file.exists():
        tools_code = tools_file.read_text(encoding="utf-8")
        # Extract function bodies for each tool
        for tool_spec in spec.get("tools", []):
            tname = tool_spec.get("name", "")
            if not tname:
                continue
            # Find the function in existing code
            import re
            pattern = re.compile(
                rf"^def {tname}\(.*?\) -> str:\n(.*?)(?=\n\ndef |\Z)",
                re.DOTALL | re.MULTILINE,
            )
            match = pattern.search(tools_code)
            if match:
                original_impls[tname] = match.group(1).strip()

    # Generate new tool implementations
    from scaff.writer import _generate_tool_implementations

    new_impls, pattern_map = _generate_tool_implementations(spec.get("tools", []))

    # Compare and detect modifications
    preserved = []
    regenerated = []
    conflicts = []

    for idx, tool_spec in enumerate(spec.get("tools", [])):
        tname = tool_spec.get("name", "")
        if not tname:
            continue

        new_code = new_impls[idx]
        new_body = "\n".join(new_code.split("\n")[1:]) if new_code else ""

        if tname in original_impls:
            old_body = original_impls[tname]
            # Strip comments for comparison
            old_stripped = "\n".join(
                line for line in old_body.split("\n")
                if not line.strip().startswith("#")
            )
            new_stripped = "\n".join(
                line for line in new_body.split("\n")
                if not line.strip().startswith("#")
            )
            if old_stripped.strip() != new_stripped.strip():
                modified_tools[tname] = original_impls[tname]
                preserved.append(tname)
            else:
                regenerated.append(tname)
        else:
            regenerated.append(tname)

    # Report
    console.print()
    console.print("[bold cyan]Upgrade Report[/bold cyan]")
    console.print(f"  Directory: {directory}")
    console.print(f"  Provider: {current_provider} -> {new_provider}")
    console.print(f"  Tools: {len(spec.get('tools', []))} total")
    console.print()

    if preserved:
        console.print("  [green]Preserved (user-modified):[/green]")
        for t in preserved:
            console.print(f"    - {t}")
        console.print()

    if regenerated:
        console.print("  [yellow]Regenerated (auto):[/yellow]")
        for t in regenerated:
            pat = pattern_map.get(t, "default")
            console.print(f"    - {t} ({pat})")
        console.print()

    if not preserved and not regenerated:
        console.print("  No tools to upgrade.")
        console.print()

    if not preserved and regenerated:
        console.print("  [green]All tools will be regenerated (none were modified).[/green]")
        console.print()

    if dry_run:
        console.print("[yellow]Dry run - no files written.[/yellow]")
        console.print()
        return

    # Write preserved implementations back into spec
    for tool_spec in spec.get("tools", []):
        tname = tool_spec.get("name", "")
        if tname in modified_tools:
            # Use the original user code instead of regenerated
            tool_spec["_implementation_code"] = modified_tools[tname]

    # Regenerate
    try:
        from scaff.writer import write_generated_files

        created_files = write_generated_files(
            spec,
            str(agent_dir),
            provider=new_provider,
            ui_mode=manifest.get("ui_mode"),
            schedule=manifest.get("schedule"),
            verbose=verbose,
        )

        console.print()
        console.print("[bold green][OK] Agent upgraded successfully![/bold green]")
        console.print(f"  {len(created_files)} file(s) written.")
        console.print()

    except Exception as e:
        console.print(f"[bold red][ERROR] Upgrade failed: {e}[/bold red]")
        if verbose:
            console.print_exception()
        raise typer.Exit(code=1)


@app.command(
    short_help="Real-time usage monitoring dashboard",
    epilog="""
    Examples:
      scaff monitor              # Live dashboard
      scaff monitor --refresh 5  # Update every 5 seconds
      scaff monitor --once       # Single snapshot

    Shows rate limiter state, token usage, memory, and cache stats.
    """,
)
def monitor(
    refresh: int = typer.Option(
        2,
        "--refresh",
        "-r",
        help="Refresh interval in seconds",
    ),
    once: bool = typer.Option(
        False,
        "--once",
        help="Show a single snapshot and exit",
    ),
) -> None:
    """Real-time monitoring dashboard for scaff.

    Displays live-updating stats about API usage, rate limits,
    memory consumption, and cache status using a Rich dashboard.
    """
    import time as _time
    from datetime import datetime as _dt
    from rich.live import Live
    from rich.layout import Layout
    from rich.panel import Panel
    from rich.table import Table
    from rich.text import Text
    from rich import box as _box

    from .token_tracker import TokenTracker
    from .request_enforcer import Mode, ModeRegistry
    from .session_manager import MemoryMonitor
    from . import cache as scache
    from .config import ScaffConfig

    tracker = TokenTracker()
    mm = MemoryMonitor()

    def _build_dashboard() -> Layout:
        layout = Layout()
        layout.split_column(
            Layout(name="header", size=3),
            Layout(name="body"),
            Layout(name="footer", size=3),
        )
        layout["body"].split_row(
            Layout(name="left"),
            Layout(name="right"),
        )
        layout["left"].split_column(
            Layout(name="rate_limiter"),
            Layout(name="session"),
        )
        layout["right"].split_column(
            Layout(name="usage"),
            Layout(name="system"),
        )

        mode_str = ScaffConfig.get("api_mode", "medium")
        now = _dt.now().strftime("%Y-%m-%d %H:%M:%S")

        # Header
        header = Panel(
            Text(f"scaff Monitor - {now}", style="bold cyan"),
            box=_box.ROUNDED,
        )
        layout["header"].update(header)

        # Rate limiter panel
        try:
            mode = Mode(mode_str)
            config = ModeRegistry.get_config(mode)
            bucket = TokenBucket(config.burst_capacity, config.refill_rate)
            utilization = bucket.utilization * 100
            rt = Table(box=_box.SIMPLE)
            rt.add_column("Metric", style="cyan")
            rt.add_column("Value", style="green")
            rt.add_row("Mode", mode_str.upper())
            rt.add_row("Model", config.model)
            rt.add_row("Burst Cap", str(config.burst_capacity))
            rt.add_row("Refill Rate", f"{config.refill_rate}/s")
            rt.add_row("Utilization", f"{utilization:.1f}%")
            rt.add_row("Est. Wait", f"{bucket.estimated_wait():.2f}s")
            layout["rate_limiter"].update(Panel(rt, title="Rate Limiter", box=_box.ROUNDED))
        except Exception:
            layout["rate_limiter"].update(Panel("N/A", title="Rate Limiter", box=_box.ROUNDED))

        # Session panel
        try:
            session = mm.get_session_stats() if hasattr(mm, "get_session_stats") else {}
            st = Table(box=_box.SIMPLE)
            st.add_column("Metric", style="cyan")
            st.add_column("Value", style="green")
            st.add_row("Memory (MB)", f"{session.get('memory_mb', 0):.1f}")
            st.add_row("Peak (MB)", f"{session.get('peak_memory_mb', 0):.1f}")
            st.add_row("Duration (s)", str(session.get("duration_seconds", 0)))
            layout["session"].update(Panel(st, title="Session", box=_box.ROUNDED))
        except Exception:
            layout["session"].update(Panel("No active session", title="Session", box=_box.ROUNDED))

        # Usage panel
        try:
            monthly = tracker.get_monthly_stats()
            all_time = tracker.get_all_time_stats()
            ut = Table(box=_box.SIMPLE)
            ut.add_column("Metric", style="cyan")
            ut.add_column("This Month", style="green")
            ut.add_column("All-Time", style="blue")
            ut.add_row(
                "Tokens",
                f"{monthly['total_tokens']:,}",
                f"{all_time['total_tokens']:,}",
            )
            ut.add_row(
                "Cost",
                f"${monthly['total_cost']:.2f}",
                f"${all_time['total_cost']:.2f}",
            )
            ut.add_row(
                "Sessions",
                str(monthly["session_count"]),
                str(all_time["total_sessions"]),
            )
            if monthly["total_cost"] > 0:
                day = max(_dt.now().day, 1)
                projected = (monthly["total_cost"] / day) * 30
                ut.add_row("Projected", f"${projected:.2f}", "")
            layout["usage"].update(Panel(ut, title="Token Usage", box=_box.ROUNDED))
        except Exception:
            layout["usage"].update(Panel("N/A", title="Token Usage", box=_box.ROUNDED))

        # System panel
        try:
            import psutil
            p = psutil.Process()
            syst = Table(box=_box.SIMPLE)
            syst.add_column("Metric", style="cyan")
            syst.add_column("Value", style="green")
            syst.add_row("CPU", f"{p.cpu_percent(interval=0)}%")
            syst.add_row("Memory", f"{p.memory_info().rss / 1024 / 1024:.1f} MB")
            syst.add_row("Threads", str(p.num_threads()))
            syst.add_row("Open FDs", str(p.num_fds() if hasattr(p, "num_fds") else "N/A"))
            ct = scache.status()
            syst.add_row("Cache Files", str(ct["entries"]))
            syst.add_row("Cache Size", f"{ct['size_bytes'] / 1024:.1f} KB")
            layout["system"].update(Panel(syst, title="System", box=_box.ROUNDED))
        except Exception:
            layout["system"].update(Panel("N/A", title="System", box=_box.ROUNDED))

        # Footer
        footer_text = f"Press Ctrl+C to exit | Refreshing every {refresh}s"
        layout["footer"].update(Panel(Text(footer_text, style="dim"), box=_box.ROUNDED))

        return layout

    try:
        if once:
            console.print(_build_dashboard())
        else:
            with Live(_build_dashboard(), refresh_per_second=1 / refresh, screen=True):
                while True:
                    _time.sleep(refresh)
    except KeyboardInterrupt:
        console.print()
        console.print("[OK] Monitor stopped.")


def main() -> None:
    """Entry point for the scaff CLI."""
    app()


if __name__ == "__main__":
    main()
