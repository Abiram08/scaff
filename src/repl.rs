use std::io::{self, BufRead, Write};

use anyhow::Result;
use console::style;

use crate::agent::{self, AgentOptions, UseCase};
use crate::config::{self, ScaffConfig};
use crate::display;
use crate::history;
use crate::pipeline::{self, Execution, Source, SourceKind};

pub struct ReplState {
    pub cfg: ScaffConfig,
    pub provider: &'static config::ProviderSpec,
    pub api_key: String,
    pub model: String,
    pub history: Vec<Execution>,
    pub show_stages: bool,
    pub show_sources: bool,
    pub web_enabled: bool,
}

pub fn run() -> Result<()> {
    let mut state = setup()?;

    println!(
        "\n  {} {}\n",
        style("scaff chat").cyan().bold(),
        style("— type a question, /help for commands, /quit to exit").dim()
    );
    if state.history.is_empty() {
        println!(
            "  {} try: {}",
            style("tip").yellow(),
            style("what is Harness CD canary?").italic()
        );
    } else {
        println!(
            "  {} {} previous execution(s) in this session",
            style("memory").yellow(),
            state.history.len()
        );
    }
    println!();

    let stdin = io::stdin();
    let mut input = String::new();

    loop {
        print_prompt();
        io::stdout().flush().ok();
        input.clear();
        let n = stdin.lock().read_line(&mut input)?;
        if n == 0 {
            println!();
            break;
        }
        let line = input.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('/') {
            match handle_slash(line, &mut state) {
                Slash::Continue => continue,
                Slash::Exit => break,
            }
        }

        if let Err(e) = ask(&mut state, line) {
            display::err(&format!("{e}"));
        }
    }
    Ok(())
}

fn print_prompt() {
    print!("{} ", style("scaff>").green().bold());
}

fn setup() -> Result<ReplState> {
    crate::init::ensure_global_dirs()?;
    let cfg = ScaffConfig::load();
    let provider =
        config::resolve_provider(Some(&cfg.default_provider)).map_err(anyhow::Error::msg)?;
    let api_key = config::require_api_key(provider, &cfg).map_err(anyhow::Error::msg)?;
    let model = config::resolve_model(provider, Some(&cfg.model), cfg.cheap);
    let web_enabled = cfg.web_enabled;
    Ok(ReplState {
        cfg,
        provider,
        api_key,
        model,
        history: Vec::new(),
        show_stages: true,
        show_sources: true,
        web_enabled,
    })
}

enum Slash {
    Continue,
    Exit,
}

