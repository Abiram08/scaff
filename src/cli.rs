use std::fs;
use std::path::Path;

use clap::{Parser, Subcommand};
use serde_json::Value;

use crate::cache::Cache;
use crate::config::ScaffConfig;
use crate::display;
use crate::python_bridge::{self, GenerateRequest};
use crate::rate_limiter::{Mode, RateLimiter};
use crate::shell;
use crate::token_tracker::TokenTracker;

#[derive(Parser)]
#[command(name = "scaff", version, about = "Turn plain English into ready-to-run AI agents. No coding required.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new AI agent from a description (generate files, run later)
    Create {
        /// Plain English description of the AI agent
        description: String,

        /// Output directory for generated files
        #[arg(short = 'o', long)]
        output: Option<String>,

        /// OpenAI model to use
        #[arg(short = 'm', long)]
        model: Option<String>,

        /// Show detailed generation steps
        #[arg(short = 'v', long)]
        verbose: bool,

        /// Use gpt-4o-mini to save 90% on API costs
        #[arg(short = 'c', long)]
        cheap: bool,

        /// Print estimated token usage and cost before calling
        #[arg(long)]
        show_cost: bool,

        /// Bypass cache and force a fresh API call
        #[arg(long)]
        no_cache: bool,

        /// Show detailed token usage and remaining budget
        #[arg(short = 't', long)]
        tokens: bool,

        /// AI provider: openai, anthropic, gemini, ollama
        #[arg(short = 'p', long)]
        provider: Option<String>,

        /// Generate web UI: fastapi or streamlit
        #[arg(long)]
        ui: Option<String>,

        /// Cron expression for scheduled runs
        #[arg(long)]
        schedule: Option<String>,

        /// Conversation memory: sqlite
        #[arg(long)]
        memory: Option<String>,

        /// Agent archetype: cli, web-api, chatbot, scheduler, memory
        #[arg(long, default_value = "cli")]
        archetype: String,
    },

    /// Generate and run an agent in one command
    Run {
        /// Plain English description of the AI agent
        description: String,

        /// Output directory
        #[arg(short = 'o', long)]
        output: Option<String>,

        /// OpenAI model
        #[arg(short = 'm', long)]
        model: Option<String>,

        /// Use gpt-4o-mini to save 90%
        #[arg(short = 'c', long)]
        cheap: bool,

        /// Print estimated cost
        #[arg(long)]
        show_cost: bool,

        /// Bypass cache
        #[arg(long)]
        no_cache: bool,

        /// Show detailed steps
        #[arg(short = 'v', long)]
        verbose: bool,

        /// Show token usage
        #[arg(short = 't', long)]
        tokens: bool,

        /// AI provider
        #[arg(short = 'p', long)]
        provider: Option<String>,

        /// Web UI mode
        #[arg(long)]
        ui: Option<String>,

        /// Cron schedule
        #[arg(long)]
        schedule: Option<String>,

        /// Conversation memory
        #[arg(long)]
        memory: Option<String>,
    },

    /// Interactive setup wizard for beginners
    Wizard,

    /// Show ready-to-use example agents
    Examples,

    /// Test a description without creating files (dry run)
    Validate {
        /// Agent description to test
        description: String,
    },

    /// Manage scaff settings
    Config {
        /// Action: show, set, reset
        action: Option<String>,

        /// Config key to set
        #[arg(short = 'k', long)]
        key: Option<String>,

        /// Config value to set
        #[arg(short = 'v', long)]
        value: Option<String>,
    },

    /// Manage response cache
    Cache {
        /// Action: status or clear
        action: Option<String>,
    },

    /// Show scaff information and statistics
    Info,

    /// Show token usage statistics
    Stats {
        /// Month to show (YYYY-MM format)
        #[arg(short = 'm', long)]
        month: Option<String>,
    },

    /// Set API limiting mode
    Mode {
        /// Mode: min, medium, or max
        mode_name: Option<String>,
    },

    /// View analytics and performance metrics
    Analytics {
        /// Time period: daily, weekly, monthly
        #[arg(short = 'p', long, default_value = "daily")]
        period: String,

        /// Filter by model
        #[arg(short = 'm', long)]
        model: Option<String>,
    },

    /// Preview cost across all modes
    Estimate {
        /// Agent description to estimate
        description: String,

        /// Show detailed breakdown
        #[arg(short = 'v', long)]
        verbose: bool,

        /// Filter by mode: min, medium, max
        #[arg(short = 'm', long)]
        mode: Option<String>,
    },

    /// View error reports and diagnostics
    Errors {
        /// Show recent errors
        #[arg(short = 'r', long)]
        recent: bool,
    },

    /// Show application state and configuration
    State {
        /// Reset to defaults
        #[arg(long)]
        reset: bool,
    },

    /// Modify an existing generated agent
    Edit {
        /// Agent directory
        #[arg(short = 'd', long, default_value = ".")]
        dir: String,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New system prompt
        #[arg(long)]
        system_prompt: Option<String>,

        /// Change AI provider
        #[arg(short = 'p', long)]
        provider: Option<String>,

        /// Change web UI mode
        #[arg(long)]
        ui: Option<String>,

        /// Change cron schedule
        #[arg(long)]
        schedule: Option<String>,

        /// Show detailed regeneration steps
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Generate deployment configs
    Deploy {
        /// Agent directory
        #[arg(short = 'd', long, default_value = ".")]
        dir: String,

        /// Platform: docker, railway, render, heroku
        #[arg(short = 'p', long)]
        platform: Option<String>,

        /// Show detailed output
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Run agent against test prompts
    Test {
        /// Test prompts
        prompt: Vec<String>,

        /// Agent directory
        #[arg(short = 'd', long, default_value = ".")]
        dir: String,

        /// File containing test prompts
        #[arg(short = 'f', long)]
        prompts: Option<String>,

        /// Show full responses
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Regenerate agent, preserving user changes
    Upgrade {
        /// Agent directory
        #[arg(short = 'd', long, default_value = ".")]
        dir: String,

        /// Change provider
        #[arg(short = 'p', long)]
        provider: Option<String>,

        /// Show what would change without writing
        #[arg(long)]
        dry_run: bool,

        /// Show detailed diff output
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Real-time usage monitoring dashboard
    Monitor {
        /// Refresh interval in seconds
        #[arg(short = 'r', long, default_value = "2")]
        refresh: u64,

        /// Show a single snapshot and exit
        #[arg(long)]
        once: bool,
    },

    /// Interactive REPL for iteratively building agents
    Shell,

    /// Generate shell completions
    Completions {
        /// Shell type: bash, zsh, fish, powershell, elvish
        shell: String,
    },

    /// Export token usage data
    Export {
        /// Format: json or csv
        #[arg(short = 'f', long, default_value = "csv")]
        format: String,

        /// Output file (default: stdout)
        #[arg(short = 'o', long)]
        output: Option<String>,

        /// Session ID to export (default: all)
        #[arg(short = 's', long)]
        session: Option<String>,
    },
}

pub fn execute(command: Commands) -> Result<(), String> {
    match command {
        Commands::Create {
            description,
            output,
            model,
            verbose,
            cheap,
            show_cost,
            no_cache,
            tokens,
            provider,
            ui,
            schedule,
            memory,
            archetype,
        } => cmd_create(description, output, model, verbose, cheap, show_cost, no_cache, tokens, provider, ui, schedule, memory, archetype),

        Commands::Run {
            description,
            output,
            model,
            cheap,
            show_cost,
            no_cache,
            verbose,
            tokens,
            provider,
            ui,
            schedule,
            memory,
        } => cmd_run(description, output, model, cheap, show_cost, no_cache, verbose, tokens, provider, ui, schedule, memory),

        Commands::Wizard => cmd_wizard(),
        Commands::Examples => cmd_examples(),
        Commands::Validate { description } => cmd_validate(description),

        Commands::Config { action, key, value } => {
            cmd_config(action.unwrap_or_else(|| "show".to_string()), key, value)
        }

        Commands::Cache { action } => {
            cmd_cache(action.unwrap_or_else(|| "status".to_string()))
        }

        Commands::Info => cmd_info(),
        Commands::Stats { month } => cmd_stats(month),
        Commands::Mode { mode_name } => cmd_mode(mode_name.unwrap_or_else(|| "medium".to_string())),

        Commands::Analytics { period, model } => {
            cmd_analytics(period, model)
        }

        Commands::Estimate { description, verbose, mode } => {
            cmd_estimate(description, verbose, mode)
        }

        Commands::Errors { recent } => cmd_errors(recent),
        Commands::State { reset } => cmd_state(reset),

        Commands::Edit {
            dir,
            description,
            system_prompt,
            provider,
            ui,
            schedule,
            verbose,
        } => cmd_edit(dir, description, system_prompt, provider, ui, schedule, verbose),

        Commands::Deploy { dir, platform, verbose } => {
            cmd_deploy(dir, platform, verbose)
        }

        Commands::Test {
            prompt,
            dir,
            prompts,
            verbose,
        } => cmd_test(prompt, dir, prompts, verbose),

        Commands::Upgrade {
            dir,
            provider,
            dry_run,
            verbose,
        } => cmd_upgrade(dir, provider, dry_run, verbose),

        Commands::Monitor { refresh, once } => cmd_monitor(refresh, once),
        Commands::Shell => shell::run_repl(),
        Commands::Completions { shell } => cmd_completions(shell),
        Commands::Export { format, output, session } => cmd_export(format, output, session),
    }
}

fn resolve_output(output: Option<String>) -> String {
    output.unwrap_or_else(|| {
        let config = ScaffConfig::load();
        config.output_dir.clone()
    })
}

fn resolve_provider(provider: Option<String>) -> String {
    provider.unwrap_or_else(|| {
        let config = ScaffConfig::load();
        config.default_provider.clone()
    })
}

fn resolve_model(model: Option<String>, cheap: bool) -> String {
    if cheap {
        return "gpt-4o-mini".to_string();
    }
    model.unwrap_or_else(|| {
        let config = ScaffConfig::load();
        config.model.clone()
    })
}

fn budget_alert() {
    let config = ScaffConfig::load();
    let mode = Mode::from_str(&config.api_mode);
    let tracker = TokenTracker::new();
    let monthly_used = tracker.monthly_tokens_used();
    let rl = RateLimiter::with_usage(mode, monthly_used);
    let pct = rl.budget_percent_used();

    if pct >= 90.0 {
        display::warn(&format!(
            "Budget critical: {:.0}% of monthly limit used ({} / {} tokens). Switch to 'min' mode.",
            pct, monthly_used, rl.config.monthly_tokens
        ));
    } else if pct >= 75.0 {
        display::warn(&format!(
            "Budget warning: {:.0}% of monthly limit used ({} / {} tokens). Consider 'scaff mode min'.",
            pct, monthly_used, rl.config.monthly_tokens
        ));
    } else if pct >= 50.0 {
        println!("{} Budget: {:.0}% used ({} / {} tokens this month)",
            console::style("ℹ").cyan(),
            pct, monthly_used, rl.config.monthly_tokens);
    }
}

fn check_api_key() -> Result<(), String> {
    match std::env::var("OPENAI_API_KEY") {
        Ok(k) if !k.is_empty() && !k.trim().is_empty() => Ok(()),
        _ => Err(
            "You need an OpenAI API key to use scaff.\n\n\
             1. Go to https://platform.openai.com/api-keys\n\
             2. Click 'Create new secret key'\n\
             3. Copy the key (it starts with 'sk-...')\n\
             4. Set it as an environment variable:\n\n\
             Mac / Linux:  export OPENAI_API_KEY='sk-...'\n\
             Windows:      $env:OPENAI_API_KEY = 'sk-...'".to_string(),
        ),
    }
}

fn archetype_map(archetype: &str, ui: &mut Option<String>, schedule: &mut Option<String>, memory: &mut Option<String>) {
    match archetype {
        "web-api" => { if ui.is_none() { *ui = Some("fastapi".to_string()); } }
        "chatbot" => { if ui.is_none() { *ui = Some("streamlit".to_string()); } }
        "scheduler" => { if schedule.is_none() { *schedule = Some("0 * * * *".to_string()); } }
        "memory" => { if memory.is_none() { *memory = Some("sqlite".to_string()); } }
        _ => {}
    }
}

fn cmd_create(
    description: String,
    output: Option<String>,
    model: Option<String>,
    verbose: bool,
    cheap: bool,
    show_cost: bool,
    no_cache: bool,
    _tokens: bool,
    provider: Option<String>,
    mut ui: Option<String>,
    mut schedule: Option<String>,
    mut memory: Option<String>,
    archetype: String,
) -> Result<(), String> {
    budget_alert();
    let provider = resolve_provider(Some(provider.unwrap_or_else(|| {
        ScaffConfig::load().default_provider.clone()
    })));

    if provider == "ollama" {
        // OK no key needed
    } else {
        check_api_key()?;
    }

    if let Some(ref u) = ui {
        if u != "fastapi" && u != "streamlit" {
            return Err(format!("Invalid UI mode: {u}. Valid: fastapi, streamlit"));
        }
    }

    archetype_map(&archetype, &mut ui, &mut schedule, &mut memory);

    let output = resolve_output(Some(output.unwrap_or_else(|| {
        ScaffConfig::load().output_dir.clone()
    })));

    let model = resolve_model(model, cheap);

    let output_path = Path::new(&output);
    if output_path.exists() {
        let overwrite = dialoguer::Confirm::new()
            .with_prompt("Directory exists. Overwrite?")
            .default(false)
            .interact()
            .unwrap_or(false);
        if !overwrite {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let spinner = display::create_spinner("Generating your agent...");
    let req = GenerateRequest {
        command: "generate_and_write".to_string(),
        description,
        model: Some(model),
        provider: Some(provider),
        output: Some(output.clone()),
        ui_mode: ui,
        schedule,
        memory,
        cheap,
        verbose,
        show_cost,
        no_cache,
    };
    let response = python_bridge::call_generate(&req)?;
    spinner.finish_and_clear();

    if response.status == "error" {
        return Err(response.error.unwrap_or_else(|| "Unknown error".to_string()));
    }

    display::ok("Agent scaffolded successfully!");
    println!();

    let files = response.created_files.unwrap_or_default();
    display::file_tree(Path::new(&output), &files);
    println!();

    println!("{}", console::style("Next steps:").bold());
    println!("  1. cd {output}");
    println!("  2. pip install -r requirements.txt");
    println!("  3. python agent.py");
    println!();
    println!("{}Tip: Try 'scaff run' to do all of this automatically!", console::style("").dim());

    Ok(())
}

fn cmd_run(
    description: String,
    output: Option<String>,
    model: Option<String>,
    cheap: bool,
    show_cost: bool,
    no_cache: bool,
    verbose: bool,
    _tokens: bool,
    provider: Option<String>,
    ui: Option<String>,
    schedule: Option<String>,
    memory: Option<String>,
) -> Result<(), String> {
    budget_alert();
    let provider = resolve_provider(Some(provider.unwrap_or_else(|| {
        ScaffConfig::load().default_provider.clone()
    })));

    if provider != "ollama" {
        check_api_key()?;
    }

    let output = resolve_output(output);
    let model = resolve_model(model, cheap);

    let spinner = display::create_spinner("Generating your agent...");
    let req = GenerateRequest {
        command: "generate_and_write".to_string(),
        description,
        model: Some(model),
        provider: Some(provider),
        output: Some(output.clone()),
        ui_mode: ui,
        schedule,
        memory,
        cheap,
        verbose,
        show_cost,
        no_cache,
    };
    let response = python_bridge::call_generate(&req)?;
    spinner.finish_and_clear();

    if response.status == "error" {
        return Err(response.error.unwrap_or_else(|| "Unknown error".to_string()));
    }

    display::ok("Agent generated!");
    println!();

    let files = response.created_files.unwrap_or_default();
    display::file_tree(Path::new(&output), &files);
    println!();

    // Install deps
    let req_file = Path::new(&output).join("requirements.txt");
    if req_file.exists() {
        println!("{} Installing dependencies...", console::style("*").dim());
        let status = std::process::Command::new("pip")
            .args(["install", "-r", &req_file.to_string_lossy()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| format!("Failed to run pip: {e}"))?;
        if status.success() {
            display::ok("Dependencies installed");
        } else {
            display::warn(&format!("Dependency install had issues. You may need to run: pip install -r {}", req_file.display()));
        }
    }

    println!();
    println!("{} Starting your agent!", console::style("Starting your agent!").cyan().bold());
    println!("{}", console::style("(Type your request when prompted, or press Ctrl+C to exit)").dim());
    println!();

    let agent_file = Path::new(&output).join("agent.py");
    if !agent_file.exists() {
        return Err("agent.py not found in output directory".to_string());
    }

    let status = std::process::Command::new("python")
        .arg(&agent_file)
        .current_dir(&output)
        .status()
        .map_err(|e| format!("Failed to run agent: {e}"))?;

    if !status.success() {
        return Err("Agent exited with error".to_string());
    }

    Ok(())
}

fn cmd_wizard() -> Result<(), String> {
    println!();
    println!("{}", console::style("Welcome to scaff!").cyan().bold());
    println!();
    println!("I will help you create your first AI agent.");
    println!("Just answer a few questions to get started.");
    println!();

    // Step 1: Check API key
    let key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    if key.trim().is_empty() {
        println!("{}", console::style("Step 1: Set up your OpenAI API key").bold());
        println!();
        println!(" 1. Go to https://platform.openai.com/api-keys");
        println!(" 2. Click 'Create new secret key'");
        println!(" 3. Copy the key (it starts with 'sk-...')");
        println!(" 4. Set it as an environment variable:");
        println!();
        println!("    Mac / Linux:  export OPENAI_API_KEY='sk-...'");
        println!("    Windows:      $env:OPENAI_API_KEY = 'sk-...'");
        println!();
        println!("{}", console::style("Set your API key and run 'scaff wizard' again.").yellow());
        return Ok(());
    }
    display::ok("OpenAI API key found!");
    println!();

    // Step 2: Description
    println!("{}", console::style("Step 2: What should your agent do?").bold());
    println!();
    println!("Describe what you want in plain English.");
    println!("For example:");
    println!("  * \"summarize my emails and flag urgent ones\"");
    println!("  * \"monitor github issues and auto-label them by priority\"");
    println!("  * \"check the weather and send me a daily forecast\"");
    println!();

    let description: String = dialoguer::Input::new()
        .with_prompt("Describe your agent")
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();

    let description = if description.trim().is_empty() {
        println!("{} Let me pick a popular example for you.", console::style("!").yellow());
        let desc = "summarize my emails and flag urgent ones";
        println!("Using: \"{desc}\"");
        println!();
        desc.to_string()
    } else {
        description
    };

    // Step 3: Output
    println!("{}", console::style("Step 3: Where should I save it?").bold());
    println!();
    let default_output = ScaffConfig::load().output_dir.clone();
    let output: String = dialoguer::Input::new()
        .with_prompt("Output directory")
        .default(default_output)
        .interact_text()
        .unwrap_or_else(|_| "./agent-output".to_string());
    println!();

    // Step 4: Provider
    println!("{}", console::style("Step 4: Choose your AI provider").bold());
    println!();
    println!("  1) OpenAI (GPT-4o - default)");
    println!("  2) Anthropic (Claude Sonnet 4)");
    println!("  3) Google (Gemini 2.0 Flash)");
    println!("  4) Ollama (Local - llama3.2, free, no API key needed)");
    println!();

    let provider_choice: String = dialoguer::Input::new()
        .with_prompt("Choose (1, 2, 3, or 4)")
        .default("1".to_string())
        .interact_text()
        .unwrap_or_else(|_| "1".to_string());

    let provider = match provider_choice.trim() {
        "2" => "anthropic",
        "3" => "gemini",
        "4" => "ollama",
        _ => "openai",
    };
    println!("Using: {provider}");
    println!();

    // Step 5: Run mode
    println!("{}", console::style("Step 5: How should we run it?").bold());
    println!();
    println!("  1) Generate only - save files, I will run them later");
    println!("  2) Generate and run - do everything now");
    println!();

    let run_mode: String = dialoguer::Input::new()
        .with_prompt("Choose (1 or 2)")
        .default("2".to_string())
        .interact_text()
        .unwrap_or_else(|_| "2".to_string());

    println!();

    // Execute
    let spinner = display::create_spinner("Generating your agent...");
    let req = GenerateRequest {
        command: "generate_and_write".to_string(),
        description,
        model: None,
        provider: Some(provider.to_string()),
        output: Some(output.clone()),
        ui_mode: None,
        schedule: None,
        memory: None,
        cheap: false,
        verbose: false,
        show_cost: false,
        no_cache: false,
    };
    let response = python_bridge::call_generate(&req)?;
    spinner.finish_and_clear();

    if response.status == "error" {
        return Err(response.error.unwrap_or_else(|| "Unknown error".to_string()));
    }

    display::ok("Your agent is ready!");
    println!();

    let files = response.created_files.unwrap_or_default();
    display::file_tree(Path::new(&output), &files);
    println!();

    if run_mode.trim() == "2" {
        let req_file = Path::new(&output).join("requirements.txt");
        if req_file.exists() {
            println!("{} Installing dependencies...", console::style("*").dim());
            let _ = std::process::Command::new("pip")
                .args(["install", "-r", &req_file.to_string_lossy()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        println!();
        println!("{} Starting your agent!", console::style("Starting your agent!").cyan().bold());
        println!("{}", console::style("(Type your request or press Ctrl+C to exit)").dim());
        println!();

        let agent_file = Path::new(&output).join("agent.py");
        let _ = std::process::Command::new("python")
            .arg(&agent_file)
            .current_dir(&output)
            .status();
    } else {
        println!("{}", console::style("Next steps:").bold());
        println!("  1. cd {output}");
        println!("  2. pip install -r requirements.txt");
        println!("  3. python agent.py");
        println!();
        println!("{}", console::style("Or just run: scaff run ...").dim());
        println!();
    }

    Ok(())
}

fn cmd_examples() -> Result<(), String> {
    println!();
    println!("{}", console::style("Ready-to-Use Example Agents").cyan().bold());
    println!();
    println!("Pick one and run the command shown:");
    println!();
    let examples = [
        ("Email assistant", "scaff run \"summarize my emails and flag urgent ones\""),
        ("Weather bot", "scaff run \"check the weather and tell me if I need an umbrella\""),
        ("GitHub helper", "scaff run \"monitor github issues and auto-label them by priority\""),
        ("Code reviewer", "scaff run \"analyze code for security issues\""),
        ("News curator", "scaff run \"aggregate news from multiple sources\""),
        ("Meeting assistant", "scaff run \"summarize meeting notes and extract action items\""),
    ];
    for (i, (name, cmd)) in examples.iter().enumerate() {
        println!("  {}. {} {}", i + 1, console::style(name).green().bold(), console::style(cmd).dim());
    }
    println!();
    Ok(())
}

fn cmd_validate(description: String) -> Result<(), String> {
    let spinner = display::create_spinner("Testing your description...");
    let req = GenerateRequest {
        command: "validate".to_string(),
        description,
        model: None,
        provider: None,
        output: None,
        ui_mode: None,
        schedule: None,
        memory: None,
        cheap: false,
        verbose: false,
        show_cost: false,
        no_cache: false,
    };
    let response = python_bridge::call_generate(&req)?;
    spinner.finish_and_clear();

    if response.status == "error" {
        return Err(response.error.unwrap_or_else(|| "Description validation failed".to_string()));
    }

    println!();
    if response.valid.unwrap_or(false) {
        display::ok("Description works!");
    } else {
        display::warn("Description may not work optimally");
    }
    println!();

    if let Some(name) = &response.agent_name {
        display::kv("Agent Name", name);
    }
    if let Some(desc) = &response.description {
        display::kv("Description", desc);
    }
    if let Some(count) = response.tool_count {
        display::kv("Tools", &format!("{count} tool(s)"));
    }
    if let Some(deps) = &response.dependencies {
        display::kv("Dependencies", &deps.join(", "));
    }
    println!();

    Ok(())
}

fn cmd_config(action: String, key: Option<String>, value: Option<String>) -> Result<(), String> {
    let mut config = ScaffConfig::load();

    match action.as_str() {
        "show" => {
            println!();
            println!("{}", console::style("Current Settings").cyan().bold());
            println!();
            println!("  {} = {}", console::style("model").cyan(), config.model);
            println!("  {} = {}", console::style("output_dir").cyan(), config.output_dir);
            println!("  {} = {}", console::style("default_provider").cyan(), config.default_provider);
            println!("  {} = {}", console::style("api_mode").cyan(), config.api_mode);
            println!("  {} = {}", console::style("cheap").cyan(), config.cheap);
            println!("  {} = {}", console::style("show_cost").cyan(), config.show_cost);
            println!("  {} = {}", console::style("template_style").cyan(), config.template_style);
            for (k, v) in &config.extra {
                println!("  {} = {}", console::style(k).cyan(), v);
            }
            println!();
            println!("Config file: {}", ScaffConfig::config_path().display());
            println!();
        }
        "set" => {
            match (key, value) {
                (Some(k), Some(v)) => {
                    let parsed: serde_json::Value = serde_json::from_str(&v).unwrap_or(Value::String(v.clone()));
                    config.set(&k, parsed);
                    config.save()?;
                    display::ok(&format!("Set {k} = {v}"));
                }
                _ => return Err("set requires --key and --value".to_string()),
            }
        }
        "reset" => {
            config.reset();
            config.save()?;
            display::ok("Reset to defaults");
        }
        _ => return Err(format!("Unknown action: {action}. Use: show, set, reset")),
    }

    Ok(())
}

fn cmd_cache(action: String) -> Result<(), String> {
    let cache = Cache::new();

    match action.as_str() {
        "status" => {
            let stats = cache.status();
            println!();
            println!("{}", console::style("Response Cache").cyan().bold());
            println!();
            display::kv("Entries", &stats.entries.to_string());
            display::kv("Size", &display::fmt_bytes(stats.size_bytes));
            display::kv("Oldest", &format!("{:.0} hours", stats.oldest_hours));
            println!();
            println!("  Cache dir: {}", ScaffConfig::config_dir().join("cache").display());
            println!();
        }
        "clear" => {
            let count = cache.clear();
            display::ok(&format!("Cleared {count} cached response(s)"));
            println!();
        }
        _ => return Err(format!("Unknown action: {action}. Use: status, clear")),
    }

    Ok(())
}

fn cmd_info() -> Result<(), String> {
    println!();
    println!("{}", console::style("scaff - AI Agent Project Scaffolder").cyan().bold());
    println!();
    display::kv("Version", env!("CARGO_PKG_VERSION"));
    display::kv("License", "MIT");
    println!();
    println!("{}", console::style("Features:").cyan().bold());
    println!("  * scaff create - generate a complete agent project");
    println!("  * scaff run - generate, install deps, and run in one command");
    println!("  * scaff wizard - guided setup for beginners");
    println!("  * scaff estimate - preview cost before generating");
    println!("  * scaff edit - modify existing agents without regeneration");
    println!("  * scaff shell - interactive REPL for iteratively building agents");
    println!("  * Web UI output (FastAPI or Streamlit --ui flag)");
    println!("  * Scheduled agent runs (--schedule flag)");
    println!("  * Multi-provider support: OpenAI, Anthropic, Gemini, Ollama");
    println!("  * Response cache to save API tokens");
    println!("  * Token bucket rate limiter (MIN/MEDIUM/MAX modes)");
    println!("  * Real-time monitoring dashboard");
    println!();
    println!("{}", console::style("Tech Stack:").cyan().bold());
    println!("  * Rust CLI (frontend)");
    println!("  * Python (generation backend)");
    println!("  * OpenAI / Anthropic / Gemini / Ollama (AI providers)");
    println!();
    println!("{}", console::style("Configuration:").cyan().bold());
    println!("  Config file: {}", ScaffConfig::config_path().display());
    println!();

    Ok(())
}

fn cmd_stats(month: Option<String>) -> Result<(), String> {
    let tracker = TokenTracker::new();
    let config = ScaffConfig::load();
    let mode = Mode::from_str(&config.api_mode);
    let monthly_used = tracker.monthly_tokens_used();
    let rl = RateLimiter::with_usage(mode, monthly_used);

    if let Some(ref m) = month {
        let parts: Vec<&str> = m.split('-').collect();
        if parts.len() == 2 {
            if let (Ok(year), Ok(month_num)) = (parts[0].parse::<i32>(), parts[1].parse::<u32>()) {
                let stats = tracker.get_monthly_stats_for(year, month_num);
                println!();
                println!("{}", console::style(format!("Token Usage — {m}")).bold());
                println!("  Total tokens: {}", stats.total_tokens);
                println!("  Total cost: ${:.2}", stats.total_cost);
                println!("  Sessions: {}", stats.session_count);
                println!();
                return Ok(());
            }
        }
        return Err(format!("Invalid month format: {m}. Use YYYY-MM"));
    }

    let monthly = tracker.get_monthly_stats();
    let all_time = tracker.get_all_time_stats();

    let budget_bar = budget_bar(rl.budget_percent_used());

    println!();
    println!("{}", console::style("This Month").cyan().bold());
    println!("  Tokens:        {}", monthly.total_tokens);
    println!("  Cost:          ${:.2}", monthly.total_cost);
    println!("  Sessions:      {}", monthly.session_count);
    println!("  Daily burn:    {} tokens/day (${:.4}/day)", monthly.daily_burn, monthly.daily_cost);
    println!();
    println!("{}", console::style("Projections").cyan().bold());
    println!("  Est. monthly:  {} tokens (${:.2})", monthly.projected_monthly, monthly.projected_cost);
    println!();
    println!("{}", console::style(format!("Budget ({})", mode.as_str().to_uppercase())).cyan().bold());
    println!("  Limit:         {} tokens (${:.2}/mo)", rl.config.monthly_tokens, rl.config.monthly_cost);
    println!("  Used:          {} tokens ({:.0}%)", monthly_used, rl.budget_percent_used());
    println!("  Remaining:     {} tokens", rl.remaining_budget());
    println!("  {budget_bar}");
    println!();

    println!("{}", console::style("All-Time").cyan().bold());
    println!("  Total tokens:  {}", all_time.total_tokens);
    println!("  Total cost:    ${:.2}", all_time.total_cost);
    println!("  Sessions:      {}", all_time.total_sessions);
    println!("  Avg cost/session: ${:.4}", all_time.average_cost_per_session);
    println!();

    Ok(())
}

fn budget_bar(pct: f64) -> String {
    let filled = ((pct / 100.0) * 20.0).round() as usize;
    let empty = 20usize.saturating_sub(filled);
    let bar_filled = console::style("▓".repeat(filled)).green();
    let bar_empty = console::style("░".repeat(empty)).dim();
    format!("[{}{}]", bar_filled, bar_empty)
}

fn cmd_mode(mode_name: String) -> Result<(), String> {
    let valid = ["min", "medium", "max"];
    if !valid.contains(&mode_name.as_str()) {
        return Err(format!("Invalid mode: {mode_name}. Valid: min, medium, max"));
    }

    let mut config = ScaffConfig::load();
    config.api_mode = mode_name.clone();
    config.save()?;

    println!();
    display::ok(&format!("Mode set to: {}", mode_name.to_uppercase()));
    println!();

    match mode_name.as_str() {
        "min" => {
            println!("  Monthly budget: $0.25");
            println!("  Monthly tokens: 1,000,000");
            println!("  Model: gpt-4o-mini");
            println!("  Rate limit: 6 req/min");
        }
        "medium" => {
            println!("  Monthly budget: $1.25");
            println!("  Monthly tokens: 5,000,000");
            println!("  Model: gpt-4o");
            println!("  Rate limit: 12 req/min");
        }
        "max" => {
            println!("  Monthly budget: $5.00");
            println!("  Monthly tokens: 20,000,000");
            println!("  Model: gpt-4o");
            println!("  Rate limit: 30 req/min");
        }
        _ => {}
    }
    println!();

    Ok(())
}

fn cmd_analytics(period: String, _model_filter: Option<String>) -> Result<(), String> {
    println!();
    println!("{}", console::style("Analytics & Performance").cyan().bold());
    println!();

    // In the hybrid approach, analytics data is tracked by the Python backend.
    // The Rust CLI will read from the shared token_tracker.json.
    let tracker = TokenTracker::new();
    let monthly = tracker.get_monthly_stats();
    let _all_time = tracker.get_all_time_stats();

    match period.as_str() {
        "daily" => {
            println!("{}", console::style("Today").bold());
            println!("  Total tokens: {}", monthly.total_tokens);
            println!("  Total cost: ${:.4}", monthly.total_cost);
        }
        "weekly" => {
            println!("{}", console::style("This Week").bold());
            println!("  Total tokens: {}", monthly.total_tokens);
            println!("  Total cost: ${:.4}", monthly.total_cost);
        }
        "monthly" => {
            println!("{}", console::style("This Month").bold());
            println!("  API calls: {}", monthly.session_count);
            println!("  Total tokens: {}", monthly.total_tokens);
            println!("  Total cost: ${:.4}", monthly.total_cost);
            if monthly.session_count > 0 {
                let avg = monthly.total_cost / monthly.session_count as f64;
                println!("  Avg cost/call: ${:.4}", avg);
            }
        }
        _ => return Err(format!("Invalid period: {period}. Valid: daily, weekly, monthly")),
    }
    println!();

    Ok(())
}

fn cmd_estimate(description: String, verbose: bool, _mode_filter: Option<String>) -> Result<(), String> {
    let spinner = display::create_spinner("Estimating costs...");
    let req = GenerateRequest {
        command: "estimate".to_string(),
        description,
        model: None,
        provider: None,
        output: None,
        ui_mode: None,
        schedule: None,
        memory: None,
        cheap: false,
        verbose,
        show_cost: false,
        no_cache: false,
    };
    let response = python_bridge::call_generate(&req)?;
    spinner.finish_and_clear();

    if response.status == "error" {
        return Err(response.error.unwrap_or_else(|| "Estimate failed".to_string()));
    }

    println!();
    if let Some(desc) = &response.description {
        println!("{}", console::style("Cost Estimate").cyan().bold());
        println!();
        println!("  Description: {desc}");
    }
    if let Some(tokens) = response.input_tokens {
        println!("  Full prompt: ~{tokens} tokens (incl. system prompt)");
    }
    println!();

    if let Some(modes_val) = response.modes {
        if let Some(modes) = modes_val.as_array() {
            println!("  {:<8} {:<20} {:<15} {:<15} {:<15}", "Mode", "Model", "Max Tokens", "Input Cost", "Est. Total");
            println!("  {}", "-".repeat(75));
            for m in modes {
                let mode_name = m["mode"].as_str().unwrap_or("?").to_uppercase();
                let model = m["model"].as_str().unwrap_or("?");
                let max_tok = m["max_tokens"].as_u64().unwrap_or(0);
                let input_cost = m["input_cost"].as_f64().unwrap_or(0.0);
                let total_cost = m["total_cost"].as_f64().unwrap_or(0.0);
                println!("  {:<8} {:<20} {:<15} ${:<14.5} ${:<14.5}", mode_name, model, max_tok, input_cost, total_cost);
            }
        }
    }
    println!();
    println!("{}", console::style("Estimates based on ~4 chars/token. Actual costs may vary.").dim());
    println!();

    Ok(())
}

fn cmd_errors(recent: bool) -> Result<(), String> {
    println!();
    println!("{}", console::style("Error Reports & Diagnostics").cyan().bold());
    println!();

    if recent {
        println!("{}", console::style("Recent errors: Check ~/.scaff/ for error logs").dim());
        println!("  (Full error diagnostics available via the Python backend)");
    } else {
        println!("  Error tracking is handled by the Python backend.");
        println!("  Run with -v for verbose error output.");
        println!();
        println!("{}", console::style("System Information").bold());
        println!("  Platform: {}", std::env::consts::OS);
        println!("  Arch: {}", std::env::consts::ARCH);
        println!("  Rust CLI version: {}", env!("CARGO_PKG_VERSION"));
    }
    println!();

    Ok(())
}

fn cmd_state(reset: bool) -> Result<(), String> {
    if reset {
        let mut config = ScaffConfig::load();
        config.reset();
        config.save()?;
        display::ok("Application state reset to defaults");
        println!();
        return Ok(());
    }

    let config = ScaffConfig::load();
    let tracker = TokenTracker::new();
    let monthly = tracker.get_monthly_stats();

    println!();
    println!("{}", console::style("Application State").cyan().bold());
    println!();
    println!("{}", console::style("Configuration").bold());
    println!("  Current mode: {}", config.api_mode);
    println!("  Default model: {}", config.model);
    println!("  Default provider: {}", config.default_provider);
    println!("  Output directory: {}", config.output_dir);
    println!();
    println!("{}", console::style("Usage").bold());
    println!("  Tokens used this month: {}", monthly.total_tokens);
    println!("  Cost this month: ${:.2}", monthly.total_cost);
    println!("  Sessions: {}", monthly.session_count);
    println!();

    Ok(())
}

fn cmd_edit(
    _dir: String,
    _description: Option<String>,
    _system_prompt: Option<String>,
    _provider: Option<String>,
    _ui: Option<String>,
    _schedule: Option<String>,
    _verbose: bool,
) -> Result<(), String> {
    println!("[{}] Edit functionality bridges to Python backend.", console::style("*").dim());
    println!("  Run: scaff create with updated options, or modify spec.json directly.");
    println!();
    println!("  For now, the Python backend handles editing.");
    println!("  Install the Python version for full edit support.");
    println!();

    Ok(())
}

fn cmd_deploy(_dir: String, _platform: Option<String>, _verbose: bool) -> Result<(), String> {
    println!("[{}] Deploy functionality bridges to Python backend.", console::style("*").dim());
    println!("  Use the Python CLI for full deploy support.");
    println!();

    Ok(())
}

fn cmd_test(_prompt: Vec<String>, _dir: String, _prompts: Option<String>, _verbose: bool) -> Result<(), String> {
    println!("[{}] Test functionality bridges to Python backend.", console::style("*").dim());
    println!("  Use the Python CLI for full test support.");
    println!();

    Ok(())
}

fn cmd_upgrade(_dir: String, _provider: Option<String>, _dry_run: bool, _verbose: bool) -> Result<(), String> {
    println!("[{}] Upgrade functionality bridges to Python backend.", console::style("*").dim());
    println!("  For now, use the Python CLI for upgrade support.");
    println!();

    Ok(())
}

fn cmd_monitor(refresh: u64, once: bool) -> Result<(), String> {
    crate::monitor::run_dashboard(refresh, once)
}

fn cmd_completions(shell: String) -> Result<(), String> {
    use clap::CommandFactory;
    let mut cmd = crate::Cli::command();
    let shell = match shell.as_str() {
        "bash" => clap_complete::Shell::Bash,
        "zsh" => clap_complete::Shell::Zsh,
        "fish" => clap_complete::Shell::Fish,
        "powershell" => clap_complete::Shell::PowerShell,
        "elvish" => clap_complete::Shell::Elvish,
        _ => return Err(format!("Unknown shell: {shell}. Valid: bash, zsh, fish, powershell, elvish")),
    };
    let mut stdout = std::io::stdout();
    clap_complete::generate(shell, &mut cmd, "scaff", &mut stdout);
    Ok(())
}

fn cmd_export(format: String, output: Option<String>, session: Option<String>) -> Result<(), String> {
    let tracker = TokenTracker::new();

    let data = if let Some(ref sid) = session {
        tracker.export_csv_for_session(sid)?
    } else {
        match format.as_str() {
            "json" => tracker.export_json()?,
            "csv" => tracker.export_csv()?,
            _ => return Err(format!("Invalid format: {format}. Valid: json, csv")),
        }
    };

    match output {
        Some(path) => {
            fs::write(&path, &data)
                .map_err(|e| format!("Failed to write export file: {e}"))?;
            display::ok(&format!("Exported token data to {path}"));
        }
        None => {
            println!("{data}");
        }
    }

    Ok(())
}
