"""Typer CLI entry point for scaff."""

import os
import subprocess
import sys
from pathlib import Path

import typer
from rich.console import Console
from rich.table import Table
from rich.tree import Tree

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
) -> list[str]:
    """Generate agent and return created files list."""
    if Path(output).exists():
        overwrite = typer.confirm("Directory exists. Overwrite?", default=False)
        if not overwrite:
            console.print("[yellow]Cancelled.[/yellow]")
            raise typer.Exit(code=0)

    with console.status("[bold green]Generating your agent..."):
        agent_spec = generate_agent_spec(description, model=model, verbose=verbose)

    if verbose:
        console.print("[OK] Agent specification generated successfully")

    if verbose:
        console.print("[*] Writing generated files...")

    created_files = write_generated_files(agent_spec, output)

    if verbose:
        console.print("[OK] Files written successfully")

    return created_files


@app.command(
    short_help="Create a new AI agent from a description",
    epilog="""
    Examples:
      scaff create "summarize emails and flag urgent ones"
      scaff create "monitor github issues" -o ./github-agent -v

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
) -> None:
    """Create a new AI agent from a plain English description.

    scaff sends your description to OpenAI, which designs the agent's
    system prompt, tools, and dependencies. The result is a complete,
    runnable project in the output directory.
    """

    key_error = _check_api_key()
    if key_error:
        console.print(f"[bold red][ERROR][/bold red] {key_error}")
        raise typer.Exit(code=1)

    # Use config defaults if not specified
    output = output or ScaffConfig.get("output_dir")
    model = model or ScaffConfig.get("model")

    try:
        created_files = _run_agent(output, description, model, verbose)

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
        console.print("3. Set your API key in .env (or copy .env.example to .env)")
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
      scaff run "monitor github issues" -v

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
    verbose: bool = typer.Option(
        False,
        "--verbose",
        "-v",
        help="Show detailed generation steps",
    ),
) -> None:
    """Generate and run an AI agent - all in one command.

    This is the quickest way to use scaff. It generates the agent,
    installs dependencies, and starts the agent automatically.
    """
    key_error = _check_api_key()
    if key_error:
        console.print(f"[bold red][ERROR][/bold red] {key_error}")
        raise typer.Exit(code=1)

    output = output or ScaffConfig.get("output_dir")
    model = model or ScaffConfig.get("model")

    try:
        created_files = _run_agent(output, description, model, verbose)

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
    console.print('  • "summarize my emails and flag urgent ones"')
    console.print('  • "monitor github issues and auto-label them by priority"')
    console.print('  • "check the weather and send me a daily forecast"')
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

    # Step 4: Run mode
    console.print("[bold]Step 4: How should we run it?[/bold]")
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

        created_files = write_generated_files(agent_spec, output)

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
    console.print("[bold]Version:[/bold] 0.1.0")
    console.print("[bold]License:[/bold] MIT")
    console.print("[bold]Repository:[/bold] https://github.com/your-repo/scaff")
    console.print()
    console.print("[bold cyan]Features:[/bold cyan]")
    console.print("  [*] scaff run - generate, install deps, and run in one command")
    console.print("  [*] scaff wizard - guided setup for beginners")
    console.print("  [*] scaff create - generate a complete agent project")
    console.print("  [*] scaff examples - 6 pre-built agents to try")
    console.print("  [*] OpenAI GPT-4o powered design")
    console.print("  [*] Self-contained agents (no heavy frameworks)")
    console.print("  [*] Cross-platform (Windows, Mac, Linux)")
    console.print()
    console.print("[bold cyan]Tech Stack:[/bold cyan]")
    console.print("  • Python 3.11+")
    console.print("  • Typer (CLI)")
    console.print("  • OpenAI SDK")
    console.print("  • Rich (UI)")
    console.print("  • Jinja2 (Templates)")
    console.print()
    console.print("[bold cyan]Configuration:[/bold cyan]")
    console.print(f"  Config file: {ScaffConfig.CONFIG_FILE}")
    console.print()


def main() -> None:
    """Entry point for the scaff CLI."""
    app()


if __name__ == "__main__":
    main()