fn handle_slash(line: &str, state: &mut ReplState) -> Slash {
    let mut parts = line.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next().unwrap_or("").trim();

    match cmd {
        "/help" | "/?" => {
            print_help();
        }
        "/quit" | "/exit" | "/q" => return Slash::Exit,
        "/stages" => {
            state.show_stages = !state.show_stages;
            println!(
                "  {} stage visibility: {}",
                style("ok").green(),
                if state.show_stages { "on" } else { "off" }
            );
        }
        "/sources-toggle" => {
            state.show_sources = !state.show_sources;
            println!(
                "  {} show sources: {}",
                style("ok").green(),
                if state.show_sources { "on" } else { "off" }
            );
        }
        "/sources" => {
            if let Some(exec) = state.history.last() {
                print_sources_from_exec(exec);
            } else {
                display::err("no previous execution");
            }
        }
        "/web" => {
            state.web_enabled = !state.web_enabled;
            println!(
                "  {} web fallback: {}",
                style("ok").green(),
                if state.web_enabled { "on" } else { "off" }
            );
        }
        "/model" => {
            if arg.is_empty() {
                println!("  current model: {}", state.model);
            } else {
                state.model = arg.to_string();
                println!("  {} model = {}", style("ok").green(), state.model);
            }
        }
        "/provider" => {
            if arg.is_empty() {
                println!("  current provider: {}", state.provider.name);
            } else {
                match config::resolve_provider(Some(arg)) {
                    Ok(p) => match config::require_api_key(p, &state.cfg) {
                        Ok(k) => {
                            state.provider = p;
                            state.api_key = k;
                            state.model = config::resolve_model(p, None, state.cfg.cheap);
                            println!(
                                "  {} provider = {}, model = {}",
                                style("ok").green(),
                                p.name,
                                state.model
                            );
                        }
                        Err(e) => display::err(&e),
                    },
                    Err(e) => display::err(&e),
                }
            }
        }
        "/history" | "/h" => {
            for (i, e) in state.history.iter().enumerate() {
                let status = match e.status {
                    pipeline::ExecutionStatus::Succeeded => style("ok").green(),
                    pipeline::ExecutionStatus::Failed => style("fail").red(),
                    pipeline::ExecutionStatus::Running => style("...").yellow(),
                };
                println!(
                    "  {}. [{}] {} — {}",
                    i + 1,
                    status,
                    e.id,
                    truncate(&e.question, 60)
                );
            }
        }
        "/show" => {
            let id = arg;
            if id.is_empty() {
                display::err("usage: /show <id or index>");
            } else {
                show_execution(state, id);
            }
        }
        "/expand" | "/e" => {
            let n: usize = match arg.parse() {
                Ok(n) => n,
                Err(_) => {
                    display::err("usage: /expand <source-number>");
                    return Slash::Continue;
                }
            };
            expand_source(state, n);
        }
        "/save" => {
            save_current_pipeline(state, arg);
        }
        "/last" => {
            if let Some(e) = state.history.last() {
                if let Some(p) = &e.report_path {
                    println!("  artifact: {p}");
                } else {
                    println!("  no artifact path stored for the last execution");
                }
            } else {
                display::err("no previous execution");
            }
        }
        "/clear" => {
            state.history.clear();
            println!("  {} session history cleared", style("ok").green());
        }
        "/deepen" => {
            if arg.is_empty() {
                display::err("usage: /deepen <topic>");
            } else {
                let question = format!("Deep dive: {arg}");
                if let Err(e) = ask(state, &question) {
                    display::err(&format!("{e}"));
                }
            }
        }
        "/redirect" => {
            if arg.is_empty() {
                display::err("usage: /redirect <new angle>");
            } else {
                println!("  {} redirecting with new framing...", style("ok").green());
                if let Err(e) = ask(state, arg) {
                    display::err(&format!("{e}"));
                }
            }
        }
        "/confidence" => {
            if let Some(exec) = state.history.last() {
                print_confidence_breakdown(exec);
            } else {
                display::err("no previous execution");
            }
        }
        "/export" => {
            if let Some(exec) = state.history.last() {
                export_session(exec);
            } else {
                display::err("no previous execution to export");
            }
        }
        _ => {
            display::err(&format!("unknown command: {cmd}. Try /help."));
        }
    }
    Slash::Continue
}

fn print_help() {
    println!();
    println!("  {}", style("Chat commands:").cyan().bold());
    println!("    /help                  show this help");
    println!("    /quit, /exit, /q       leave the REPL");
    println!();
    println!("  {}", style("Toggles:").cyan().bold());
    println!("    /stages                toggle pipeline-stage visibility");
    println!("    /sources-toggle        toggle inline source display");
    println!("    /web                   toggle web search fallback");
    println!();
    println!("  {}", style("Configuration:").cyan().bold());
    println!("    /model [name]          show or set the LLM model");
    println!("    /provider [name]       show or set the provider");
    println!();
    println!("  {}", style("Research:").cyan().bold());
    println!("    /deepen <topic>        spawn a focused mini-pipeline on a subtopic");
    println!("    /redirect <framing>    re-run from Plan with a new angle");
    println!("    /confidence            show full confidence breakdown for all claims");
    println!();
    println!("  {}", style("Memory:").cyan().bold());
    println!("    /history, /h           list executions in this session");
    println!("    /show <id|#>           reprint an execution's report");
    println!("    /expand <N>            show full text of source N from the last report");
    println!("    /clear                 clear session memory");
    println!();
    println!("  {}", style("Artifacts:").cyan().bold());
    println!("    /last                  print the path of the last report");
    println!("    /save [name]           save the last research as a reusable pipeline YAML");
    println!("    /export                export the full session as markdown");
    println!();
    println!("  {}", style("Tip:").yellow());
    println!("    Ask follow-up questions in plain English. The REPL keeps the corpus,");
    println!("    provider, and model across turns. Type your question (no slash) to run.");
    println!();
}

