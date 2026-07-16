use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use console::style;

use crate::config::{self, ScaffConfig};

pub const SCAFF_PROJECT_FILE: &str = ".scaff.yaml";

pub fn ensure_global_dirs() -> Result<()> {
    let dir = ScaffConfig::config_dir();
    fs::create_dir_all(&dir).with_context(|| format!("create {dir:?}"))?;
    fs::create_dir_all(dir.join("executions")).ok();
    fs::create_dir_all(dir.join("reports")).ok();
    fs::create_dir_all(dir.join("pipelines")).ok();
    Ok(())
}

pub fn has_global_config() -> bool {
    ScaffConfig::config_path().exists()
}

pub fn write_global_default_config() -> Result<()> {
    let cfg = ScaffConfig::default();
    cfg.save()
        .map_err(|e| anyhow::anyhow!("save default config: {e}"))?;
    Ok(())
}

/// Auto-setup on first run: detect provider from env, seed corpus.
/// Returns true if first-run setup was performed.
pub fn auto_setup(print_status: bool) -> Result<bool> {
    let is_first = !has_global_config();
    let cfg = ScaffConfig::load();

    // Check if we already have a working config with API keys
    let has_keys = !cfg.api_keys.is_empty()
        || config::PROVIDERS.iter().any(|p| {
            p.requires_key
                && std::env::var(p.env_var)
                    .ok()
                    .is_some_and(|v| !v.trim().is_empty())
        });

    if is_first {
        if print_status {
            let bar = "─".repeat(50);
            println!("\n{}", style(&bar).dim());
            println!("  {}  scaff  —  Harness Research Agent", style("⚡").cyan());
            println!("{}", style(&bar).dim());
            println!();
        }
        ensure_global_dirs()?;
    }

    // Auto-detect provider from env vars on first run
    let mut needs_save = false;
    let mut cfg = cfg;
    if is_first || cfg.default_provider == "openai" && !has_keys {
        for provider in config::PROVIDERS {
            if provider.requires_key {
                if let Ok(key) = std::env::var(provider.env_var) {
                    if !key.trim().is_empty() {
                        if is_first || cfg.default_provider != provider.name {
                            cfg.default_provider = provider.name.to_string();
                            cfg.set_api_key(provider.name, key.trim().to_string());
                            if print_status {
                                println!(
                                    "  {} Detected {} from ${}",
                                    style("✓").green(),
                                    style(provider.name).cyan().bold(),
                                    provider.env_var
                                );
                            }
                            needs_save = true;
                        }
                        break;
                    }
                }
            }
        }
    }

    // On first run, check for Ollama (no key required)
    if is_first && !has_keys {
        if let Ok(resp) = ureq::get("http://localhost:11434/api/tags")
            .timeout(std::time::Duration::from_secs(3))
            .call()
        {
            if resp.status() == 200 {
                cfg.default_provider = "ollama".to_string();
                if print_status {
                    println!(
                        "  {} Found {} running locally (no API key needed)",
                        style("✓").green(),
                        style("Ollama").cyan().bold()
                    );
                }
                needs_save = true;
            }
        }
    }

    if needs_save {
        cfg.save()
            .map_err(|e| anyhow::anyhow!("save config: {e}"))?;
    }

    // Seed corpus if empty
    let seeded = crate::corpus::ensure_seeded().map_err(|e| anyhow::anyhow!("seed corpus: {e}"))?;

    if is_first && print_status {
        println!(
            "  {} Corpus seeded with {} entries",
            style("✓").green(),
            style(seeded).cyan()
        );
        if has_keys || cfg.default_provider == "ollama" {
            println!(
                "  {} Ready! Try: scaff \"what is Harness CD?\"",
                style("→").dim()
            );
        } else {
            println!(
                "  {} No API key found. Set one via: scaff config set-key openai <KEY>",
                style("!").yellow()
            );
            println!(
                "  {} Or set env: export OPENAI_API_KEY=sk-...",
                style("→").dim()
            );
        }
        println!();
    }

    Ok(is_first)
}

