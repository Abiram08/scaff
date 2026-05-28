"""Typer CLI entry point for scaff."""

import os
from pathlib import Path

import typer
from rich.console import Console
from rich.table import Table
from rich.tree import Tree

from .config import ScaffConfig
from .examples import get_example_list, get_example_command
from .generator import generate_agent_spec
from .writer import write_generated_files

app = typer.Typer(help="scaff - CLI tool for scaffolding AI agent projects")
console = Console()


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
        help="Output directory",
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
    """Create a new AI agent from a plain English description.

    scaff sends your description to OpenAI, which designs the agent's
    system prompt, tools, and dependencies. The result is a complete,
    runnable project in the output directory.
    """

    # Use config defaults if not specified
    output = output or ScaffConfig.get("output_dir")
    model = model or ScaffConfig.get("model")

    try:
        # Generate agent specification
        with console.status("[bold green]Generating your agent..."):
            agent_spec = generate_agent_spec(description, model=model, verbose=verbose)
        
        if verbose:
            console.print("[OK] Agent specification generated successfully")
        
        # Write files
        if verbose:
            console.print("[*] Writing generated files...")
        
        created_files = write_generated_files(agent_spec, output)
        
        if verbose:
            console.print("[OK] Files written successfully")
        
        # Display success message with tree
        console.print()
        console.print("[bold green][OK] Agent scaffolded successfully![/bold green]")
        console.print()
        
        # Create and display tree
        tree = Tree(f"{output}")
        for file_path in sorted(created_files):
            rel_path = Path(file_path).name
            tree.add(f"{rel_path}")
        
        console.print(tree)
        console.print()
        console.print("[bold]Next steps:[/bold]")
        console.print(f"1. cd {output}")
        console.print("2. pip install -r requirements.txt")
        console.print("3. export OPENAI_API_KEY='your-key'")
        console.print("4. python agent.py")
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
    short_help="Show available example agents",
    epilog="""
    Examples:
      scaff examples
      scaff create "summarize emails and flag urgent ones"

    Copy any example command to get started quickly.
    """,
)
def examples() -> None:
    """Show available example agents.

    Displays 6 pre-built agent examples you can create with a single command.
    Each example shows the description, tools, dependencies, and the exact
    'scaff create' command to run.
    """
    console.print()
    console.print("[bold cyan]Available Example Agents:[/bold cyan]")
    console.print()
    console.print(get_example_list())


@app.command(
    short_help="Manage scaff configuration",
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
    """Manage scaff configuration.

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
        console.print("[bold cyan]Current Configuration:[/bold cyan]")
        console.print()
        
        table = Table(title="scaff Config")
        table.add_column("Key", style="cyan")
        table.add_column("Value", style="green")
        
        for k, v in config_data.items():
            table.add_row(k, str(v))
        
        console.print(table)
        console.print()
        console.print(f"Config file: {ScaffConfig.CONFIG_FILE}")
        console.print()
        
    elif action == "set":
        if not key or not value:
            console.print("[bold red][ERROR] set action requires --key and --value[/bold red]")
            raise typer.Exit(code=1)
        
        ScaffConfig.set(key, value)
        console.print(f"[OK] Set {key} = {value}")
        
    elif action == "reset":
        ScaffConfig.reset()
        console.print("[OK] Configuration reset to defaults")
        
    else:
        console.print(f"[ERROR] Unknown action: {action}")
        raise typer.Exit(code=1)


@app.command(
    short_help="Validate an agent description (dry run)",
    epilog="""
    Example:
      scaff validate "summarize emails and flag urgent ones"

    This tests your description with OpenAI without writing any files.
    Use it to check if a description will work before generating.
    """,
)
def validate(
    description: str = typer.Argument(..., help="Agent description to validate"),
) -> None:
    """Validate an agent description (dry run).

    Tests your description with OpenAI to verify the agent specification
    can be generated, WITHOUT writing any files to disk.

    This is useful for checking descriptions before committing to
    full generation, especially when iterating on a complex agent idea.
    """
    
    try:
        console.print()
        console.print("[*] Validating agent description...")
        console.print()
        
        # Try to generate spec without writing files
        agent_spec = generate_agent_spec(description, verbose=True)
        
        console.print()
        console.print("[bold green][OK] Description is valid![/bold green]")
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
    console.print("  [*] Plain English to AI agent scaffolding")
    console.print("  [*] OpenAI GPT-4o powered design")
    console.print("  [*] Automatic tool discovery")
    console.print("  [*] Self-contained agents")
    console.print("  [*] MCP configuration")
    console.print("  [*] Cross-platform support")
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