fn ask(state: &mut ReplState, question: &str) -> Result<()> {
    let spinner_msg = if state.show_stages {
        "agent · search_corpus → finish".to_string()
    } else {
        format!("researching: {question}")
    };
    let spinner = display::create_spinner(&spinner_msg);

    let conn = crate::corpus::open_db()?;
    let chunks = crate::corpus::load_all_chunks(&conn)?;
    let index = if chunks.is_empty() {
        eprintln!(
            "  {} corpus is empty — answering with reduced citations",
            style("warn").yellow()
        );
        crate::search::Index::build(vec![])
    } else {
        crate::search::Index::build(chunks)
    };

    let options = AgentOptions {
        use_case: UseCase::Ask,
        retrieval_k: state.cfg.retrieval_k,
        web_enabled: state.web_enabled,
        model: state.model.clone(),
        temperature: 0.2,
        max_output_tokens: 2048,
        max_steps: 8,
        show_stages: state.show_stages,
        show_cost: false,
    };

    let out = agent::run(
        question,
        state.provider,
        Some(&state.api_key),
        &index,
        &options,
    )?;

    spinner.finish_and_clear();

    let mut exec = Execution::new("ask", question);
    exec.model = options.model.clone();
    exec.provider = state.provider.name.to_string();
    exec.input_tokens = out.stats.input_tokens;
    exec.output_tokens = out.stats.output_tokens;
    exec.estimated_cost_usd = out.stats.estimated_cost_usd;
    exec.finished_at = Some(chrono::Utc::now());
    exec.status = pipeline::ExecutionStatus::Succeeded;
    exec.tldr = Some(out.payload.tldr.clone());

    for line in &out.steps_log {
        let mut stage = pipeline::Stage::new("tool");
        stage.status = pipeline::StageStatus::Ok;
        stage.finished_at = Some(chrono::Utc::now());
        stage = stage.detail("step", line.clone());
        exec.stages.push(stage);
    }

    let mut sources: Vec<Source> = Vec::new();
    for (i, s) in out.payload.sources.iter().enumerate() {
        sources.push(Source {
            id: i + 1,
            title: s.title.clone(),
            url: s.url.clone(),
            kind: if s.kind == "web" {
                SourceKind::Web
            } else {
                SourceKind::Corpus
            },
            score: 0.0,
        });
    }
    exec.sources = sources;
    exec.claims = out
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

    let report_path = history::save_report(&exec, &out.report).ok();
    if let Some(p) = &report_path {
        exec.report_path = Some(p.display().to_string());
    }
    history::save_execution(&exec).ok();

    if state.show_stages {
        eprintln!();
        eprintln!("  {}", style("Agent steps").cyan().bold());
        for line in &out.steps_log {
            eprintln!("  {} {}", style("→").dim(), line);
        }
        eprintln!();
    }

    println!("{}", out.report);

    if state.show_sources && !out.payload.sources.is_empty() {
        eprintln!();
        eprintln!("  {}", style("Sources").cyan().bold());
        for s in &out.payload.sources {
            eprintln!(
                "    {}  {}  {}",
                style(&s.id).dim(),
                style(&s.title).bold(),
                style(&s.url).dim()
            );
        }
    }

    state.history.push(exec);
    Ok(())
}

fn expand_source(state: &ReplState, n: usize) {
    let exec = match state.history.last() {
        Some(e) => e,
        None => {
            display::err("no previous execution");
            return;
        }
    };
    if n == 0 || n > exec.sources.len() {
        display::err(&format!(
            "source {} not in last report (have {})",
            n,
            exec.sources.len()
        ));
        return;
    }
    let src = &exec.sources[n - 1];
    if matches!(src.kind, pipeline::SourceKind::Web) {
        eprintln!(
            "  {} (web source — fetch on demand with `scaff corpus crawl {}`)",
            style(&src.url).dim(),
            src.url
        );
        return;
    }
    let conn = match crate::corpus::open_db() {
        Ok(c) => c,
        Err(e) => {
            display::err(&e.to_string());
            return;
        }
    };
    let chunks = match crate::corpus::load_all_chunks(&conn) {
        Ok(c) => c,
        Err(e) => {
            display::err(&e.to_string());
            return;
        }
    };
    let idx = crate::search::Index::build(chunks);
    let hits = idx.search_in(&src.title, 1, Some(&src.url));
    if let Some(h) = hits.first() {
        println!(
            "\n  {} — {}\n",
            style(&h.chunk.title).bold(),
            style(&h.chunk.url).dim()
        );
        println!("{}\n", h.chunk.content);
    } else {
        display::err("source not found in current corpus");
    }
}

fn show_execution(state: &ReplState, id_or_index: &str) {
    let exec: Option<&Execution> = if let Ok(idx) = id_or_index.parse::<usize>() {
        state.history.get(idx.saturating_sub(1))
    } else {
        state.history.iter().find(|e| e.id == id_or_index)
    };
    match exec {
        Some(e) => {
            if let Some(p) = &e.report_path {
                if let Ok(body) = std::fs::read_to_string(p) {
                    println!("{body}");
                    return;
                }
            }
            display::err("report artifact not on disk");
        }
        None => display::err("execution not found"),
    }
}

