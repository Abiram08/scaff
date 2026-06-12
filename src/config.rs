use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffConfig {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default = "default_corpus_dir")]
    pub corpus_dir: String,
    #[serde(default)]
    pub cheap: bool,
    #[serde(default)]
    pub show_cost: bool,
    #[serde(default = "default_retrieval_k")]
    pub retrieval_k: usize,
    #[serde(default = "default_web_enabled")]
    pub web_enabled: bool,
    #[serde(default = "default_api_keys")]
    pub api_keys: HashMap<String, String>,
}

fn default_model() -> String {
    "gpt-4o-mini".to_string()
}
fn default_provider() -> String {
    "openai".to_string()
}
fn default_corpus_dir() -> String {
    "~/.scaff/corpus".to_string()
}
fn default_retrieval_k() -> usize {
    8
}
fn default_web_enabled() -> bool {
    true
}
fn default_api_keys() -> HashMap<String, String> {
    HashMap::new()
}

impl Default for ScaffConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            default_provider: default_provider(),
            corpus_dir: default_corpus_dir(),
            cheap: false,
            show_cost: false,
            retrieval_k: default_retrieval_k(),
            web_enabled: default_web_enabled(),
            api_keys: default_api_keys(),
        }
    }
}

pub struct ProviderSpec {
    pub name: &'static str,
    pub env_var: &'static str,
    pub default_model: &'static str,
    pub base_url: &'static str,
    pub requires_key: bool,
    pub kind: ProviderKind,
}

impl std::fmt::Display for ProviderSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAiCompat,
    Anthropic,
}

pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        name: "openai",
        env_var: "OPENAI_API_KEY",
        default_model: "gpt-4o-mini",
        base_url: "https://api.openai.com/v1",
        requires_key: true,
        kind: ProviderKind::OpenAiCompat,
    },
    ProviderSpec {
        name: "anthropic",
        env_var: "ANTHROPIC_API_KEY",
        default_model: "claude-3-5-haiku-latest",
        base_url: "https://api.anthropic.com/v1",
        requires_key: true,
        kind: ProviderKind::Anthropic,
    },
    ProviderSpec {
        name: "gemini",
        env_var: "GEMINI_API_KEY",
        default_model: "gemini-1.5-flash",
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        requires_key: true,
        kind: ProviderKind::OpenAiCompat,
    },
    ProviderSpec {
        name: "groq",
        env_var: "GROQ_API_KEY",
        default_model: "llama-3.1-8b-instant",
        base_url: "https://api.groq.com/openai/v1",
        requires_key: true,
        kind: ProviderKind::OpenAiCompat,
    },
    ProviderSpec {
        name: "ollama",
        env_var: "",
        default_model: "llama3.1",
        base_url: "http://localhost:11434/v1",
        requires_key: false,
        kind: ProviderKind::OpenAiCompat,
    },
];

pub fn provider(name: &str) -> Option<&'static ProviderSpec> {
    PROVIDERS.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

pub fn provider_names() -> Vec<&'static str> {
    PROVIDERS.iter().map(|p| p.name).collect()
}

pub fn resolve_provider(name: Option<&str>) -> Result<&'static ProviderSpec, String> {
    let n = name.unwrap_or("openai");
    provider(n).ok_or_else(|| {
        format!(
            "Unknown provider '{n}'. Valid: {}",
            provider_names().join(", ")
        )
    })
}

pub fn resolve_model(provider: &ProviderSpec, override_model: Option<&str>, cheap: bool) -> String {
    if let Some(m) = override_model {
        return m.to_string();
    }
    if cheap {
        return match provider.name {
            "openai" => "gpt-4o-mini".to_string(),
            "anthropic" => "claude-3-5-haiku-latest".to_string(),
            "gemini" => "gemini-1.5-flash".to_string(),
            "groq" => "llama-3.1-8b-instant".to_string(),
            "ollama" => "llama3.1:8b".to_string(),
            _ => provider.default_model.to_string(),
        };
    }
    provider.default_model.to_string()
}

pub fn resolve_api_key(
    provider: &ProviderSpec,
    config: &ScaffConfig,
) -> Result<Option<String>, String> {
    if !provider.requires_key {
        return Ok(None);
    }
    if let Ok(v) = std::env::var(provider.env_var) {
        if !v.trim().is_empty() {
            return Ok(Some(v.trim().to_string()));
        }
    }
    if let Some(k) = config.api_keys.get(provider.name) {
        if !k.trim().is_empty() {
            return Ok(Some(k.trim().to_string()));
        }
    }
    Ok(None)
}

pub fn require_api_key(
    provider: &ProviderSpec,
    config: &ScaffConfig,
) -> Result<String, String> {
    resolve_api_key(provider, config)?.ok_or_else(|| {
        format!(
            "Missing API key for {}.\nSet it via env var {} or run: scaff config set-key {provider} <KEY>",
            provider.name,
            provider.env_var
        )
    })
}

impl ScaffConfig {
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".scaff")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn corpus_db_path() -> PathBuf {
        Self::config_dir().join("corpus.db")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            return Self::default();
        }
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
        let path = Self::config_path();
        let body = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;
        write_atomic(&path, &body)
    }

    pub fn set_api_key(&mut self, provider: &str, key: String) {
        self.api_keys.insert(provider.to_string(), key);
    }
}

pub fn write_atomic(path: &Path, body: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, body).map_err(|e| format!("Failed to write temp file: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("Failed to rename temp file: {e}"))?;
    Ok(())
}

pub fn read_to_string_lossy(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}
