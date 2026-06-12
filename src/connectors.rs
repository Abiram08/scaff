use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::corpus;
use crate::local_files;
use crate::search::Index;
use crate::web;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectorKind {
    Corpus,
    Web,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connector {
    pub name: String,
    pub kind: ConnectorKind,
    pub description: String,
    pub status: ConnectorStatus,
    pub last_tested: Option<chrono::DateTime<chrono::Utc>>,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectorStatus {
    Unknown,
    Healthy,
    Degraded,
    Failed,
}

impl ConnectorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnectorStatus::Unknown => "unknown",
            ConnectorStatus::Healthy => "healthy",
            ConnectorStatus::Degraded => "degraded",
            ConnectorStatus::Failed => "failed",
        }
    }
}

pub fn list_default_connectors() -> Vec<Connector> {
    vec![
        Connector {
            name: "corpus".to_string(),
            kind: ConnectorKind::Corpus,
            description: "Curated Harness knowledge base (SQLite + JSONL seed)".to_string(),
            status: ConnectorStatus::Unknown,
            last_tested: None,
            details: serde_json::json!({}),
        },
        Connector {
            name: "web".to_string(),
            kind: ConnectorKind::Web,
            description: "DuckDuckGo HTML web search fallback".to_string(),
            status: ConnectorStatus::Unknown,
            last_tested: None,
            details: serde_json::json!({}),
        },
    ]
}

pub fn test_corpus() -> (ConnectorStatus, serde_json::Value) {
    match corpus::open_db() {
        Ok(conn) => match corpus::stats(&conn) {
            Ok(s) if s.total > 0 => (
                ConnectorStatus::Healthy,
                serde_json::json!({
                    "chunks": s.total,
                    "urls": s.unique_urls,
                    "bytes": s.total_bytes,
                }),
            ),
            Ok(_) => (ConnectorStatus::Degraded, serde_json::json!({"chunks": 0})),
            Err(e) => (
                ConnectorStatus::Failed,
                serde_json::json!({"error": e.to_string()}),
            ),
        },
        Err(e) => (
            ConnectorStatus::Failed,
            serde_json::json!({"error": e.to_string()}),
        ),
    }
}

pub fn test_web() -> (ConnectorStatus, serde_json::Value) {
    match web::search("harness", 1) {
        Ok(rs) => (
            ConnectorStatus::Healthy,
            serde_json::json!({"test_query": "harness", "results": rs.len()}),
        ),
        Err(e) => (
            ConnectorStatus::Degraded,
            serde_json::json!({"error": e.to_string(), "note": "non-fatal; corpus still works"}),
        ),
    }
}

pub fn test_connector(name: &str) -> Option<(ConnectorStatus, serde_json::Value)> {
    match name {
        "corpus" => Some(test_corpus()),
        "web" => Some(test_web()),
        _ => None,
    }
}

pub fn build_index() -> anyhow::Result<Index> {
    let conn = corpus::open_db()?;
    let chunks = corpus::load_all_chunks(&conn)?;
    Ok(Index::build(chunks))
}

pub fn build_index_with_local(extra_paths: &[PathBuf]) -> anyhow::Result<Index> {
    let mut chunks = corpus::load_all_chunks(&corpus::open_db()?)?;
    for p in extra_paths {
        if let Ok(mut local) = local_files::ingest_directory(p) {
            chunks.append(&mut local);
        }
    }
    Ok(Index::build(chunks))
}

pub fn local_dir_status(path: &Path) -> serde_json::Value {
    serde_json::json!({
        "path": path.display().to_string(),
        "exists": path.exists(),
        "is_dir": path.is_dir(),
    })
}