/// Interactive setup wizard
pub fn run_setup_wizard() -> Result<()> {
    ensure_global_dirs()?;

    let bar = "─".repeat(50);
    println!("\n{}", style(&bar).cyan());
    println!("  {} scaff setup", style("⚡").cyan().bold());
    println!("{}", style(&bar).cyan());
    println!();
    println!("  This wizard will configure scaff for first use.");
    println!();

    // Step 1: Provider selection
    println!(
        "  {}",
        style("Step 1: Choose an LLM provider").cyan().bold()
    );
    println!();

    for (i, p) in config::PROVIDERS.iter().enumerate() {
        let status = if p.requires_key {
            let from_env = std::env::var(p.env_var)
                .ok()
                .is_some_and(|v| !v.trim().is_empty());
            let from_cfg = ScaffConfig::load()
                .api_keys
                .get(p.name)
                .is_some_and(|v| !v.trim().is_empty());
            if from_env || from_cfg {
                "✓ configured"
            } else {
                "needs key"
            }
        } else {
            let running = ureq::get("http://localhost:11434/api/tags")
                .timeout(std::time::Duration::from_secs(2))
                .call()
                .is_ok_and(|r| r.status() == 200);
            if running {
                "✓ running (no key)"
            } else {
                "not running"
            }
        };
        let status_color = match status.chars().next() {
            Some('✓') => style(status).green(),
            _ => style(status).yellow(),
        };
        println!(
            "    {}. {} — {}  ({})",
            i + 1,
            style(p.name).bold(),
            p.default_model,
            status_color
        );
    }

    println!();
    let provider_idx = loop {
        print!("  {} ", style("Select provider [1-5]:").dim());
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let n: usize = match input.trim().parse() {
            Ok(n) if n >= 1 && n <= config::PROVIDERS.len() => n,
            _ => {
                println!(
                    "  {} Please enter a number between 1 and 5",
                    style("!").yellow()
                );
                continue;
            }
        };
        break n;
    };
    let provider = &config::PROVIDERS[provider_idx - 1];
    let mut cfg = ScaffConfig::load();
    cfg.default_provider = provider.name.to_string();

    // Step 2: API key
    if provider.requires_key {
        let has_key = std::env::var(provider.env_var)
            .ok()
            .is_some_and(|v| !v.trim().is_empty())
            || cfg
                .api_keys
                .get(provider.name)
                .is_some_and(|v| !v.trim().is_empty());

        if !has_key {
            println!();
            println!("  {}", style("Step 2: Enter your API key").cyan().bold());
            println!();
            println!("  Get a key at: {}", style(provider.base_url).dim());
            println!(
                "  (press Enter to skip, set later with `scaff config set-key {} <KEY>`)",
                provider.name
            );
            println!();
            print!("  {} ", style("API key:").dim());
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().lock().read_line(&mut input)?;
            let key = input.trim().to_string();
            if !key.is_empty() {
                cfg.set_api_key(provider.name, key);
            }
        }
    }

    cfg.save()
        .map_err(|e| anyhow::anyhow!("save config: {e}"))?;
    println!();
    println!(
        "  {} Configuration saved to {}",
        style("✓").green(),
        ScaffConfig::config_path().display()
    );

    // Step 3: Seed corpus
    println!();
    println!("  {}", style("Step 3: Seed the corpus").cyan().bold());
    print!("  {} ", style("Downloading seed entries...").dim());
    io::stdout().flush()?;
    let n = crate::corpus::ensure_seeded()?;
    println!("{} entries", style(n).cyan());
    println!(
        "  {} Corpus ready at {}",
        style("✓").green(),
        ScaffConfig::corpus_db_path().display()
    );

    // Step 4: Test connection
    println!();
    println!("  {}", style("Step 4: Test connection").cyan().bold());
    print!("  {} ", style("Testing...").dim());
    io::stdout().flush()?;

    let api_key = config::resolve_api_key(provider, &cfg).ok().flatten();
    let model = config::resolve_model(provider, None, false);

    let test_passed = crate::llm::chat(
        provider,
        api_key.as_deref(),
        &model,
        &[crate::llm::ChatMessage::user("Say exactly: OK")],
        0.0,
        50,
        &[],
    )
    .map(|r| !r.content.is_empty())
    .unwrap_or(false);

    if test_passed {
        println!("{}", style("connected").green());
    } else {
        println!("{}", style("failed — check your API key").yellow());
        println!("  {} Run `scaff doctor` for diagnostics", style("→").dim());
    }

    // Done!
    println!();
    println!("{}", style(&bar).green());
    println!("  {} scaff is ready!", style("✓").green().bold());
    println!();
    println!("  Try:");
    println!("    scaff \"What is Harness Continuous Delivery?\"");
    println!("    scaff chat");
    println!("    scaff doctor");
    println!();
    println!("{}", style(&bar).green());
    println!();

    Ok(())
}

