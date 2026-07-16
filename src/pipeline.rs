//! Execution history types (persisted runs, not agent orchestration).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StageStatus {
    Pending,
    Running,
    Ok,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
    Contested,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Confidence::High => write!(f, "high"),
            Confidence::Medium => write!(f, "medium"),
            Confidence::Low => write!(f, "low"),
            Confidence::Contested => write!(f, "contested"),
        }
    }
}

impl Confidence {
    pub fn emoji(self) -> &'static str {
        match self {
            Confidence::High => "✅",
            Confidence::Medium => "⚠️",
            Confidence::Low => "🔴",
            Confidence::Contested => "⚡",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    pub name: String,
    pub status: StageStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: u128,
    pub details: BTreeMap<String, serde_json::Value>,
}

impl Stage {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: StageStatus::Pending,
            started_at: Utc::now(),
            finished_at: None,
            duration_ms: 0,
            details: BTreeMap::new(),
        }
    }

    pub fn detail(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: usize,
    pub title: String,
    pub url: String,
    pub kind: SourceKind,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Corpus,
    Web,
    LocalFile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub id: usize,
    pub text: String,
    #[serde(default)]
    pub source_ids: Vec<usize>,
    pub confidence: Confidence,
    #[serde(default)]
    pub conflict_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    pub id: String,
    pub pipeline: String,
    pub question: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: ExecutionStatus,
    pub stages: Vec<Stage>,
    pub sources: Vec<Source>,
    pub claims: Vec<Claim>,
    pub model: String,
    pub provider: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub estimated_cost_usd: f64,
    pub report_path: Option<String>,
    pub tldr: Option<String>,
    pub error: Option<String>,
}

impl Execution {
    pub fn new(pipeline: impl Into<String>, question: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            pipeline: pipeline.into(),
            question: question.into(),
            started_at: Utc::now(),
            finished_at: None,
            status: ExecutionStatus::Running,
            stages: Vec::new(),
            sources: Vec::new(),
            claims: Vec::new(),
            model: String::new(),
            provider: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            estimated_cost_usd: 0.0,
            report_path: None,
            tldr: None,
            error: None,
        }
    }

    pub fn total_duration_ms(&self) -> u128 {
        if let Some(f) = self.finished_at {
            (f - self.started_at).num_milliseconds().max(0) as u128
        } else {
            (Utc::now() - self.started_at).num_milliseconds().max(0) as u128
        }
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub fn high_confidence_count(&self) -> usize {
        self.claims
            .iter()
            .filter(|c| c.confidence == Confidence::High)
            .count()
    }

    pub fn contested_count(&self) -> usize {
        self.claims
            .iter()
            .filter(|c| c.confidence == Confidence::Contested)
            .count()
    }

    pub fn confidence_summary(&self) -> String {
        let high = self.high_confidence_count();
        let medium = self
            .claims
            .iter()
            .filter(|c| c.confidence == Confidence::Medium)
            .count();
        let low = self
            .claims
            .iter()
            .filter(|c| c.confidence == Confidence::Low)
            .count();
        let contested = self.contested_count();
        format!("✅ {high} high | ⚠️ {medium} medium | 🔴 {low} low | ⚡ {contested} contested")
    }
}

pub fn new_id() -> String {
    let now = Utc::now();
    let stamp = now.format("%Y%m%d-%H%M%S").to_string();
    let suffix: u32 = (now.timestamp_subsec_nanos() % 9999) + 1;
    format!("exec-{stamp}-{suffix:04}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_id_is_unique() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
    }

    #[test]
    fn execution_serializes() {
        let mut e = Execution::new("ask", "What is Harness CD?");
        e.stages.push(Stage::new("finish").detail("findings", 2u64));
        e.claims.push(Claim {
            id: 0,
            text: "Harness uses Argo CD for GitOps".to_string(),
            source_ids: vec![],
            confidence: Confidence::High,
            conflict_note: None,
        });
        let json = serde_json::to_string(&e).unwrap();
        let parsed: Execution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.question, "What is Harness CD?");
        assert_eq!(parsed.stages.len(), 1);
        assert_eq!(parsed.claims.len(), 1);
    }

    #[test]
    fn confidence_summary_format() {
        let mut e = Execution::new("ask", "q");
        e.claims.push(Claim {
            id: 0,
            text: "a".into(),
            source_ids: vec![],
            confidence: Confidence::High,
            conflict_note: None,
        });
        e.claims.push(Claim {
            id: 1,
            text: "b".into(),
            source_ids: vec![],
            confidence: Confidence::Medium,
            conflict_note: None,
        });
        let s = e.confidence_summary();
        assert!(s.contains("1 high"));
        assert!(s.contains("1 medium"));
    }

    #[test]
    fn confidence_display() {
        assert_eq!(Confidence::High.to_string(), "high");
        assert_eq!(Confidence::Contested.to_string(), "contested");
    }
}
