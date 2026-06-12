use clap::Parser;
use scaff::cli;

fn main() {
    let c = cli::Cli::parse();

    if matches!(&c.command, Some(cli::Commands::Completions { .. })) {
        if let Err(e) = cli::execute(c) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = cli::execute(c) {
        let msg = e.to_string();
        eprintln!();
        if msg.contains("API key") {
            eprintln!("  {} {}", console::style("✗").red().bold(), msg);
            eprintln!("  {} Set one via: scaff setup or scaff config set-key <provider> <KEY>", console::style("→").dim());
            eprintln!("  {} Or set the env var: export OPENAI_API_KEY=sk-...", console::style("→").dim());
        } else if msg.contains("research failed") {
            eprintln!("  {} {}", console::style("✗").red().bold(), msg);
            eprintln!("  {} Try: scaff doctor to check your setup", console::style("→").dim());
            eprintln!("  {} Or: scaff setup to reconfigure", console::style("→").dim());
        } else {
            eprintln!("  {} {}", console::style("ERROR").red().bold(), msg);
        }
        eprintln!();
        std::process::exit(1);
    }
}