pub fn init_project(cwd: &Path) -> Result<()> {
    let path = cwd.join(SCAFF_PROJECT_FILE);
    if path.exists() {
        anyhow::bail!("{} already exists in this directory", SCAFF_PROJECT_FILE);
    }
    let body = format!(
        "# scaff project config — drop this in your repo root\n\
         # See `scaff init --help` for the full schema.\n\
         \n\
         version: 1\n\
         # Default provider and model used when --provider/--model aren't passed.\n\
         default_provider: openai\n\
         default_model: gpt-4o-mini\n\
         \n\
         # Optional: include local documentation from your repo as a connector.\n\
         connectors:\n\
           - kind: local\n\
             name: repo-docs\n\
             path: ./docs\n\
             # Only these extensions are picked up: .md .markdown .txt .rst .adoc\n\
         \n\
         # Default research behavior.\n\
         research:\n\
           retrieval_k: 8\n\
           web_enabled: true\n\
           show_stages: false\n\
           show_sources: true\n\
         \n\
         # Scaff can also expose aliases so a team can share common research prompts.\n\
         pipelines:\n\
           - name: canary\n\
             question: \"What is the recommended canary rollout strategy in Harness CD?\"\n\
           - name: onboard\n\
             question: \"I'm new to Harness — what should I learn first?\"\n\
         \n\
         # Created by scaff init on {}\n",
        Utc::now().to_rfc3339()
    );
    fs::write(&path, body).with_context(|| format!("write {path:?}"))?;
    Ok(())
}

pub fn print_welcome() {
    let bar = "─".repeat(60);
    println!("\n{}", bar);
    println!("  Welcome to scaff — the Harness Research Agent.");
    println!("{}", bar);
    println!();
    println!("  scaff is a Pi-simple Harness research agent (four tools, one loop).");
    println!("  Ask a question, get a cited Markdown report.");
    println!();
    println!("  Quick start:");
    println!("    1. Set a provider key:");
    println!("         export OPENAI_API_KEY=sk-proj-...");
    println!("         # or: scaff config set-key openai sk-proj-...");
    println!("    2. Bootstrap the corpus (one time, ~30 entries):");
    println!("         scaff corpus update");
    println!("    3. Ask your first question:");
    println!("         scaff \"what is Harness CD canary?\"");
    println!();
    println!("  Want an interactive REPL? Run: scaff chat");
    println!("  Want to index your own docs? Run:  scaff corpus add-dir ./docs");
    println!("  Run `scaff doctor` any time to check the install.");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_project_writes_file() {
        let tmp = tempfile::tempdir().unwrap();
        init_project(tmp.path()).unwrap();
        let p = tmp.path().join(SCAFF_PROJECT_FILE);
        assert!(p.exists());
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("version: 1"));
        assert!(body.contains("default_provider: openai"));
    }
}
