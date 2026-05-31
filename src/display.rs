use std::path::Path;

use console::style;
use indicatif::{ProgressBar, ProgressStyle};

pub fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
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

pub fn section(title: &str) {
    println!();
    println!("{}", style(title).cyan().bold());
    println!();
}

pub fn kv(key: &str, value: &str) {
    println!("  {}: {}", style(key).cyan(), value);
}

pub fn file_tree(base: &Path, files: &[String]) {
    println!("  {}", style(base.display()).bold());
    for f in files {
        let rel = Path::new(f)
            .strip_prefix(base)
            .unwrap_or(Path::new(f));
        println!("  {} {}", style("├──").dim(), rel.display());
    }
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
