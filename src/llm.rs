//! Multi-provider chat with native tool calling (OpenAI-compat + Anthropic).
//! Per Anthropic + Pi: the model uses tools in a loop — not fragile free-text JSON.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{ProviderKind, ProviderSpec};

// ── Public types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallMsg>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallMsg {
    pub id: String,
    #[serde(rename = "type", default = "default_fn_type")]
    pub kind: String,
    pub function: FunctionCall,
}

fn default_fn_type() -> String {
    "function".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    /// JSON-encoded arguments object.
    pub arguments: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    pub fn assistant_text(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    pub fn assistant_tools(calls: Vec<ToolCallMsg>) -> Self {
        Self {
            role: "assistant".into(),
            content: None,
            tool_calls: Some(calls),
            tool_call_id: None,
            name: None,
        }
    }
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Debug, Clone, Default)]
pub struct ChatUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCallMsg>,
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
    #[error("rate limited — retry later")]
    RateLimited,
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}\n... (truncated)", &s[..n])
    }
}

/// Chat with optional native tools. Retries 429 / 5xx / transport with backoff.
pub fn chat(
    provider: &ProviderSpec,
    api_key: Option<&str>,
    model: &str,
    messages: &[ChatMessage],
    temperature: f32,
    max_tokens: u32,
    tools: &[ToolSpec],
) -> Result<ChatResponse, LlmError> {
    if provider.requires_key && api_key.is_none() {
        return Err(LlmError::NoKey);
    }
    let mut last = None;
    for attempt in 0..3u32 {
        let result = match provider.kind {
            ProviderKind::OpenAiCompat => chat_openai(
                provider,
                api_key,
                model,
                messages,
                temperature,
                max_tokens,
                tools,
            ),
            ProviderKind::Anthropic => chat_anthropic(
                provider,
                api_key,
                model,
                messages,
                temperature,
                max_tokens,
                tools,
            ),
        };
        match result {
            Ok(r) => return Ok(r),
            Err(e) => {
                let retryable = matches!(
                    e,
                    LlmError::RateLimited
                        | LlmError::Http(_)
                        | LlmError::Api {
                            status: 429 | 500 | 502 | 503 | 529,
                            ..
                        }
                );
                last = Some(e);
                if !retryable || attempt == 2 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(400 * 2u64.pow(attempt)));
            }
        }
    }
    Err(last.unwrap_or(LlmError::Http("unknown".into())))
}

