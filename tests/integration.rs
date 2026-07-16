use std::path::PathBuf;

use scaff::{config::ScaffConfig, corpus, search::Index};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("corpus")
        .join("seed.jsonl")
}

#[test]
fn load_real_seed_parses() {
    let path = fixtures_dir();
    assert!(path.exists(), "seed.jsonl missing at {path:?}");
    let raw = corpus::load_seed_file(&path).expect("seed loads");
    assert!(raw.len() >= 25, "expected at least 25 chunks, got {}", raw.len());
    for r in &raw {
        assert!(!r.id.is_empty());
        assert!(!r.url.is_empty());
        assert!(!r.title.is_empty());
        assert!(!r.content.is_empty());
    }
}

#[test]
fn build_index_from_seed() {
    let path = fixtures_dir();
    let raw = corpus::load_seed_file(&path).expect("seed loads");
    let chunks: Vec<scaff::search::Chunk> = raw
        .into_iter()
        .map(|r| scaff::search::Chunk {
            id: r.id,
            url: r.url,
            title: r.title,
            section: r.section,
            content: r.content,
        })
        .collect();
    let index = Index::build(chunks);
    assert!(!index.is_empty());

    let hits = index.search("canary deployment", 5);
    assert!(!hits.is_empty(), "expected hits for canary");
    assert!(hits[0].chunk.content.to_lowercase().contains("canary"));

    let hits = index.search("OIDC SAML SSO", 3);
    // May or may not have direct hits — just ensure the call returns and ranks sensibly.
    let _ = hits;
}

#[test]
fn search_returns_unique_chunks() {
    let path = fixtures_dir();
    let raw = corpus::load_seed_file(&path).expect("seed loads");
    let chunks: Vec<scaff::search::Chunk> = raw
        .into_iter()
        .map(|r| scaff::search::Chunk {
            id: r.id,
            url: r.url,
            title: r.title,
            section: r.section,
            content: r.content,
        })
        .collect();
    let index = Index::build(chunks);
    let hits = index.search("harness", 10);
    let mut ids: Vec<_> = hits.iter().map(|h| h.chunk.id.clone()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), hits.len(), "duplicate hits returned");
}

#[test]
fn config_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.json");
    let mut cfg = ScaffConfig::default();
    cfg.model = "claude-3-5-sonnet-latest".into();
    cfg.default_provider = "anthropic".into();
    cfg.retrieval_k = 12;
    cfg.web_enabled = false;
    cfg.set_api_key("anthropic", "test-key".to_string());
    let body = serde_json::to_string_pretty(&cfg).unwrap();
    std::fs::write(&path, body).unwrap();

    let raw = std::fs::read_to_string(&path).unwrap();
    let parsed: ScaffConfig = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed.model, "claude-3-5-sonnet-latest");
    assert_eq!(parsed.default_provider, "anthropic");
    assert_eq!(parsed.retrieval_k, 12);
    assert!(!parsed.web_enabled);
    assert_eq!(parsed.api_keys.get("anthropic").unwrap(), "test-key");
}

#[test]
fn render_with_real_hits() {
    let path = fixtures_dir();
    let raw = corpus::load_seed_file(&path).expect("seed loads");
    let chunks: Vec<scaff::search::Chunk> = raw
        .into_iter()
        .map(|r| scaff::search::Chunk {
            id: r.id,
            url: r.url,
            title: r.title,
            section: r.section,
            content: r.content,
        })
        .collect();
    let index = Index::build(chunks);
    let hits = index.search("canary percentage rollout", 3);
    assert!(!hits.is_empty());
    let payload = scaff::agent::FinishPayload {
        tldr: "Start at 10% and advance through 25%, 50%, then 100% with verification gates."
            .into(),
        findings: vec![scaff::agent::Finding {
            text: format!(
                "Canary splits traffic by percentage; seed hit: {}",
                hits[0].chunk.title
            ),
            confidence: scaff::agent::Confidence::High,
            source_ids: vec![format!("corpus:{}", hits[0].chunk.id)],
            conflict_note: None,
        }],
        known_unknowns: vec![],
        sources: vec![scaff::agent::SourceRef {
            id: format!("corpus:{}", hits[0].chunk.id),
            title: hits[0].chunk.title.clone(),
            url: hits[0].chunk.url.clone(),
            kind: "corpus".into(),
        }],
    };
    let report = scaff::report::render(&payload);
    assert!(report.contains("## TL;DR"));
    assert!(report.contains("## Findings"));
    assert!(report.contains("## Sources"));
    assert!(report.contains("canary") || report.to_lowercase().contains("canary"));
}

