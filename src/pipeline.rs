use std::collections::BTreeMap;
use std::time::Instant;

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
    pub fn from_score(score: f64, has_contradiction: bool) -> Self {
        if has_contradiction {
            Confidence::Contested
        } else if score >= 0.8 {
            Confidence::High
        } else if score >= 0.6 {
            Confidence::Medium
        } else {
            Confidence::Low
        }
    }

    pub fn emoji(&self) -> &str {
        match self {
            Confidence::High => "✅",
            Confidence::Medium => "⚠️",
            Confidence::Low => "🔴",
            Confidence::Contested => "⚡",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Confidence::High => "Multiple sources agree",
            Confidence::Medium => "Single source; verify before acting",
            Confidence::Low => "Limited corroboration; treat as hypothesis",
            Confidence::Contested => "Sources disagree; see conflict note",
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

pub struct StageTimer {
    stage: Stage,
    started: Instant,
}

impl StageTimer {
    pub fn start(name: impl Into<String>) -> Self {
        Self {
            stage: Stage::new(name),
            started: Instant::now(),
        }
    }

    pub fn detail(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.stage.details.insert(key.into(), value.into());
        self
    }

    pub fn finish(mut self, status: StageStatus) -> Stage {
        self.stage.finished_at = Some(Utc::now());
        self.stage.duration_ms = self.started.elapsed().as_millis();
        self.stage.status = status;
        self.stage
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
    pub source_ids: Vec<usize>,
    pub confidence: Confidence,
    pub conflict_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub claim_id: usize,
    pub source_id: usize,
    pub supported: bool,
    pub evidence: String,
    pub stance: Stance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stance {
    Supports,
    Contradicts,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub index: usize,
    pub total: usize,
    pub confidence: Confidence,
    pub content: String,
    pub sources: Vec<Source>,
    pub conflict_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub query: String,
    pub status: SessionStatus,
    pub sub_questions: Vec<SubQuestion>,
    pub findings: Vec<Finding>,
    pub synthesis: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Planning,
    Searching,
    Verifying,
    Synthesizing,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubQuestion {
    pub id: String,
    pub question: String,
    pub source_tag: SourceTag,
    pub priority: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceTag {
    HarnessCorpus,
    Web,
    Both,
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
    pub verifications: Vec<VerificationResult>,
    pub findings: Vec<Finding>,
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
            verifications: Vec::new(),
            findings: Vec::new(),
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

    pub fn overall_confidence(&self) -> f64 {
        if self.claims.is_empty() {
            return 0.0;
        }
        let total: f64 = self.claims.iter().map(|c| match c.confidence {
            Confidence::High => 1.0,
            Confidence::Medium => 0.7,
            Confidence::Low => 0.4,
            Confidence::Contested => 0.2,
        }).sum();
        total / self.claims.len() as f64
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub fn verified_count(&self) -> usize {
        self.verifications.iter().filter(|v| v.supported).count()
    }

    pub fn high_confidence_count(&self) -> usize {
        self.claims.iter().filter(|c| c.confidence == Confidence::High).count()
    }

    pub fn contested_count(&self) -> usize {
        self.claims.iter().filter(|c| c.confidence == Confidence::Contested).count()
    }

    pub fn confidence_summary(&self) -> String {
        let high = self.high_confidence_count();
        let medium = self.claims.iter().filter(|c| c.confidence == Confidence::Medium).count();
        let low = self.claims.iter().filter(|c| c.confidence == Confidence::Low).count();
        let contested = self.contested_count();
        format!(
            "✅ {} high | ⚠️ {} medium | 🔴 {} low | ⚡ {} contested",
            high, medium, low, contested
        )
    }
}

pub fn new_id() -> String {
    let now = Utc::now();
    let stamp = now.format("%Y%m%d-%H%M%S").to_string();
    let suffix: u32 = (now.timestamp_subsec_nanos() % 9999) + 1;
    format!("exec-{stamp}-{suffix:04}")
}

pub const PIPELINE_STAGES: &[&str] = &["plan", "broad_search", "verify", "synthesize", "render"];

pub fn deduplicate_sources(sources: &mut Vec<Source>) {
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();
    sources.retain(|s| seen_urls.insert(s.url.clone()));
}

pub fn harness_first_routing(
    question: &str,
    corpus_relevance: f64,
) -> (bool, bool) {
    let q = question.to_lowercase();
    let is_harness_topic = q.contains("harness")
        || q.contains("continuous delivery")
        || q.contains("continuous integration")
        || q.contains("gitops")
        || q.contains("feature flag")
        || q.contains("devops")
        || q.contains("canary")
        || q.contains("blue/green")
        || q.contains("rollback");

    if is_harness_topic && corpus_relevance >= 0.75 {
        (true, true)
    } else if is_harness_topic {
        (true, true)
    } else {
        (false, true)
    }
}

pub fn retry_with_backoff<F, T, E>(
    mut f: F,
    max_retries: u32,
    base_delay_ms: u64,
) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    let mut last_err = None;
    for attempt in 0..=max_retries {
        match f() {
            Ok(val) => return Ok(val),
            Err(e) => {
                last_err = Some(e);
                if attempt < max_retries {
                    let delay = base_delay_ms * 2u64.pow(attempt);
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
            }
        }
    }
    Err(last_err.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_timer_records_duration() {
        let t = StageTimer::start("plan");
        std::thread::sleep(std::time::Duration::from_millis(5));
        let s = t.finish(StageStatus::Ok);
        assert!(s.duration_ms >= 4);
        assert_eq!(s.status, StageStatus::Ok);
        assert!(s.finished_at.is_some());
    }

    #[test]
    fn execution_id_is_unique() {
        let a = new_id();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let b = new_id();
        assert_ne!(a, b);
        assert!(a.starts_with("exec-"));
    }

    #[test]
    fn execution_serializes() {
        let mut e = Execution::new("research", "What is Harness CD?");
        e.stages.push(Stage::new("plan").detail("sub_questions", 4));
        e.status = ExecutionStatus::Succeeded;
        e.finished_at = Some(Utc::now());
        e.claims.push(Claim {
            id: 0,
            text: "Harness uses Argo CD for GitOps".to_string(),
            source_ids: vec![0],
            confidence: Confidence::High,
            conflict_note: None,
        });
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("research"));
        assert!(json.contains("sub_questions"));
        assert!(json.contains("confidence"));
        let parsed: Execution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.stages.len(), 1);
        assert_eq!(parsed.claims.len(), 1);
    }

    #[test]
    fn overall_confidence_empty_claims() {
        let e = Execution::new("test", "question");
        assert_eq!(e.overall_confidence(), 0.0);
    }

    #[test]
    fn overall_confidence_weighted_average() {
        let mut e = Execution::new("test", "question");
        e.claims.push(Claim { id: 0, text: "a".into(), source_ids: vec![], confidence: Confidence::High, conflict_note: None });
        e.claims.push(Claim { id: 1, text: "b".into(), source_ids: vec![], confidence: Confidence::Low, conflict_note: None });
        let conf = e.overall_confidence();
        assert!((conf - 0.7).abs() < 0.01);
    }

    #[test]
    fn pipeline_stages_constant() {
        assert_eq!(PIPELINE_STAGES.len(), 5);
        assert_eq!(PIPELINE_STAGES[0], "plan");
        assert_eq!(PIPELINE_STAGES[4], "render");
    }

    #[test]
    fn confidence_from_score() {
        assert_eq!(Confidence::from_score(0.9, false), Confidence::High);
        assert_eq!(Confidence::from_score(0.7, false), Confidence::Medium);
        assert_eq!(Confidence::from_score(0.3, false), Confidence::Low);
        assert_eq!(Confidence::from_score(0.9, true), Confidence::Contested);
    }

    #[test]
    fn confidence_display() {
        assert_eq!(Confidence::High.to_string(), "high");
        assert_eq!(Confidence::Contested.to_string(), "contested");
    }

    #[test]
    fn deduplicate_sources_removes_dupes() {
        let mut sources = vec![
            Source { id: 0, title: "A".into(), url: "http://a.com".into(), kind: SourceKind::Web, score: 0.9 },
            Source { id: 1, title: "A2".into(), url: "http://a.com".into(), kind: SourceKind::Web, score: 0.8 },
            Source { id: 2, title: "B".into(), url: "http://b.com".into(), kind: SourceKind::Web, score: 0.7 },
        ];
        deduplicate_sources(&mut sources);
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn harness_first_routing_detects_harness() {
        let (corpus, web) = harness_first_routing("How does Harness CD work?", 0.8);
        assert!(corpus);
        assert!(web);
    }

    #[test]
    fn harness_first_routing_general_topic() {
        let (corpus, web) = harness_first_routing("What is Kubernetes?", 0.5);
        assert!(!corpus);
        assert!(web);
    }

    #[test]
    fn confidence_summary_format() {
        let mut e = Execution::new("test", "q");
        e.claims.push(Claim { id: 0, text: "a".into(), source_ids: vec![], confidence: Confidence::High, conflict_note: None });
        e.claims.push(Claim { id: 1, text: "b".into(), source_ids: vec![], confidence: Confidence::Contested, conflict_note: None });
        let summary = e.confidence_summary();
        assert!(summary.contains("1 high"));
        assert!(summary.contains("1 contested"));
    }

    #[test]
    fn retry_with_backoff_succeeds() {
        let mut attempts = 0;
        let result = retry_with_backoff(|| {
            attempts += 1;
            if attempts < 3 {
                Err("not yet")
            } else {
                Ok("success")
            }
        }, 5, 10);
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts, 3);
    }

    #[test]
    fn retry_with_backoff_exhausts() {
        let result: Result<&str, &str> = retry_with_backoff(|| Err("always fail"), 2, 10);
        assert!(result.is_err());
    }
}
