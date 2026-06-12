use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use crate::pipeline::{Confidence, Finding};

pub fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("\u{280b}\u{2819}\u{2839}\u{2838}\u{283b}\u{283a}\u{2807}\u{280f}\u{281b}\u{281a}")
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

/// Show a stage transition in the pipeline with a brief status message.
pub fn show_stage(stage: &str, msg: &str) {
    println!(
        "  {} {}  {}",
        style("◆").cyan(),
        style(stage).bold(),
        msg
    );
}

/// Show a stage completion with OK status.
pub fn stage_ok(stage: &str, detail: &str) {
    println!(
        "  {} {}  {}",
        style("✓").green(),
        style(stage).bold(),
        style(detail).dim()
    );
}

pub fn ok(msg: &str) {
    println!(" {} {}", style("OK").green().bold(), msg);
}

pub fn warn(msg: &str) {
    println!(" {} {}", style("!").yellow().bold(), msg);
}

pub fn err(msg: &str) {
    eprintln!(" {} {}", style("ERROR").red().bold(), msg);
}

pub fn kv(key: &str, value: &str) {
    println!("  {}: {}", style(key).cyan(), value);
}

/// Show a finding inline with its confidence badge.
pub fn finding(f: &Finding) {
    let emoji = match f.confidence {
        Confidence::High => "✅",
        Confidence::Medium => "⚠️",
        Confidence::Low => "🔴",
        Confidence::Contested => "⚡",
    };
    println!();
    println!(
        "  {} {}  {} {}",
        style("Finding").cyan().bold(),
        style(format!("{}/{}", f.index, f.total)).dim(),
        emoji,
        style(format!("[{}]", f.confidence)).dim(),
    );
    println!("  {}", f.content);
    if !f.sources.is_empty() {
        let sources_str: Vec<String> = f
            .sources
            .iter()
            .map(|s| format!("[{}] {}", s.id, s.title))
            .collect();
        println!("  {}", style(sources_str.join(", ")).dim());
    }
    if let Some(note) = &f.conflict_note {
        println!("  {} {}", style("⚡").yellow(), style(note).dim());
    }
}

pub fn section(title: &str) {
    println!();
    println!("{}", style(title).cyan().bold());
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