#[test]
fn pipeline_execution_round_trip() {
    use scaff::pipeline::{Execution, ExecutionStatus, Source, SourceKind, Stage, StageStatus};

    let mut exec = Execution::new("research.v2", "what is harness?");
    exec.stages.push(Stage::new("plan"));
    exec.stages.push(Stage::new("search"));
    exec.stages[1].status = StageStatus::Ok;
    exec.sources.push(Source {
        id: 0,
        kind: SourceKind::Corpus,
        title: "Harness overview".to_string(),
        url: "harness://docs/intro".to_string(),
        score: 0.92,
    });
    exec.status = ExecutionStatus::Succeeded;
    exec.finished_at = Some(chrono::Utc::now());
    assert_eq!(exec.stages.len(), 2);
    assert_eq!(exec.sources.len(), 1);

    let json = serde_json::to_string(&exec).expect("serialize");
    let back: Execution = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.id, exec.id);
    assert_eq!(back.question, "what is harness?");
    assert_eq!(back.sources.len(), 1);
    assert!(matches!(back.status, ExecutionStatus::Succeeded));
}

#[test]
fn history_round_trip_uses_temp_home() {
    use scaff::history;
    use scaff::pipeline::{Execution, ExecutionStatus, Source, SourceKind};

    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("USERPROFILE", tmp.path());
    std::env::set_var("XDG_DATA_HOME", tmp.path().join("data"));
    std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("config"));
    std::env::set_var("APPDATA", tmp.path().join("appdata"));

    let mut exec = Execution::new("research.v2", "explain canary deploys");
    exec.sources.push(Source {
        id: 0,
        kind: SourceKind::Corpus,
        title: "Canary basics".to_string(),
        url: "harness://cd/canary".to_string(),
        score: 0.88,
    });
    exec.status = ExecutionStatus::Succeeded;
    exec.finished_at = Some(chrono::Utc::now());
    exec.tldr = Some("Canary is a deployment strategy.".to_string());

    let path = history::save_execution(&exec).expect("save");
    assert!(path.exists(), "execution file should exist at {path:?}");

    let md_path = history::save_report(
        &exec,
        "## TL;DR\n\nCanary is a deployment strategy.\n\n## Findings\n\nCanary splits traffic [1].\n",
    )
    .expect("save report");
    let md = std::fs::read_to_string(&md_path).expect("read report");
    assert!(md.contains("---"));
    assert!(md.contains("id: "));
    assert!(md.contains("pipeline: research.v2"));
    assert!(md.contains("## TL;DR"));
    assert!(md.contains("[1]"));

    let listed = history::list_executions(50).expect("list");
    assert!(listed.iter().any(|e| e.id == exec.id));
}

#[test]
fn local_files_chunk_markdown_splits_on_heading() {
    use scaff::local_files;

    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("doc.md");
    std::fs::write(
        &md,
        "# Title\n\nintro text\n\n## Sub\n\nbody text under sub.\n",
    )
    .unwrap();

    let raw = local_files::ingest_file(&md).expect("ingest");
    assert!(!raw.is_empty(), "expected at least one chunk");
    let joined: String = raw.iter().map(|r| r.content.clone()).collect::<Vec<_>>().join("\n");
    assert!(joined.contains("Title"));
    assert!(joined.contains("Sub"));
    assert!(raw.iter().all(|r| r.url.starts_with("local://")));
}

#[test]
fn init_writes_project_file() {
    use scaff::init;

    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("USERPROFILE", tmp.path());
    std::env::set_var("XDG_DATA_HOME", tmp.path().join("data"));
    std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("config"));
    std::env::set_var("APPDATA", tmp.path().join("appdata"));

    init::init_project(tmp.path()).expect("init project");
    let cfg_path = tmp.path().join(".scaff.yaml");
    assert!(cfg_path.exists(), ".scaff.yaml should exist at {cfg_path:?}");
    let body = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(body.contains("model:"));
    assert!(body.contains("default_provider:"));
}

#[test]
fn connectors_list_default_includes_corpus_and_web() {
    use scaff::connectors;

    let list = connectors::list_default_connectors();
    assert!(list.iter().any(|c| matches!(c.kind, scaff::connectors::ConnectorKind::Corpus)));
    assert!(list.iter().any(|c| matches!(c.kind, scaff::connectors::ConnectorKind::Web)));
}
