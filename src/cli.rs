use std::io::Write;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use console::style;

use crate::agent::{self, AgentOptions, UseCase};
use crate::config::{self, ScaffConfig};
use crate::connectors;
use crate::corpus;
use crate::display;
use crate::history;
use crate::init;
use crate::mcp;
use crate::pipeline::{self, Execution, Source, SourceKind};
use crate::repl;
use crate::search::Index;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "scaff",
    version,
    about = "Pi-simple Harness research agent — cited answers, four tools, one loop.",
    long_about = "scaff is a production research CLI for the Harness platform.\n\
                  Tiny agent loop (like Pi): short prompt, four tools, cited report.\n\
                  \n  \
                  Quick start:  scaff \"What is Harness Continuous Delivery?\"\n  \
                  Compare:      scaff compare \"canary vs blue/green in Harness\"\n  \
                  How-to:       scaff howto \"set up a CD pipeline\"\n  \
                  Setup:        scaff setup\n  \
                  Interactive:  scaff chat\n  \
                  Diagnostics:  scaff doctor\n\
                  \n  \
                  Tools: search_corpus · web_search · fetch_url · finish\n\
                  Providers: OpenAI, Anthropic, Gemini, Groq, Ollama (API keys from env).",
    subcommand_required = false,
    arg_required_else_help = false
)]
pub struct Cli {
    /// Provider: openai, anthropic, gemini, groq, ollama.
    #[arg(short = 'p', long, global = true)]
    pub provider: Option<String>,

    /// Override the model name.
    #[arg(short = 'm', long, global = true)]
    pub model: Option<String>,

    /// Use the cheap / fast model tier.
    #[arg(short = 'c', long, global = true)]
    pub cheap: bool,

    /// Show estimated LLM cost after the report is generated.
    #[arg(long, global = true)]
    pub show_cost: bool,

    /// Show agent tool steps as they run (search_corpus → finish).
    #[arg(long, global = true)]
    pub stages: bool,

    /// Show the source list after the report body.
    #[arg(long, global = true)]
    pub show_sources: bool,

    /// Number of corpus chunks to retrieve per search.
    #[arg(short = 'k', long, global = true)]
    pub retrieval_k: Option<usize>,

    /// Disable web fallback.
    #[arg(long, global = true)]
    pub no_web: bool,

    /// Write the report to this file instead of stdout.
    #[arg(short = 'o', long, global = true)]
    pub output: Option<PathBuf>,

