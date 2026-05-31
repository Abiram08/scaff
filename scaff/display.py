"""Display utilities for token tracking and mode information."""

import sys
from rich.console import Console
from rich.table import Table
from typing import Dict, Any, Optional

console = Console()


class Display:
    """Display utilities for token tracking and mode info"""

    @staticmethod
    def show_mode_info(mode_name: str) -> None:
        """Display information about a mode"""
        from .request_enforcer import Mode, ModeRegistry

        try:
            mode = Mode(mode_name)
        except ValueError:
            console.print(f"[red]Invalid mode: {mode_name}[/red]")
            return

        config = ModeRegistry.get_config(mode)

        table = Table(title=f"Mode: {mode_name.upper()}")
        table.add_column("Setting", style="cyan")
        table.add_column("Value", style="green")

        table.add_row("Monthly Budget", f"${config.monthly_cost:.2f}")
        table.add_row("Monthly Tokens", f"{config.monthly_tokens:,}")
        table.add_row("Rate Limit", f"{config.rate_limit_per_minute} req/min")
        table.add_row("Rate Limit (burst)", f"{config.burst_capacity} req")
        table.add_row("Refill Rate", f"{config.refill_rate} req/s")
        table.add_row("Concurrent Limit", str(config.concurrent_limit))
        table.add_row("Cache TTL", f"{config.cache_ttl_days} days")
        table.add_row("Model", config.model)
        table.add_row("Temperature", f"{config.temperature:.1f}")

        console.print(table)

    @staticmethod
    def show_token_estimation(
        input_tokens: int,
        output_tokens_estimate: int,
        model: str,
    ) -> None:
        """Show token estimation before making request"""
        from .pricing import PricingCalculator

        cost = PricingCalculator.calculate_request_cost(
            model, input_tokens, output_tokens_estimate
        )

        console.print()
        console.print("[bold]Token Estimation:[/bold]")
        console.print(f"  Input: ~{input_tokens:,} tokens")
        console.print(f"  Output: ~{output_tokens_estimate:,} tokens (estimated)")
        console.print(f"  Total: ~{input_tokens + output_tokens_estimate:,} tokens")
        console.print(f"  Estimated cost: ${cost:.5f}")
        console.print()

    @staticmethod
    def show_token_usage(
        tokens_used: int,
        monthly_budget: int,
        cost: float,
        monthly_cost: float,
    ) -> None:
        """Show token usage statistics"""
        percent_used = (tokens_used / monthly_budget) * 100 if monthly_budget > 0 else 0
        remaining = monthly_budget - tokens_used
        percent_cost = (cost / monthly_cost) * 100 if monthly_cost > 0 else 0

        console.print()
        console.print("[bold]Token Usage:[/bold]")
        console.print(f"  Used: {tokens_used:,} / {monthly_budget:,} tokens ({percent_used:.1f}%)")
        console.print(f"  Remaining: {remaining:,} tokens")
        console.print(f"  Cost: ${cost:.2f} / ${monthly_cost:.2f} ({percent_cost:.1f}%)")
        console.print()

    @staticmethod
    def show_streaming_progress(
        token_count: int,
        final: bool = False,
    ) -> None:
        """Show streaming token progress in-place.

        Args:
            token_count: Current estimated token count
            final: If True, print final newline instead of \r
        """
        if final:
            print(f"[OK] Streaming complete: {token_count:,} tokens received")
        else:
            print(f"[*] Streaming: ~{token_count:,} tokens received...", end="\r")
            sys.stdout.flush()
