mod cache;
mod cli;
mod config;
mod display;
mod monitor;
mod python_bridge;
mod rate_limiter;
mod shell;
mod token_tracker;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Completions { .. } => {
            // Handled directly in cli.rs
            if let Err(e) = cli::execute(cli.command) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        _ => {
            if let Err(e) = cli::execute(cli.command) {
                eprintln!("\n  {} {}\n", console::style("ERROR").red().bold(), e);
                std::process::exit(1);
            }
        }
    }
}
