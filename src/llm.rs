use serde::{Deserialize, Serialize};

use crate::config::{ProviderKind, ProviderSpec};

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".to_string(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".to_string(), content: content.into() }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChatUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub usage: ChatUsage,
    pub model: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("API error {status}: {}", truncate(body, 600))]
    Api { status: u16, body: String },
    #[error("Missing API key")]
    NoKey,
    #[error("API returned no choices")]
    NoChoices,
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}\n... (truncated)", &s[..n])
    }
}

pub fn chat(
    provider: &ProviderSpec,
    api_key: Option<&str>,
    model: &str,
    messages: &[ChatMessage],
    temperature: f32,
    max_tokens: u32,
) -> Result<ChatResponse, LlmError> {
    if provider.requires_key && api_key.is_none() {
        return Err(LlmError::NoKey);
    }
    match provider.kind {
        ProviderKind::OpenAiCompat => chat_openai_compat(provider, api_key, model, messages, temperature, max_tokens),
        ProviderKind::Anthropic => chat_anthropic(provider, api_key, model, messages, temperature, max_tokens),
    }
}

fn chat_openai_compat(
    provider: &ProviderSpec,
    api_key: Option<&str>,
    model: &str,
    messages: &[ChatMessage],
    temperature: f32,
    max_tokens: u32,
) -> Result<ChatResponse, LlmError> {
    #[derive(Serialize)]
    struct Req<'a> {
        model: &'a str,
        messages: &'a [ChatMessage],
        temperature: f32,
        max_tokens: u32,
    }
    #[derive(Deserialize)]
    struct Resp {
        model: String,
        choices: Vec<Choice>,
        usage: Option<Usage>,
    }
    #[derive(Deserialize)]
    struct Choice {
        message: ChoiceMsg,
    }
    #[derive(Deserialize)]
    struct ChoiceMsg {
        content: Option<String>,
    }
    #[derive(Deserialize)]
    struct Usage {
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
    }

    let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
    let body = Req { model, messages, temperature, max_tokens };

    let mut req = ureq::post(&url)
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(120));
    if let Some(k) = api_key {
        req = req.set("Authorization", &format!("Bearer {k}"));
    }

    let resp = req.send_json(serde_json::to_value(&body).map_err(|e| LlmError::Parse(e.to_string()))?)
        .map_err(|e| match e {
            ureq::Error::Status(s, r) => {
                let body = r.into_string().unwrap_or_default();
                LlmError::Api { status: s, body }
            }
            ureq::Error::Transport(t) => LlmError::Http(t.to_string()),
        })?;
    let raw = resp.into_string().map_err(|e| LlmError::Http(e.to_string()))?;
    let parsed: Resp = serde_json::from_str(&raw).map_err(|e| {
        LlmError::Parse(format!("{e}: {}", truncate(&raw, 400)))
    })?;
    let content = parsed
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or(LlmError::NoChoices)?;
    Ok(ChatResponse {
        content,
        usage: ChatUsage {
            input_tokens: parsed.usage.as_ref().and_then(|u| u.prompt_tokens),
            output_tokens: parsed.usage.as_ref().and_then(|u| u.completion_tokens),
        },
        model: parsed.model,
    })
}

fn chat_anthropic(
    provider: &ProviderSpec,
    api_key: Option<&str>,
    model: &str,
    messages: &[ChatMessage],
    temperature: f32,
    max_tokens: u32,
) -> Result<ChatResponse, LlmError> {
    #[derive(Serialize)]
    struct Req<'a> {
        model: &'a str,
        max_tokens: u32,
        temperature: f32,
        system: Option<String>,
        messages: Vec<AnthropicMsg<'a>>,
    }
    #[derive(Serialize)]
    struct AnthropicMsg<'a> {
        role: &'a str,
        content: &'a str,
    }
    #[derive(Deserialize)]
    struct Resp {
        model: String,
        content: Vec<Block>,
        usage: Option<AnthropicUsage>,
    }
    #[derive(Deserialize)]
    struct Block {
        #[serde(rename = "type")]
        kind: String,
        text: Option<String>,
    }
    #[derive(Deserialize)]
    struct AnthropicUsage {
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
    }

    let key = api_key.ok_or(LlmError::NoKey)?;
    let system = messages.iter().find(|m| m.role == "system").map(|m| m.content.clone());
    let convo: Vec<AnthropicMsg> = messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| AnthropicMsg { role: &m.role, content: &m.content })
        .collect();
    if convo.is_empty() {
        return Err(LlmError::Parse("Anthropic requires at least one user/assistant message".to_string()));
    }
    let body = Req { model, max_tokens, temperature, system, messages: convo };
    let url = format!("{}/messages", provider.base_url.trim_end_matches('/'));

    let req = ureq::post(&url)
        .set("Content-Type", "application/json")
        .set("x-api-key", key)
        .set("anthropic-version", "2023-06-01")
        .timeout(std::time::Duration::from_secs(120));

    let resp = req.send_json(serde_json::to_value(&body).map_err(|e| LlmError::Parse(e.to_string()))?)
        .map_err(|e| match e {
            ureq::Error::Status(s, r) => {
                let body = r.into_string().unwrap_or_default();
                LlmError::Api { status: s, body }
            }
            ureq::Error::Transport(t) => LlmError::Http(t.to_string()),
        })?;
    let raw = resp.into_string().map_err(|e| LlmError::Http(e.to_string()))?;
    let parsed: Resp = serde_json::from_str(&raw).map_err(|e| {
        LlmError::Parse(format!("{e}: {}", truncate(&raw, 400)))
    })?;
    let content = parsed
        .content
        .into_iter()
        .filter(|b| b.kind == "text")
        .filter_map(|b| b.text)
        .collect::<Vec<_>>()
        .join("");
    if content.is_empty() {
        return Err(LlmError::NoChoices);
    }
    Ok(ChatResponse {
        content,
        usage: ChatUsage {
            input_tokens: parsed.usage.as_ref().and_then(|u| u.input_tokens),
            output_tokens: parsed.usage.as_ref().and_then(|u| u.output_tokens),
        },
        model: parsed.model,
    })
}

pub fn approximate_cost(model: &str, input: u32, output: u32) -> f64 {
    let (in_rate, out_rate) = match model {
        m if m.starts_with("gpt-4o-mini") => (0.15, 0.60),
        m if m.starts_with("gpt-4o") => (2.50, 10.00),
        m if m.starts_with("claude-3-5-haiku") => (0.80, 4.00),
        m if m.starts_with("claude-3-5-sonnet") => (3.00, 15.00),
        m if m.starts_with("claude-3-opus") => (15.00, 75.00),
        m if m.starts_with("gemini-1.5-flash") => (0.075, 0.30),
        m if m.starts_with("gemini-1.5-pro") => (3.50, 10.50),
        m if m.starts_with("llama-3.1-8b") => (0.05, 0.08),
        m if m.starts_with("llama-3.1-70b") => (0.59, 0.79),
        _ => (0.0, 0.0),
    };
    (input as f64 / 1_000_000.0) * in_rate + (output as f64 / 1_000_000.0) * out_rate
}