    /// Verbose logging.
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    /// The research question (when no subcommand is given).
    pub question: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Product Q&A with citations (default). Alias: `scaff "..."`.
    Ask {
        /// The research question.
        question: Vec<String>,
    },
    /// Structured comparison (A vs B, Harness vs X, canary vs blue/green).
    Compare { question: Vec<String> },
    /// Steps, setup, or troubleshooting guidance.
    Howto { question: Vec<String> },
    /// Alias for `ask` (compat).
    Research {
        /// The research question.
        question: Vec<String>,
    },
    /// Start the interactive REPL with multi-turn research and slash commands.
    Chat,
    /// Show past research executions.
    History {
        /// Number of recent executions to show.
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// Reprint a saved research report by execution id (or `:n` index from history).
    Show { id: String },
    /// Manage data sources (corpus, web, local files).
    Connector {
        #[command(subcommand)]
        action: ConnectorAction,
    },
    /// Initialize a .scaff.yaml project file in the current directory.
    Init,
    /// Manage the local corpus.
    Corpus {
        #[command(subcommand)]
        action: CorpusAction,
    },
    /// View or change configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Health check: API keys, corpus, connectors, dependencies.
    Doctor,
    /// Interactive first-time setup wizard.
    Setup,
    /// Start an MCP stdio server exposing the `research` tool.
    Mcp,
    /// Generate shell completions.
    Completions { shell: String },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConnectorAction {
    /// List configured connectors and their status.
    List,
    /// Test a connector by name.
    Test { name: String },
    /// Add a local directory of markdown/text files as a connector.
    AddDir { path: PathBuf },
}

#[derive(Subcommand, Debug, Clone)]
pub enum CorpusAction {
    /// Show corpus statistics.
    Status,
    /// Add chunks from a local JSONL file.
    Add { path: PathBuf },
    /// Add chunks from a local directory of .md/.txt/.rst/.adoc files.
    AddDir { path: PathBuf },
    /// Crawl a URL and add its content to the corpus.
    Crawl {
        url: String,
        #[arg(long)]
        source: Option<String>,
    },
    /// Download a fresh seed corpus from a URL.
    Update {
        #[arg(long)]
        url: Option<String>,
    },
    /// Remove all chunks from the local corpus.
    Clear,
    /// Run a search against the corpus and print the top hits.
    Search {
        query: Vec<String>,
        #[arg(short = 'k', long, default_value_t = 5usize)]
        k: usize,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigAction {
    Show,
    Set { key: String, value: String },
    SetKey { provider: String, key: String },
    Reset,
}

pub fn execute(cli: Cli) -> Result<(), String> {
    // Auto-setup: detect provider, seed corpus on first run
    if !matches!(&cli.command, Some(Commands::Completions { .. })) {
        init::auto_setup(true).ok();
    }

    if cli.command.is_none() {
        if let Some(q) = cli.question.as_ref() {
            if q.trim().is_empty() {
                return cmd_chat();
            }
            return cmd_agent(&cli, q, UseCase::Ask);
        }
        return cmd_chat();
    }

    match cli.command.as_ref().unwrap() {
        Commands::Ask { question } => {
            let q = question.join(" ");
            if q.is_empty() {
                return Err("Usage: scaff ask <question>".to_string());
            }
            cmd_agent(&cli, &q, UseCase::Ask)
        }
        Commands::Compare { question } => {
            let q = question.join(" ");
            if q.is_empty() {
                return Err("Usage: scaff compare <question>".to_string());
            }
            cmd_agent(&cli, &q, UseCase::Compare)
        }
        Commands::Howto { question } => {
            let q = question.join(" ");
            if q.is_empty() {
                return Err("Usage: scaff howto <question>".to_string());
            }
            cmd_agent(&cli, &q, UseCase::Howto)
        }
        Commands::Research { question } => {
            let q = question.join(" ");
            if q.is_empty() {
                return Err("Usage: scaff research <question>".to_string());
            }
            cmd_agent(&cli, &q, UseCase::Ask)
        }
        Commands::Chat => cmd_chat(),
        Commands::History { limit } => cmd_history(*limit),
        Commands::Show { id } => cmd_show(id),
        Commands::Connector { action } => cmd_connector(action.clone()),
        Commands::Init => cmd_init(),
        Commands::Corpus { action } => cmd_corpus(action.clone()),
        Commands::Config { action } => cmd_config(action.clone()),
        Commands::Setup => cmd_setup(),
        Commands::Doctor => cmd_doctor(),
        Commands::Mcp => {
            mcp::run().map_err(|e| format!("MCP server error: {e}"))?;
            Ok(())
        }
        Commands::Completions { shell } => cmd_completions(shell),
    }
}

fn cmd_chat() -> Result<(), String> {
    if !ScaffConfig::config_path().exists() {
        init::print_welcome();
    }
    repl::run().map_err(|e| format!("chat error: {e}"))
}

fn cmd_history(limit: usize) -> Result<(), String> {
    let execs = history::list_executions(limit).map_err(|e| format!("history: {e}"))?;
    if execs.is_empty() {
        println!(
            "\n  {} no executions yet — try: scaff \"what is Harness CD?\"",
            style("!").yellow()
        );
        return Ok(());
    }
    display::section("Execution history");
    for (i, e) in execs.iter().enumerate() {
        let status = match e.status {
            pipeline::ExecutionStatus::Succeeded => style("ok").green(),
            pipeline::ExecutionStatus::Failed => style("fail").red(),
            pipeline::ExecutionStatus::Running => style("...").yellow(),
        };
        let when = e.started_at.format("%Y-%m-%d %H:%M").to_string();
        let model = if e.model.is_empty() {
            "?".to_string()
        } else {
            e.model.clone()
        };
        println!(
            "  {:>3}. [{}] {}  {}  {}  ${:.4}  {}",
            i + 1,
            status,
            style(&e.id).dim(),
            style(when).dim(),
            style(&model).cyan(),
            e.estimated_cost_usd,
            truncate(&e.question, 60)
        );
    }
    println!(
        "\n  {} run `scaff show <id>` to reprint a report",
        style("→").dim()
    );
    Ok(())
}

fn cmd_show(id: &str) -> Result<(), String> {
    let id = id.trim_start_matches(':');
    if let Ok(idx) = id.parse::<usize>() {
        let execs = history::list_executions(100).map_err(|e| format!("history: {e}"))?;
        if let Some(e) = execs.get(idx.saturating_sub(1)) {
            print_execution_report(e);
            return Ok(());
        }
        return Err(format!("no execution at index {idx}"));
    }
    let exec = history::load_execution(id).map_err(|e| format!("load: {e}"))?;
    print_execution_report(&exec);
    Ok(())
}

fn print_execution_report(exec: &Execution) {
    if let Some(p) = &exec.report_path {
        if let Ok(body) = std::fs::read_to_string(p) {
            print!("{body}");
            return;
        }
    }
    // Fallback: print what we have.
    if let Some(t) = &exec.tldr {
        println!("\n## TL;DR\n\n{t}\n");
    }
    if let Some(err) = &exec.error {
        eprintln!("\n  {} {err}", style("ERROR").red());
    }
}

fn cmd_connector(action: ConnectorAction) -> Result<(), String> {
    match action {
        ConnectorAction::List => {
            display::section("Connectors");
            for mut c in connectors::list_default_connectors() {
                let (status, detail) = connectors::test_connector(&c.name)
                    .unwrap_or((connectors::ConnectorStatus::Unknown, serde_json::json!({})));
                c.status = status;
                c.details = detail;
                c.last_tested = Some(chrono::Utc::now());
                let badge = match c.status {
                    connectors::ConnectorStatus::Healthy => style("HEALTHY").green(),
                    connectors::ConnectorStatus::Degraded => style("DEGRADED").yellow(),
                    connectors::ConnectorStatus::Failed => style("FAILED").red(),
                    connectors::ConnectorStatus::Unknown => style("UNKNOWN").dim(),
                };
                let kind = match c.kind {
                    connectors::ConnectorKind::Corpus => "corpus",
                    connectors::ConnectorKind::Web => "web",
                    connectors::ConnectorKind::Local => "local",
                };
                println!(
                    "  {}  [{}]  {} — {}",
                    badge,
                    style(kind).cyan(),
                    style(&c.name).bold(),
                    c.description
                );
                if !c.details.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                    println!("        {}", style(c.details.to_string()).dim());
                }
            }
            Ok(())
        }
        ConnectorAction::Test { name } => {
            let (status, detail) = connectors::test_connector(&name)
                .ok_or_else(|| format!("unknown connector: {name}"))?;
            let badge = match status {
                connectors::ConnectorStatus::Healthy => style("HEALTHY").green(),
                connectors::ConnectorStatus::Degraded => style("DEGRADED").yellow(),
                connectors::ConnectorStatus::Failed => style("FAILED").red(),
                connectors::ConnectorStatus::Unknown => style("UNKNOWN").dim(),
            };
            println!("  {badge}  {name}\n  {}", style(detail.to_string()).dim());
            Ok(())
        }
        ConnectorAction::AddDir { path } => {
            if !path.exists() {
                return Err(format!("path does not exist: {}", path.display()));
            }
            let spinner = display::create_spinner(&format!("Indexing {}...", path.display()));
            let n = crate::local_files::add_directory_to_corpus(&path)
                .map_err(|e| format!("index: {e}"))?;
            spinner.finish_and_clear();
            display::ok(&format!("Indexed {n} chunks from {}", path.display()));
            Ok(())
        }
    }
}

fn cmd_init() -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
    init::init_project(&cwd).map_err(|e| format!("init: {e}"))?;
    display::ok(&format!(
        "Created {} in {}",
        init::SCAFF_PROJECT_FILE,
        cwd.display()
    ));
    display::kv(
        "next",
        "edit .scaff.yaml, then run `scaff connector add-dir ./docs` to index your local docs",
    );
    Ok(())
}

fn cmd_agent(cli: &Cli, question: &str, use_case: UseCase) -> Result<(), String> {
    let question = question.trim();
    if question.is_empty() || question.len() < 3 {
        return Err(
            "Question must be at least 3 characters. Try: scaff \"what is Harness CD?\"".into(),
        );
    }
    if question.len() > 5000 {
        return Err(format!(
            "Question is too long ({} chars, max 5000)",
            question.len()
        ));
    }

    let cfg = ScaffConfig::load();
    let provider = config::resolve_provider(cli.provider.as_deref())?;
    let api_key = config::require_api_key(provider, &cfg)?;
    let model = config::resolve_model(
        provider,
        cli.model.as_deref().or(Some(&cfg.model)),
        cli.cheap || cfg.cheap,
    );

    let conn = corpus::open_db().map_err(|e| format!("open corpus: {e}"))?;
    let chunks = corpus::load_all_chunks(&conn).map_err(|e| format!("load corpus: {e}"))?;
    let index = Index::build(chunks.clone());

    if chunks.is_empty() {
        eprintln!(
            "  {} corpus is empty — agent will rely on web (if enabled) and model knowledge",
            style("warn").yellow()
        );
    }

    let options = AgentOptions {
        use_case,
        retrieval_k: cli.retrieval_k.unwrap_or(cfg.retrieval_k).max(1),
        web_enabled: !cli.no_web && cfg.web_enabled,
        model: model.clone(),
        temperature: 0.2,
        max_output_tokens: 2048,
        max_steps: 8,
        show_stages: cli.stages || cli.verbose,
        show_cost: cli.show_cost || cfg.show_cost,
    };

    let spinner_msg = if options.show_stages {
        format!(
            "{} · search_corpus → web_search → fetch_url → finish",
            use_case.as_str()
        )
    } else {
        format!("{}: \"{question}\"...", use_case.as_str())
    };
    let spinner = display::create_spinner(&spinner_msg);
    let result = agent::run(question, provider, Some(&api_key), &index, &options)
        .map_err(|e| format!("agent failed: {e}"))?;
    spinner.finish_and_clear();

    let mut exec = Execution::new(use_case.pipeline_name(), question);
    exec.model = model.clone();
    exec.provider = provider.name.to_string();
    exec.input_tokens = result.stats.input_tokens;
    exec.output_tokens = result.stats.output_tokens;
    exec.estimated_cost_usd = result.stats.estimated_cost_usd;
    exec.finished_at = Some(chrono::Utc::now());
    exec.status = pipeline::ExecutionStatus::Succeeded;
    exec.tldr = Some(result.payload.tldr.clone());

    for line in &result.steps_log {
        let mut stage = pipeline::Stage::new("tool");
        stage.status = pipeline::StageStatus::Ok;
        stage.finished_at = Some(chrono::Utc::now());
        stage = stage.detail("step", line.clone());
        exec.stages.push(stage);
    }
    let mut finish_stage = pipeline::Stage::new("finish");
    finish_stage.status = pipeline::StageStatus::Ok;
    finish_stage.duration_ms = result.stats.elapsed_ms;
    finish_stage.finished_at = Some(chrono::Utc::now());
    finish_stage = finish_stage.detail("findings", result.payload.findings.len() as u64);
    finish_stage = finish_stage.detail("tool_calls", result.stats.tool_calls as u64);
    exec.stages.push(finish_stage);

    let mut sources: Vec<Source> = Vec::new();
    for (i, s) in result.payload.sources.iter().enumerate() {
        let kind = if s.kind == "web" {
            SourceKind::Web
        } else {
            SourceKind::Corpus
        };
        sources.push(Source {
            id: i + 1,
            title: s.title.clone(),
            url: s.url.clone(),
            kind,
            score: 0.0,
        });
    }
    exec.sources = sources;

    exec.claims = result
        .payload
        .findings
        .iter()
        .enumerate()
        .map(|(i, f)| pipeline::Claim {
            id: i,
            text: f.text.clone(),
            source_ids: Vec::new(),
            confidence: match f.confidence {
                agent::Confidence::High => pipeline::Confidence::High,
                agent::Confidence::Medium => pipeline::Confidence::Medium,
                agent::Confidence::Low => pipeline::Confidence::Low,
                agent::Confidence::Contested => pipeline::Confidence::Contested,
            },
            conflict_note: f.conflict_note.clone(),
        })
        .collect();

    let saved_report = history::save_report(&exec, &result.report).ok();
    if let Some(p) = &saved_report {
        exec.report_path = Some(p.display().to_string());
    }
    history::save_execution(&exec).ok();

    if options.show_stages {
        eprintln!();
        eprintln!("  {}", style("Agent steps").cyan().bold());
        for line in &result.steps_log {
            eprintln!("  {} {}", style("→").dim(), line);
        }
        eprintln!(
            "  {}  {} — {} findings, {} tool calls, {} ms",
            style("✓").green(),
            style(&exec.id).dim(),
            result.payload.findings.len(),
            result.stats.tool_calls,
            result.stats.elapsed_ms
        );
        eprintln!();
    }

    if cli.show_sources && !result.payload.sources.is_empty() {
        eprintln!();
        eprintln!("  {}", style("Sources").cyan().bold());
        for s in &result.payload.sources {
            eprintln!(
                "    {}  {}  {}",
                style(&s.id).dim(),
                style(&s.title).bold(),
                style(&s.url).dim()
            );
        }
    }

    if let Some(path) = &cli.output {
        std::fs::write(path, &result.report)
            .map_err(|e| format!("write {}: {e}", path.display()))?;
        display::ok(&format!("Wrote report to {}", path.display()));
    } else {
        print!("{}", result.report);
    }

    if options.show_cost {
        eprintln!();
        display::section("Stats");
        display::kv("use case", use_case.as_str());
        display::kv("model", &options.model);
        display::kv("provider", provider.name);
        display::kv("steps", &result.stats.steps.to_string());
        display::kv("tool calls", &result.stats.tool_calls.to_string());
        display::kv("corpus searches", &result.stats.corpus_searches.to_string());
        display::kv("web searches", &result.stats.web_searches.to_string());
        display::kv("fetches", &result.stats.fetches.to_string());
        display::kv("findings", &result.payload.findings.len().to_string());
        display::kv("elapsed", &format!("{} ms", result.stats.elapsed_ms));
        display::kv(
            "tokens (in/out)",
            &format!(
                "{} / {}",
                result.stats.input_tokens, result.stats.output_tokens
            ),
        );
        display::kv(
            "est. cost",
            &format!("${:.6}", result.stats.estimated_cost_usd),
        );
    }
    Ok(())
}

fn cmd_corpus(action: CorpusAction) -> Result<(), String> {
    match action {
        CorpusAction::Status => {
            let conn = corpus::open_db().map_err(|e| format!("open corpus: {e}"))?;
            let s = corpus::stats(&conn).map_err(|e| format!("stats: {e}"))?;
            let path = corpus::db_path();
            display::section("Corpus");
            display::kv("database", &path.display().to_string());
            display::kv("chunks", &s.total.to_string());
            display::kv("unique URLs", &s.unique_urls.to_string());
            display::kv("unique sources", &s.unique_sources.to_string());
            display::kv("size", &display::fmt_bytes(s.total_bytes));
            Ok(())
        }
        CorpusAction::Add { path } => {
            let conn = corpus::open_db().map_err(|e| format!("open corpus: {e}"))?;
            let n = corpus::ingest_seed_path(&conn, &path).map_err(|e| format!("ingest: {e}"))?;
            display::ok(&format!("Ingested {n} chunks from {}", path.display()));
            Ok(())
        }
        CorpusAction::AddDir { path } => {
            if !path.exists() {
                return Err(format!("path does not exist: {}", path.display()));
            }
            let spinner = display::create_spinner(&format!("Indexing {}...", path.display()));
            let n = crate::local_files::add_directory_to_corpus(&path)
                .map_err(|e| format!("index: {e}"))?;
            spinner.finish_and_clear();
            display::ok(&format!("Indexed {n} chunks from {}", path.display()));
            Ok(())
        }
        CorpusAction::Crawl { url, source } => {
            let spinner = display::create_spinner(&format!("Crawling {url}..."));
            let source = source.unwrap_or_else(|| "crawl".to_string());
            let chunks = corpus::crawl_url(&url, &source).map_err(|e| format!("crawl: {e}"))?;
            spinner.finish_and_clear();
            let conn = corpus::open_db().map_err(|e| format!("open corpus: {e}"))?;
            let n = corpus::ingest_raw(&conn, &chunks).map_err(|e| format!("ingest: {e}"))?;
            display::ok(&format!("Added {n} chunks from {url}"));
            Ok(())
        }
        CorpusAction::Update { url } => {
            let url = url.unwrap_or_else(|| corpus::DEFAULT_SEED_URL.to_string());
            let spinner = display::create_spinner(&format!("Downloading seed from {url}..."));
            let dest = corpus::seed_path();
            let n = corpus::download_seed(&url, &dest).map_err(|e| format!("download: {e}"))?;
            spinner.finish_and_clear();
            display::ok(&format!("Ingested {n} chunks from {url}"));
            Ok(())
        }
        CorpusAction::Clear => {
            let conn = corpus::open_db().map_err(|e| format!("open corpus: {e}"))?;
            let n = corpus::clear(&conn).map_err(|e| format!("clear: {e}"))?;
            display::ok(&format!("Removed {n} chunks"));
            Ok(())
        }
        CorpusAction::Search { query, k } => {
            let q = query.join(" ");
            if q.is_empty() {
                return Err("Usage: scaff corpus search <query>".to_string());
            }
            let conn = corpus::open_db().map_err(|e| format!("open corpus: {e}"))?;
            let chunks = corpus::load_all_chunks(&conn).map_err(|e| format!("load corpus: {e}"))?;
            let index = Index::build(chunks);
            let hits = index.search(&q, k);
            display::section(&format!("Top {k} hits for \"{q}\""));
            for (i, h) in hits.iter().enumerate() {
                println!(
                    "  {}. score={:.3}  {}",
                    i + 1,
                    h.score,
                    display_kv_color(&h.chunk.title, &h.chunk.url)
                );
                if let Some(s) = &h.chunk.section {
                    println!("     section: {s}");
                }
                let snippet = snippet(&h.chunk.content, 160);
                println!("     {snippet}");
                println!();
            }
            Ok(())
        }
    }
}

fn cmd_config(action: ConfigAction) -> Result<(), String> {
    match action {
        ConfigAction::Show => {
            let cfg = ScaffConfig::load();
            display::section("Configuration");
            display::kv("model", &cfg.model);
            display::kv("default_provider", &cfg.default_provider);
            display::kv("retrieval_k", &cfg.retrieval_k.to_string());
            display::kv("web_enabled", &cfg.web_enabled.to_string());
            display::kv("show_cost", &cfg.show_cost.to_string());
            display::kv("cheap", &cfg.cheap.to_string());
            display::kv("corpus_dir", &cfg.corpus_dir);
            display::kv(
                "stored API keys",
                &format!("{} provider(s)", cfg.api_keys.len()),
            );
            display::kv(
                "config file",
                &ScaffConfig::config_path().display().to_string(),
            );
            display::kv(
                "corpus DB",
                &ScaffConfig::corpus_db_path().display().to_string(),
            );
            Ok(())
        }
        ConfigAction::Set { key, value } => {
            let mut cfg = ScaffConfig::load();
            let value_for_msg = value.clone();
            match key.as_str() {
                "model" => cfg.model = value,
                "default_provider" => {
                    config::resolve_provider(Some(&value))?;
                    cfg.default_provider = value;
                }
                "retrieval_k" => {
                    let n: usize = value.parse().map_err(|_| "retrieval_k must be a number")?;
                    cfg.retrieval_k = n;
                }
                "web_enabled" => cfg.web_enabled = parse_bool(&value)?,
                "show_cost" => cfg.show_cost = parse_bool(&value)?,
                "cheap" => cfg.cheap = parse_bool(&value)?,
                "corpus_dir" => cfg.corpus_dir = value,
                other => return Err(format!("Unknown config key: {other}")),
            }
            cfg.save()?;
            display::ok(&format!("Set {key} = {value_for_msg}"));
            Ok(())
        }
        ConfigAction::SetKey { provider: p, key } => {
            let provider = config::resolve_provider(Some(&p))?;
            let mut cfg = ScaffConfig::load();
            cfg.set_api_key(provider.name, key);
            cfg.save()?;
            display::ok(&format!("Stored API key for {}", provider.name));
            Ok(())
        }
        ConfigAction::Reset => {
            ScaffConfig::default().save()?;
            display::ok("Reset configuration to defaults");
            Ok(())
        }
    }
}

fn cmd_doctor() -> Result<(), String> {
    display::section("scaff doctor");
    display::kv("version", env!("CARGO_PKG_VERSION"));
    display::kv(
        "config file",
        &ScaffConfig::config_path().display().to_string(),
    );

    let cfg = ScaffConfig::load();
    let provider = config::resolve_provider(Some(&cfg.default_provider))?;
    display::kv("provider", provider.name);
    match config::resolve_api_key(provider, &cfg) {
        Ok(Some(_)) => display::kv("API key", "found"),
        Ok(None) if !provider.requires_key => display::kv("API key", "not required"),
        Ok(None) => {
            display::kv("API key", "MISSING");
            eprintln!(
                "\n  Set it via env var {} or: scaff config set-key {} <KEY>",
                provider.env_var, provider.name
            );
        }
        Err(e) => display::kv("API key", &format!("error: {e}")),
    }

    display::kv("---", "---");

    // Connectors
    for mut c in connectors::list_default_connectors() {
        let (status, detail) = connectors::test_connector(&c.name)
            .unwrap_or((connectors::ConnectorStatus::Unknown, serde_json::json!({})));
        c.status = status;
        c.details = detail;
        let badge = match c.status {
            connectors::ConnectorStatus::Healthy => style("HEALTHY").green(),
            connectors::ConnectorStatus::Degraded => style("DEGRADED").yellow(),
            connectors::ConnectorStatus::Failed => style("FAILED").red(),
            connectors::ConnectorStatus::Unknown => style("UNKNOWN").dim(),
        };
        let kind = match c.kind {
            connectors::ConnectorKind::Corpus => "corpus",
            connectors::ConnectorKind::Web => "web",
            connectors::ConnectorKind::Local => "local",
        };
        println!(
            "  connector [{}] {} — {} {}",
            kind, c.name, badge, c.details
        );
    }

    display::ok("doctor complete");
    Ok(())
}

fn cmd_setup() -> Result<(), String> {
    init::run_setup_wizard().map_err(|e| format!("setup: {e}"))
}

fn cmd_completions(shell: &str) -> Result<(), String> {
    let mut app = Cli::command();
    let shell_type = match shell {
        "bash" => clap_complete::Shell::Bash,
        "zsh" => clap_complete::Shell::Zsh,
        "fish" => clap_complete::Shell::Fish,
        "powershell" => clap_complete::Shell::PowerShell,
        "elvish" => clap_complete::Shell::Elvish,
        _ => {
            return Err(format!(
                "Unknown shell: {shell}. Valid: bash, zsh, fish, powershell, elvish"
            ))
        }
    };
    let mut buf: Vec<u8> = Vec::new();
    clap_complete::generate(shell_type, &mut app, "scaff", &mut buf);
    std::io::stdout()
        .write_all(&buf)
        .map_err(|e| format!("{e}"))?;
    Ok(())
}

fn display_kv_color(k: &str, v: &str) -> String {
    format!("{} \u{2192} {}", style(k).bold(), v)
}

fn snippet(s: &str, max: usize) -> String {
    let collapsed: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.len() <= max {
        collapsed
    } else {
        format!("{}\u{2026}", &collapsed[..max])
    }
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "1" | "true" | "yes" | "y" | "on" => Ok(true),
        "0" | "false" | "no" | "n" | "off" => Ok(false),
        _ => Err(format!("expected bool, got: {s}")),
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}

#[allow(dead_code)]
const _VERSION: &str = env!("CARGO_PKG_VERSION");
