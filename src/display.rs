use indicatif::{ProgressBar, ProgressStyle};

use crate::pipeline::{Confidence, Finding};

// ── Design Tokens (AGENTS.md §13) ──────────────────────────────────
// true-color ANSI escape helpers — console::Color only offers ANSI 256

fn ansi_rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

const RESET: &str = "\x1b[0m";
const BOLD:   &str = "\x1b[1m";
const DIM:    &str = "\x1b[2m";

// brand
fn relay_fg()    -> String { ansi_rgb(99, 102, 241) }
fn relay_l_fg()  -> String { ansi_rgb(129, 140, 248) }
#[allow(dead_code)]
fn relay_d_fg()  -> String { ansi_rgb(55, 48, 163) }
#[allow(dead_code)]
fn accent_fg()   -> String { ansi_rgb(168, 85, 247) }

// confidence
fn conf_high_fg()      -> String { ansi_rgb(16, 185, 129) }
fn conf_medium_fg()    -> String { ansi_rgb(245, 158, 11) }
fn conf_low_fg()       -> String { ansi_rgb(249, 115, 22) }
fn conf_contested_fg() -> String { ansi_rgb(239, 68, 68) }

fn conf_color(c: &Confidence) -> String {
    match c {
        Confidence::High     => conf_high_fg(),
        Confidence::Medium   => conf_medium_fg(),
        Confidence::Low      => conf_low_fg(),
        Confidence::Contested => conf_contested_fg(),
    }
}

pub fn stage_color(status: &str) -> String {
    match status {
        "planning"     => ansi_rgb(251, 191, 36),
        "searching"    => ansi_rgb(99, 102, 241),
        "verifying"    => ansi_rgb(168, 85, 247),
        "synthesizing" => ansi_rgb(6, 182, 212),
        "done"         => ansi_rgb(16, 185, 129),
        "error"        => ansi_rgb(239, 68, 68),
        _ => relay_fg(),
    }
}

// ── Public API ─────────────────────────────────────────────────────

pub fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("\u{280b}\u{2819}\u{2839}\u{2838}\u{283b}\u{283a}\u{2807}\u{280f}\u{281b}\u{281a}")
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

/// Show a stage transition in the pipeline with a brief status message.
pub fn show_stage(stage: &str, msg: &str) {
    let rf = relay_fg();
    let rl = relay_l_fg();
    println!(
        "  {BOLD}{rf}◆{RESET} {BOLD}{rl}{stage}{RESET}  {msg}"
    );
}

/// Show a stage completion with OK status.
pub fn stage_ok(stage: &str, detail: &str) {
    let g = conf_high_fg();
    let rl = relay_l_fg();
    println!(
        "  {g}✓{RESET} {BOLD}{rl}{stage}{RESET}  {DIM}{detail}{RESET}"
    );
}

pub fn ok(msg: &str) {
    let g = conf_high_fg();
    println!(" {BOLD}{g}OK{RESET} {msg}");
}

pub fn warn(msg: &str) {
    let y = conf_medium_fg();
    println!(" {BOLD}{y}!{RESET} {msg}");
}

pub fn err(msg: &str) {
    let r = conf_contested_fg();
    eprintln!(" {BOLD}{r}ERROR{RESET} {msg}");
}

pub fn kv(key: &str, value: &str) {
    let rl = relay_l_fg();
    println!("  {rl}{key}{RESET}: {value}");
}

/// Show a finding inline with its confidence badge.
pub fn finding(f: &Finding) {
    let emoji = match f.confidence {
        Confidence::High      => "✅",
        Confidence::Medium    => "⚠️",
        Confidence::Low       => "🔴",
        Confidence::Contested => "⚡",
    };
    let rf = relay_fg();
    let cf = conf_color(&f.confidence);

    println!();
    println!(
        "  {BOLD}{rf}Finding{RESET} {DIM}{idx}/{tot}{RESET}  {emoji} {cf}[{conf}]{RESET}",
        idx = f.index,
        tot = f.total,
        conf = f.confidence,
    );
    println!("  {}", f.content);
    if !f.sources.is_empty() {
        let sources_str: Vec<String> = f
            .sources
            .iter()
            .map(|s| format!("[{}] {}", s.id, s.title))
            .collect();
        println!("  {DIM}{}{RESET}", sources_str.join(", "));
    }
    if let Some(note) = &f.conflict_note {
        let y = conf_medium_fg();
        println!("  {BOLD}{y}⚠{RESET} {DIM}{note}{RESET}");
    }
}

pub fn section(title: &str) {
    let rl = relay_l_fg();
    println!();
    println!("{BOLD}{rl}{title}{RESET}");
    println!();
}

pub fn fmt_bytes(b: u64) -> String {
    if b < 1024 {
        format!("{b} B")
    } else if b < 1024 * 1024 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{:.1} MB", b as f64 / (1024.0 * 1024.0))
    }
}
