//! Terminal UX helpers for the CLI agent.

use indicatif::{ProgressBar, ProgressStyle};

fn ansi_rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

fn relay_fg() -> String {
    ansi_rgb(99, 102, 241)
}
fn relay_l_fg() -> String {
    ansi_rgb(129, 140, 248)
}
fn conf_high_fg() -> String {
    ansi_rgb(16, 185, 129)
}
fn conf_medium_fg() -> String {
    ansi_rgb(245, 158, 11)
}

pub fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars(
                "\u{280b}\u{2819}\u{2839}\u{2838}\u{283b}\u{283a}\u{2807}\u{280f}\u{281b}\u{281a}",
            )
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

pub fn show_stage(stage: &str, msg: &str) {
    let rf = relay_fg();
    let rl = relay_l_fg();
    println!("  {BOLD}{rf}◆{RESET} {BOLD}{rl}{stage}{RESET}  {msg}");
}

pub fn stage_ok(stage: &str, detail: &str) {
    let g = conf_high_fg();
    let rl = relay_l_fg();
    println!("  {g}✓{RESET} {BOLD}{rl}{stage}{RESET}  {DIM}{detail}{RESET}");
}

pub fn ok(msg: &str) {
    let g = conf_high_fg();
    println!("  {g}✓{RESET} {msg}");
}

pub fn warn(msg: &str) {
    let y = conf_medium_fg();
    println!("  {y}!{RESET} {msg}");
}

pub fn err(msg: &str) {
    eprintln!("  \x1b[38;2;239;68;68m✗{RESET} {msg}");
}

pub fn kv(key: &str, value: &str) {
    let rl = relay_l_fg();
    println!("  {rl}{key}{RESET}: {value}");
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
