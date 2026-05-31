use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffConfig {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default = "default_api_mode")]
    pub api_mode: String,
    #[serde(default)]
    pub cheap: bool,
    #[serde(default)]
    pub show_cost: bool,
    #[serde(default = "default_template_style")]
    pub template_style: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_model() -> String { "gpt-4o".to_string() }
fn default_output_dir() -> String { "./agent-output".to_string() }
fn default_provider() -> String { "openai".to_string() }
fn default_api_mode() -> String { "medium".to_string() }
fn default_template_style() -> String { "modern".to_string() }

impl Default for ScaffConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            output_dir: default_output_dir(),
            default_provider: default_provider(),
            api_mode: default_api_mode(),
            cheap: false,
            show_cost: false,
            template_style: default_template_style(),
            extra: HashMap::new(),
        }
    }
}

impl ScaffConfig {
    pub fn config_dir() -> PathBuf {
        let base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."));
        base.join(".scaff")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    serde_json::from_str(&content).unwrap_or_default()
                }
                Err(_) => Self::default(),
            }
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;
        fs::write(Self::config_path(), content)
            .map_err(|e| format!("Failed to write config: {e}"))?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        match key {
            "model" => Some(serde_json::Value::String(self.model.clone())),
            "output_dir" => Some(serde_json::Value::String(self.output_dir.clone())),
            "default_provider" => Some(serde_json::Value::String(self.default_provider.clone())),
            "api_mode" => Some(serde_json::Value::String(self.api_mode.clone())),
            "cheap" => Some(serde_json::Value::Bool(self.cheap)),
            "show_cost" => Some(serde_json::Value::Bool(self.show_cost)),
            "template_style" => Some(serde_json::Value::String(self.template_style.clone())),
            _ => self.extra.get(key).cloned(),
        }
    }

    pub fn set(&mut self, key: &str, value: serde_json::Value) {
        match key {
            "model" => self.model = value.as_str().unwrap_or("gpt-4o").to_string(),
            "output_dir" => self.output_dir = value.as_str().unwrap_or("./agent-output").to_string(),
            "default_provider" => self.default_provider = value.as_str().unwrap_or("openai").to_string(),
            "api_mode" => self.api_mode = value.as_str().unwrap_or("medium").to_string(),
            "cheap" => self.cheap = value.as_bool().unwrap_or(false),
            "show_cost" => self.show_cost = value.as_bool().unwrap_or(false),
            "template_style" => self.template_style = value.as_str().unwrap_or("modern").to_string(),
            _ => { self.extra.insert(key.to_string(), value); }
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
