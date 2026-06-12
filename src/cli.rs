use std::io::Write;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use console::style;

use crate::config::{self, ScaffConfig};
use crate::connectors;
use crate::corpus;
use crate::display;
use crate::history;
use crate::init;
use crate::mcp;
use crate::pipeline::{self, Execution, Source, SourceKind};
use crate::render;
use crate::repl;
use crate::research::{self, ResearchOptions};
use crate::search::Index;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "scaff",
    version,
    about = "Harness Research Agent — cited answers about the Harness platform.",
    long_about = "scaff is a verticalized deep-research CLI for the Harness platform. \
                  Ask a question, get a cited Markdown report.\n\
                  \n  \
                  Quick start:  scaff \"What is Harness Continuous Delivery?\"\n  \
                  Setup wizard: scaff setup\n  \
                  Interactive:   scaff chat\n  \
                  Diagnostics:   scaff doctor\n\
                  \n  \
                  Supports OpenAI, Anthropic, Gemini, Groq, and Ollama providers. \
                  API keys auto-detected from environment variables.",
    subcommand_required = false,
    arg_required_else_help = false,
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

    /// Show pipeline stages (plan → search → synthesize → render) as they run.
    #[arg(long, global = true)]
    pub stages: bool,

    /// Show the source list before the report body.
    #[arg(long, global = true)]
    pub show_sources: bool,

    /// Number of corpus chunks to retrieve per sub-question.
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
    /// Run a research query. Alias for the default form: `scaff "..."`.
    Ask {
        /// The research question.
        question: Vec<String>,
    },
    /// Same as `ask`, but explicit.
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
    Show {
        id: String,
    },
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
    Completions {
        shell: String,
    },
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
                // bare `scaff` → start the REPL
                return cmd_chat();
            }
            return cmd_research(&cli, q);
        }
        // bare `scaff` with no args → REPL
        return cmd_chat();
    }

    match cli.command.as_ref().unwrap() {
        Commands::Ask { question } => {
            let q = question.join(" ");
            if q.is_empty() {
                return Err("Usage: scaff ask <question>".to_string());
            }
            cmd_research(&cli, &q)
        }
        Commands::Research { question } => {
            let q = question.join(" ");
            if q.is_empty() {
                return Err("Usage: scaff research <question>".to_string());
            }
            cmd_research(&cli, &q)
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
        println!("\n  {} no executions yet — try: scaff \"what is Harness CD?\"", style("!").yellow());
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
        let model = if e.model.is_empty() { "?".to_string() } else { e.model.clone() };
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
    display::kv("next", "edit .scaff.yaml, then run `scaff connector add-dir ./docs` to index your local docs");
    Ok(())
}

fn cmd_research(cli: &Cli, question: &str) -> Result<(), String> {
    let question = question.trim();
    if question.is_empty() || question.len() < 3 {
        return Err("Question must be at least 3 characters. Try: scaff \"what is Harness CD?\"".to_string());
    }
    if question.len() > 5000 {
        return Err(format!("Question is too long ({} chars, max 5000)", question.len()));
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

    if chunks.is_empty() && cli.verbose {
        eprintln!(
            "  {} corpus is empty — answers will rely on LLM knowledge",
            style("warn").yellow()
        );
    }

    let retrieval_k = cli.retrieval_k.unwrap_or(cfg.retrieval_k).max(1);
    let mut options = ResearchOptions {
        retrieval_k,
        web_enabled: !cli.no_web && cfg.web_enabled,
        max_sub_questions: 4,
        max_search_queries: 6,
        model: model.clone(),
        temperature: 0.3,
        max_output_tokens: 2048,
        show_cost: cli.show_cost || cfg.show_cost,
        on_finding: None,
    };

    if cli.stages {
        eprintln!();
        display::show_stage("plan", "decomposing question into sub-questions");
    }

    // Hook up progressive finding display.
    if cli.stages || cli.verbose {
        options.on_finding = Some(Box::new(move |f: &pipeline::Finding| {
            eprintln!();
            display::finding(f);
        }));
    }

    let spinner_msg = if cli.stages {
        "plan → broad_search → verify → synthesize → render".to_string()
    } else {
        format!("Researching \"{question}\"...")
    };
    let spinner = display::create_spinner(&spinner_msg);
    let result = research::run_research(question, &cfg, provider, Some(&api_key), &index, &mut options)
        .map_err(|e| format!("research failed: {e}"))?;
    spinner.finish_and_clear();

    // Build execution record (auto-save to ~/.scaff/reports and executions).
    let mut exec = Execution::new("research", question);
    exec.model = model.clone();
    exec.provider = provider.name.to_string();
    exec.input_tokens = result.stats.planner_input_tokens + result.stats.synth_input_tokens;
    exec.output_tokens = result.stats.planner_output_tokens + result.stats.synth_output_tokens;
    exec.estimated_cost_usd = result.stats.estimated_cost_usd;
    exec.finished_at = Some(chrono::Utc::now());
    exec.status = pipeline::ExecutionStatus::Succeeded;
    exec.tldr = Some(result.tldr.clone());

    let mut plan_stage = pipeline::Stage::new("plan");
    plan_stage.status = pipeline::StageStatus::Ok;
    plan_stage.finished_at = Some(chrono::Utc::now());
    plan_stage = plan_stage.detail("sub_questions", result.stats.sub_questions as u64);
    exec.stages.push(plan_stage);

    let mut search_stage = pipeline::Stage::new("broad_search");
    search_stage.status = pipeline::StageStatus::Ok;
    search_stage.finished_at = Some(chrono::Utc::now());
    search_stage = search_stage.detail("corpus_hits", result.stats.corpus_chunks_retrieved as u64);
    search_stage = search_stage.detail("web_results", result.stats.web_results_retrieved as u64);
    search_stage = search_stage.detail("search_queries", result.stats.search_queries as u64);
    exec.stages.push(search_stage);

    let mut verify_stage = pipeline::Stage::new("verify");
    verify_stage.status = pipeline::StageStatus::Ok;
    verify_stage.finished_at = Some(chrono::Utc::now());
    verify_stage = verify_stage.detail("claims", result.stats.claims_extracted as u64);
    verify_stage = verify_stage.detail("verified", result.stats.claims_verified as u64);
    exec.stages.push(verify_stage);

    let mut synth_stage = pipeline::Stage::new("synthesize");
    synth_stage.status = pipeline::StageStatus::Ok;
    synth_stage.finished_at = Some(chrono::Utc::now());
    synth_stage = synth_stage.detail("input_tokens", exec.input_tokens as u64);
    synth_stage = synth_stage.detail("output_tokens", exec.output_tokens as u64);
    exec.stages.push(synth_stage);

    let mut render_stage = pipeline::Stage::new("render");
    render_stage.status = pipeline::StageStatus::Ok;
    render_stage.duration_ms = result.stats.elapsed_ms;
    render_stage.finished_at = Some(chrono::Utc::now());
    exec.stages.push(render_stage);

    let mut sources: Vec<Source> = Vec::new();
    for (i, h) in result.corpus_hits.iter().enumerate() {
        sources.push(Source {
            id: i + 1,
            title: h.chunk.title.clone(),
            url: h.chunk.url.clone(),
            kind: SourceKind::Corpus,
            score: h.score,
        });
    }
    let offset = result.corpus_hits.len();
    for (i, w) in result.web_results.iter().enumerate() {
        sources.push(Source {
            id: offset + i + 1,
            title: w.title.clone(),
            url: w.url.clone(),
            kind: SourceKind::Web,
            score: 0.0,
        });
    }
    exec.sources = sources;
    exec.claims = result.claims.clone();
    exec.verifications = result.verifications.clone();

    let saved_report = history::save_report(&exec, &result.report).ok();
    if let Some(p) = &saved_report {
        exec.report_path = Some(p.display().to_string());
    }
    history::save_execution(&exec).ok();

    // Output: stage progress
    if cli.stages {
        eprintln!();
        eprintln!("  {}", style("Pipeline Summary").cyan().bold());
        for s in &exec.stages {
            let detail = s.details.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            if detail.is_empty() {
                display::stage_ok(&s.name, &format!("{} ms", s.duration_ms));
            } else {
                display::stage_ok(&s.name, &format!("{} ms — {}", s.duration_ms, detail));
            }
        }
        eprintln!("  {}  {} — {} sources, {} claims",
            style("✓").green(),
            style(&exec.id).dim(),
            exec.source_count(),
            exec.claims.len()
        );
        eprintln!();
    }

    if cli.show_sources {
        print_sources(&result.corpus_hits, &result.web_results);
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
        display::kv("model", &options.model);
        display::kv("provider", provider.name);
        display::kv("sub-questions", &result.stats.sub_questions.to_string());
        display::kv("search queries", &result.stats.search_queries.to_string());
        display::kv("corpus chunks", &result.stats.corpus_chunks_retrieved.to_string());
        display::kv("web results", &result.stats.web_results_retrieved.to_string());
        display::kv("claims extracted", &result.stats.claims_extracted.to_string());
        display::kv("claims verified", &result.stats.claims_verified.to_string());
        let conf = exec.overall_confidence();
        display::kv("overall confidence", &format!("{:.0}%", conf * 100.0));
        display::kv("elapsed", &format!("{} ms", result.stats.elapsed_ms));
        display::kv("tokens (in/out)", &format!("{} / {}", exec.input_tokens, exec.output_tokens));
        display::kv("est. cost", &format!("${:.6}", result.stats.estimated_cost_usd));
    }
    Ok(())
}

fn print_sources(corpus: &[crate::search::SearchHit], web: &[crate::web::WebResult]) {
    if corpus.is_empty() && web.is_empty() {
        return;
    }
    eprintln!();
    eprintln!("  {}", style("Sources").cyan().bold());
    let mut n = 0;
    for h in corpus {
        n += 1;
        eprintln!(
            "    [{:>2}] score={:>5.2}  {}  {}",
            n,
            h.score,
            style(&h.chunk.title).bold(),
            style(&h.chunk.url).dim()
        );
    }
    for w in web {
        n += 1;
        eprintln!(
            "    [{:>2}] web  {}  {}",
            n,
            style(&w.title).bold(),
            style(&w.url).dim()
        );
    }
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
            let n = corpus::ingest_seed_path(&conn, &path)
                .map_err(|e| format!("ingest: {e}"))?;
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
            let chunks = corpus::crawl_url(&url, &source)
                .map_err(|e| format!("crawl: {e}"))?;
            spinner.finish_and_clear();
            let conn = corpus::open_db().map_err(|e| format!("open corpus: {e}"))?;
            let n = corpus::ingest_raw(&conn, &chunks)
                .map_err(|e| format!("ingest: {e}"))?;
            display::ok(&format!("Added {n} chunks from {url}"));
            Ok(())
        }
        CorpusAction::Update { url } => {
            let url = url.unwrap_or_else(|| corpus::DEFAULT_SEED_URL.to_string());
            let spinner = display::create_spinner(&format!("Downloading seed from {url}..."));
            let dest = corpus::seed_path();
            let n = corpus::download_seed(&url, &dest)
                .map_err(|e| format!("download: {e}"))?;
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
            display::kv("stored API keys", &format!("{} provider(s)", cfg.api_keys.len()));
            display::kv("config file", &ScaffConfig::config_path().display().to_string());
            display::kv("corpus DB", &ScaffConfig::corpus_db_path().display().to_string());
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
    display::kv("config file", &ScaffConfig::config_path().display().to_string());

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
        println!("  connector [{}] {} — {} {}", kind, c.name, badge, c.details);
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
        _ => return Err(format!("Unknown shell: {shell}. Valid: bash, zsh, fish, powershell, elvish")),
    };
    let mut buf: Vec<u8> = Vec::new();
    clap_complete::generate(shell_type, &mut app, "scaff", &mut buf);
    std::io::stdout().write_all(&buf).map_err(|e| format!("{e}"))?;
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

// Re-export for the report renderer so the version constant resolves.
#[allow(dead_code)]
fn _render_unused() {
    let _ = render::render_report;
}
