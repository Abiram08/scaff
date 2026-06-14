"""Design tokens and display helpers — mirrors AGENTS.md §13 color system.

Usage:
    from relay.display import console, Token, confidence_style

    console.print(Token.relay("Relay"), style="bold")
    console.print(confidence_style("high")("✓ High confidence"))
"""

from rich.console import Console
from rich.style import Style
from rich.text import Text
from rich.table import Table
from ..types import Confidence, Finding, Source


# ── Brand Palette ───────────────────────────────────────────────────

class Token:
    """Design tokens matching AGENTS.md §13."""

    # Brand
    RELAY = "#6366f1"
    RELAY_LIGHT = "#818cf8"
    RELAY_DARK = "#3730a3"
    ACCENT = "#a855f7"
    WARM = "#f59e0b"

    # Confidence
    CONFIDENCE_HIGH = "#10b981"
    CONFIDENCE_MEDIUM = "#f59e0b"
    CONFIDENCE_LOW = "#f97316"
    CONFIDENCE_CONTESTED = "#ef4444"

    # Stage status
    STAGE_PLANNING = "#fbbf24"
    STAGE_SEARCHING = "#6366f1"
    STAGE_VERIFYING = "#a855f7"
    STAGE_SYNTHESIZING = "#06b6d4"
    STAGE_DONE = "#10b981"
    STAGE_ERROR = "#ef4444"

    # Glass / surface
    GLASS_BG = "rgba(15,15,25,0.85)"
    GLASS_BORDER = "rgba(99,102,241,0.15)"

    # ── Helpers ─────────────────────────────────────────────────

    @classmethod
    def relay(cls, text: str) -> Text:
        return Text(text, style=Style(color=cls.RELAY))

    @classmethod
    def relay_light(cls, text: str) -> Text:
        return Text(text, style=Style(color=cls.RELAY_LIGHT))

    @classmethod
    def confidence(cls, level: Confidence, text: str) -> Text:
        color = {
            Confidence.HIGH: cls.CONFIDENCE_HIGH,
            Confidence.MEDIUM: cls.CONFIDENCE_MEDIUM,
            Confidence.LOW: cls.CONFIDENCE_LOW,
            Confidence.CONTESTED: cls.CONFIDENCE_CONTESTED,
        }[level]
        return Text(text, style=Style(color=color, bold=True))

    @classmethod
    def stage(cls, status: str, text: str) -> Text:
        color = {
            "planning": cls.STAGE_PLANNING,
            "searching": cls.STAGE_SEARCHING,
            "verifying": cls.STAGE_VERIFYING,
            "synthesizing": cls.STAGE_SYNTHESIZING,
            "done": cls.STAGE_DONE,
            "error": cls.STAGE_ERROR,
        }.get(status, cls.RELAY)
        return Text(text, style=Style(color=color))

    @classmethod
    def muted(cls, text: str) -> Text:
        return Text(text, style=Style(dim=True))


# ── Shared Console ──────────────────────────────────────────────────

console = Console(
    highlight=False,
)


# ── High-level display helpers ──────────────────────────────────────

def show_stage(stage: str, msg: str) -> None:
    console.print(
        Text.assemble(
            Text(" ◆ ", style=Style(color=Token.RELAY, bold=True)),
            Text(stage, style=Style(color=Token.RELAY_LIGHT, bold=True)),
            f"  {msg}",
        )
    )


def show_ok(stage: str, detail: str) -> None:
    console.print(
        Text.assemble(
            Text(" ✓", style=Style(color=Token.CONFIDENCE_HIGH)),
            Text(f" {stage}", style=Style(color=Token.RELAY_LIGHT, bold=True)),
            Text(f"  {detail}", style=Style(dim=True)),
        )
    )


def show_finding(finding: Finding) -> None:
    emoji = finding.confidence.emoji()
    console.print()
    console.print(
        Text.assemble(
            Token.relay(" Finding"),
            Text(f" {finding.index}/{finding.total}", style=Style(dim=True)),
            Text(f"  {emoji}", style=Style(bold=True)),
            Token.confidence(finding.confidence, f" [{finding.confidence.value}]"),
        )
    )
    console.print(finding.content)
    if finding.sources:
        sources_str = ", ".join(f"[{s.id}] {s.title}" for s in finding.sources)
        console.print(Text(sources_str, style=Style(dim=True)))
    if finding.conflict_note:
        console.print(
            Text.assemble(
                Text(" ⚠", style=Style(color=Token.CONFIDENCE_MEDIUM, bold=True)),
                Text(f" {finding.conflict_note}", style=Style(dim=True)),
            )
        )


def show_confidence_table(findings: list[Finding]) -> None:
    table = Table(box=None, border_style=Style(dim=True))
    table.add_column("#", style=Style(dim=True), width=3)
    table.add_column("Confidence", no_wrap=True)
    table.add_column("Content", overflow="fold")

    for i, f in enumerate(findings, 1):
        color = {
            Confidence.HIGH: Token.CONFIDENCE_HIGH,
            Confidence.MEDIUM: Token.CONFIDENCE_MEDIUM,
            Confidence.LOW: Token.CONFIDENCE_LOW,
            Confidence.CONTESTED: Token.CONFIDENCE_CONTESTED,
        }[f.confidence]
        table.add_row(
            str(i),
            f"[{f.confidence.value.upper()}]",
            Text(f.content[:120], style=Style(color=color)),
        )

    console.print(table)
