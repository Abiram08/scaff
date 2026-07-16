//! Shared types for the Pi-style agent loop.

use serde::{Deserialize, Serialize};

/// Product use cases (v1). Shared loop; prompt addendum differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UseCase {
    #[default]
    Ask,
    Compare,
    Howto,
}

impl UseCase {
    pub fn as_str(self) -> &'static str {
        match self {
            UseCase::Ask => "ask",
            UseCase::Compare => "compare",
            UseCase::Howto => "howto",
        }
    }

    pub fn pipeline_name(self) -> &'static str {
        match self {
            UseCase::Ask => "ask",
            UseCase::Compare => "compare",
            UseCase::Howto => "howto",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
    Contested,
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

    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
            Confidence::Contested => "contested",
        }
    }
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Confidence::Medium
    }
}

/// Parse confidence from free-form model output.
pub fn parse_confidence(s: &str) -> Confidence {
    let t = s.trim().to_lowercase();
    if t.contains("contest") || t.contains("conflict") {
        Confidence::Contested
    } else if t.starts_with('h') || t.contains("high") {
        Confidence::High
    } else if t.starts_with('l') || t.contains("low") {
        Confidence::Low
    } else {
        Confidence::Medium
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    "corpus".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub text: String,
    #[serde(default)]
    pub confidence: Confidence,
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub conflict_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FinishPayload {
    #[serde(default)]
    pub tldr: String,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub known_unknowns: Vec<String>,
    #[serde(default)]
    pub sources: Vec<SourceRef>,
}

impl FinishPayload {
    pub fn is_usable(&self) -> bool {
        !self.tldr.trim().is_empty() || !self.findings.is_empty()
    }

    /// Best-effort recovery when the model returns free text instead of finish.
    pub fn from_free_text(text: &str) -> Self {
        let t = text.trim();
        Self {
            tldr: t.lines().take(3).collect::<Vec<_>>().join(" "),
            findings: vec![Finding {
                text: t.to_string(),
                confidence: Confidence::Low,
                source_ids: Vec::new(),
                conflict_note: Some(
                    "Model did not call finish; report is best-effort and unverified.".into(),
                ),
            }],
            known_unknowns: vec![
                "Structured finish payload missing — treat confidence as low.".into(),
            ],
            sources: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentOptions {
    pub use_case: UseCase,
    pub retrieval_k: usize,
    pub web_enabled: bool,
    pub model: String,
    pub temperature: f32,
    pub max_output_tokens: u32,
    pub max_steps: usize,
    pub show_stages: bool,
    pub show_cost: bool,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            use_case: UseCase::Ask,
            retrieval_k: 8,
            web_enabled: true,
            model: String::new(),
            temperature: 0.2,
            max_output_tokens: 2048,
            max_steps: 8,
            show_stages: false,
            show_cost: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentStats {
    pub steps: usize,
    pub tool_calls: usize,
    pub corpus_searches: usize,
    pub web_searches: usize,
    pub fetches: usize,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub elapsed_ms: u128,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone)]
pub struct AgentResult {
    pub report: String,
    pub payload: FinishPayload,
    pub stats: AgentStats,
    pub steps_log: Vec<String>,
}

/// One tool invocation from the model.
#[derive(Debug, Clone)]
pub struct ParsedToolCall {
    pub name: String,
    pub args: serde_json::Value,
}