fn save_current_pipeline(state: &mut ReplState, name: &str) {
    let exec = match state.history.last() {
        Some(e) => e.clone(),
        None => {
            display::err("no execution to save");
            return;
        }
    };
    let default_name = format!("pipeline-{}", &exec.id);
    let name = if name.is_empty() { &default_name } else { name };
    match history::save_pipeline_yaml(name, &exec.question, &exec.model, &exec.provider) {
        Ok(path) => println!(
            "  {} saved pipeline: {}",
            style("ok").green(),
            path.display()
        ),
        Err(e) => display::err(&e.to_string()),
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

fn print_sources_from_exec(exec: &Execution) {
    if exec.sources.is_empty() {
        println!("  no sources in last execution");
        return;
    }
    eprintln!();
    eprintln!("  {}", style("Sources").cyan().bold());
    for src in &exec.sources {
        let kind = match src.kind {
            pipeline::SourceKind::Corpus => "corpus",
            pipeline::SourceKind::Web => "web",
            pipeline::SourceKind::LocalFile => "local",
        };
        eprintln!(
            "    [{:>2}] {:>5.2}  {}  {}  {}",
            src.id,
            src.score,
            style(&src.title).bold(),
            style(&src.url).dim(),
            style(kind).dim()
        );
    }
    eprintln!();
}

fn print_confidence_breakdown(exec: &Execution) {
    if exec.claims.is_empty() {
        println!("  no claims in last execution");
        return;
    }
    eprintln!();
    eprintln!("  {}", style("Confidence Breakdown").cyan().bold());
    eprintln!("  {}", exec.confidence_summary());
    eprintln!();
    for claim in &exec.claims {
        let emoji = claim.confidence.emoji();
        let note = claim.conflict_note.as_deref().unwrap_or("");
        eprintln!(
            "  {} [{}] {} {}",
            emoji,
            claim.confidence,
            claim.text,
            if note.is_empty() {
                String::new()
            } else {
                format!("— {}", style(note).dim())
            }
        );
    }
    eprintln!();
}

fn export_session(exec: &Execution) {
    let mut md = String::new();
    md.push_str(&format!("# Research Session: {}\n\n", exec.question));
    md.push_str(&format!("**ID:** `{}`\n", exec.id));
    md.push_str(&format!("**Pipeline:** {}\n", exec.pipeline));
    md.push_str(&format!("**Model:** {} ({})\n", exec.model, exec.provider));
    md.push_str(&format!("**Status:** {:?}\n", exec.status));
    md.push_str(&format!("**Duration:** {} ms\n", exec.total_duration_ms()));
    md.push_str(&format!("**Sources:** {}\n", exec.source_count()));
    md.push_str(&format!(
        "**Claims:** {} ({})\n",
        exec.claims.len(),
        exec.confidence_summary()
    ));
    md.push_str("\n---\n\n");

    if let Some(tldr) = &exec.tldr {
        md.push_str(&format!("## TL;DR\n\n{}\n\n", tldr));
    }

    md.push_str("## Pipeline Stages\n\n");
    for stage in &exec.stages {
        md.push_str(&format!(
            "- **{}**: {:?} ({} ms)\n",
            stage.name, stage.status, stage.duration_ms
        ));
    }
    md.push_str("\n");

    if !exec.claims.is_empty() {
        md.push_str("## Claims & Confidence\n\n");
        for claim in &exec.claims {
            let emoji = claim.confidence.emoji();
            md.push_str(&format!(
                "- {} [{}] {}\n",
                emoji, claim.confidence, claim.text
            ));
            if let Some(note) = &claim.conflict_note {
                md.push_str(&format!("  - Conflict: {}\n", note));
            }
        }
        md.push_str("\n");
    }

    if !exec.sources.is_empty() {
        md.push_str("## Sources\n\n");
        for src in &exec.sources {
            md.push_str(&format!("- [{}] {} — {}\n", src.id, src.title, src.url));
        }
        md.push_str("\n");
    }

    md.push_str(&format!(
        "---\n*Exported by scaff on {}*\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
    ));

    let export_path = std::env::temp_dir().join(format!("scaff-export-{}.md", exec.id));
    match std::fs::write(&export_path, &md) {
        Ok(_) => println!(
            "  {} exported to: {}",
            style("ok").green(),
            export_path.display()
        ),
        Err(e) => display::err(&format!("export failed: {e}")),
    }
}