fn chat_openai(
    provider: &ProviderSpec,
    api_key: Option<&str>,
    model: &str,
    messages: &[ChatMessage],
    temperature: f32,
    max_tokens: u32,
    tools: &[ToolSpec],
) -> Result<ChatResponse, LlmError> {
    let url = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );
    let mut body = json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect(),
        );
        body["tool_choice"] = json!("auto");
    }

    let mut req = ureq::post(&url)
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(120));
    if let Some(k) = api_key {
        req = req.set("Authorization", &format!("Bearer {k}"));
    }

    let resp = req.send_json(body).map_err(map_ureq_err)?;
    let raw = resp
        .into_string()
        .map_err(|e| LlmError::Http(e.to_string()))?;

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
        tool_calls: Option<Vec<ToolCallMsg>>,
    }
    #[derive(Deserialize)]
    struct Usage {
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
    }

    let parsed: Resp = serde_json::from_str(&raw)
        .map_err(|e| LlmError::Parse(format!("{e}: {}", truncate(&raw, 400))))?;
    let msg = parsed
        .choices
        .first()
        .map(|c| &c.message)
        .ok_or(LlmError::NoChoices)?;

    Ok(ChatResponse {
        content: msg.content.clone().unwrap_or_default(),
        tool_calls: msg.tool_calls.clone().unwrap_or_default(),
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
    tools: &[ToolSpec],
) -> Result<ChatResponse, LlmError> {
    let key = api_key.ok_or(LlmError::NoKey)?;
    let system = messages
        .iter()
        .find(|m| m.role == "system")
        .and_then(|m| m.content.clone());

    let mut anth_messages: Vec<Value> = Vec::new();
    for m in messages.iter().filter(|m| m.role != "system") {
        match m.role.as_str() {
            "user" => {
                anth_messages.push(json!({
                    "role": "user",
                    "content": m.content.clone().unwrap_or_default(),
                }));
            }
            "assistant" => {
                if let Some(calls) = &m.tool_calls {
                    let mut blocks: Vec<Value> = Vec::new();
                    if let Some(text) = &m.content {
                        if !text.is_empty() {
                            blocks.push(json!({"type": "text", "text": text}));
                        }
                    }
                    for c in calls {
                        let input: Value =
                            serde_json::from_str(&c.function.arguments).unwrap_or(json!({}));
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": c.id,
                            "name": c.function.name,
                            "input": input,
                        }));
                    }
                    anth_messages.push(json!({"role": "assistant", "content": blocks}));
                } else {
                    anth_messages.push(json!({
                        "role": "assistant",
                        "content": m.content.clone().unwrap_or_default(),
                    }));
                }
            }
            "tool" => {
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                    "content": m.content.clone().unwrap_or_default(),
                });
                if let Some(last) = anth_messages.last_mut() {
                    if last.get("role").and_then(|r| r.as_str()) == Some("user") {
                        match last.get_mut("content") {
                            Some(Value::Array(arr)) => {
                                arr.push(block);
                                continue;
                            }
                            Some(Value::String(s)) => {
                                let prev = s.clone();
                                *last = json!({
                                    "role": "user",
                                    "content": [
                                        {"type": "text", "text": prev},
                                        block
                                    ]
                                });
                                continue;
                            }
                            _ => {}
                        }
                    }
                }
                anth_messages.push(json!({
                    "role": "user",
                    "content": [block],
                }));
            }
            _ => {}
        }
    }

    if anth_messages.is_empty() {
        return Err(LlmError::Parse(
            "Anthropic requires at least one user/assistant message".into(),
        ));
    }

    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "messages": anth_messages,
    });
    if let Some(s) = system {
        body["system"] = json!(s);
    }
    if !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect(),
        );
    }

    let url = format!("{}/messages", provider.base_url.trim_end_matches('/'));
    let req = ureq::post(&url)
        .set("Content-Type", "application/json")
        .set("x-api-key", key)
        .set("anthropic-version", "2023-06-01")
        .timeout(std::time::Duration::from_secs(120));

    let resp = req.send_json(body).map_err(map_ureq_err)?;
    let raw = resp
        .into_string()
        .map_err(|e| LlmError::Http(e.to_string()))?;

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
        id: Option<String>,
        name: Option<String>,
        input: Option<Value>,
    }
    #[derive(Deserialize)]
    struct AnthropicUsage {
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
    }

    let parsed: Resp = serde_json::from_str(&raw)
        .map_err(|e| LlmError::Parse(format!("{e}: {}", truncate(&raw, 400))))?;

    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();
    for b in parsed.content {
        match b.kind.as_str() {
            "text" => {
                if let Some(t) = b.text {
                    text_parts.push(t);
                }
            }
            "tool_use" => {
                let id = b.id.unwrap_or_else(|| format!("tool_{}", tool_calls.len()));
                let name = b.name.unwrap_or_default();
                let args = b
                    .input
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "{}".into());
                tool_calls.push(ToolCallMsg {
                    id,
                    kind: "function".into(),
                    function: FunctionCall {
                        name,
                        arguments: args,
                    },
                });
            }
            _ => {}
        }
    }

    if text_parts.is_empty() && tool_calls.is_empty() {
        return Err(LlmError::NoChoices);
    }

    Ok(ChatResponse {
        content: text_parts.join(""),
        tool_calls,
        usage: ChatUsage {
            input_tokens: parsed.usage.as_ref().and_then(|u| u.input_tokens),
            output_tokens: parsed.usage.as_ref().and_then(|u| u.output_tokens),
        },
        model: parsed.model,
    })
}

fn map_ureq_err(e: ureq::Error) -> LlmError {
    match e {
        ureq::Error::Status(429, r) => {
            let _ = r.into_string();
            LlmError::RateLimited
        }
        ureq::Error::Status(s, r) => {
            let body = r.into_string().unwrap_or_default();
            LlmError::Api { status: s, body }
        }
        ureq::Error::Transport(t) => LlmError::Http(t.to_string()),
    }
}

pub fn approximate_cost(model: &str, input: u32, output: u32) -> f64 {
    let (in_rate, out_rate) = match model {
        m if m.starts_with("gpt-4o-mini") => (0.15, 0.60),
        m if m.starts_with("gpt-4o") => (2.50, 10.00),
        m if m.starts_with("gpt-4.1") => (2.00, 8.00),
        m if m.contains("haiku") => (0.80, 4.00),
        m if m.contains("sonnet") => (3.00, 15.00),
        m if m.contains("opus") => (15.00, 75.00),
        m if m.starts_with("gemini-1.5-flash") || m.starts_with("gemini-2") => (0.075, 0.30),
        m if m.starts_with("gemini-1.5-pro") => (3.50, 10.50),
        m if m.contains("8b") => (0.05, 0.08),
        m if m.contains("70b") => (0.59, 0.79),
        _ => (0.0, 0.0),
    };
    (input as f64 / 1_000_000.0) * in_rate + (output as f64 / 1_000_000.0) * out_rate
}
