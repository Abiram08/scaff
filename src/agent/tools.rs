//! Exactly four tools: search_corpus, web_search, fetch_url, finish.
//! Tool schemas are the agent-computer interface (Anthropic: craft ACI carefully).

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::llm::ToolSpec;
use crate::search::Index;
use crate::web;

use super::types::{parse_confidence, Confidence, Finding, FinishPayload, ParsedToolCall};

pub const TOOL_NAMES: &[&str] = &["search_corpus", "web_search", "fetch_url", "finish"];

/// Max characters returned from any single tool (context engineering).
pub const TOOL_RESULT_MAX_CHARS: usize = 6000;

pub struct ToolContext<'a> {
    pub index: &'a Index,
    pub web_enabled: bool,
    pub retrieval_k: usize,
}

pub enum ToolOutcome {
    /// Continue the loop; content is tool result text for the model.
    Continue(String),
    /// Model called finish with a validated/parsed payload.
    Finished(FinishPayload),
}

/// Native tool definitions for OpenAI / Anthropic tool-calling APIs.
pub fn tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "search_corpus",
            description:
                "Search the local Harness documentation corpus (preferred first step for Harness topics). Returns ranked chunks with id, title, url, snippet, score.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "k": { "type": "integer", "description": "Max results (1-20)", "minimum": 1, "maximum": 20 }
                },
                "required": ["query"]
            }),
        },
        ToolSpec {
            name: "web_search",
            description:
                "Search the open web when the corpus is thin or for non-Harness context. Skip if the tool reports it is disabled.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "k": { "type": "integer", "description": "Max results (1-10)", "minimum": 1, "maximum": 10 }
                },
                "required": ["query"]
            }),
        },
        ToolSpec {
            name: "fetch_url",
            description:
                "Fetch and extract text from an https URL for more detail on a promising source.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "https URL to fetch" }
                },
                "required": ["url"]
            }),
        },
        ToolSpec {
            name: "finish",
            description:
                "End the research run and emit the final cited report. Call when you can answer (or clearly cannot).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "tldr": { "type": "string", "description": "1-3 sentence summary" },
                    "findings": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "text": { "type": "string" },
                                "confidence": {
                                    "type": "string",
                                    "enum": ["high", "medium", "low", "contested"]
                                },
                                "source_ids": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "conflict_note": { "type": ["string", "null"] }
                            },
                            "required": ["text", "confidence", "source_ids"]
                        }
                    },
                    "known_unknowns": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "sources": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "title": { "type": "string" },
                                "url": { "type": "string" },
                                "kind": { "type": "string", "enum": ["corpus", "web"] }
                            },
                            "required": ["id", "title", "url"]
                        }
                    }
                },
                "required": ["tldr", "findings", "sources"]
            }),
        },
    ]
}

pub fn dispatch(ctx: &ToolContext<'_>, call: &ParsedToolCall) -> Result<ToolOutcome> {
    match call.name.as_str() {
        "search_corpus" => Ok(ToolOutcome::Continue(truncate_result(&tool_search_corpus(
            ctx, &call.args,
        )?))),
        "web_search" => Ok(ToolOutcome::Continue(truncate_result(&tool_web_search(
            ctx, &call.args,
        )?))),
        "fetch_url" => Ok(ToolOutcome::Continue(truncate_result(&tool_fetch_url(
            &call.args,
        )?))),
        "finish" => Ok(ToolOutcome::Finished(parse_finish_args(&call.args)?)),
        other => Err(anyhow!(
            "unknown tool `{other}`. Allowed: {}",
            TOOL_NAMES.join(", ")
        )),
    }
}

fn truncate_result(s: &str) -> String {
    if s.chars().count() <= TOOL_RESULT_MAX_CHARS {
        return s.to_string();
    }
    let cut: String = s.chars().take(TOOL_RESULT_MAX_CHARS).collect();
    format!("{cut}\n…[truncated]")
}

/// Convert a native tool call (id + name + JSON args string) into ParsedToolCall.
pub fn from_native(name: &str, arguments_json: &str) -> Result<ParsedToolCall> {
    let args: Value = serde_json::from_str(arguments_json).unwrap_or_else(|_| json!({}));
    // Some models wrap finish fields incorrectly
    Ok(ParsedToolCall {
        name: name.to_string(),
        args,
    })
}

fn arg_str(args: &Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("missing or empty string arg `{key}`"))
}

fn arg_usize(args: &Value, key: &str, default: usize) -> usize {
    args.get(key)
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .map(|n| n as usize)
        .unwrap_or(default)
        .clamp(1, 20)
}

fn tool_search_corpus(ctx: &ToolContext<'_>, args: &Value) -> Result<String> {
    let query = arg_str(args, "query")?;
    let k = arg_usize(args, "k", ctx.retrieval_k);
    let hits = ctx.index.search(&query, k);

    if hits.is_empty() {
        return Ok(json!({
            "ok": true,
            "tool": "search_corpus",
            "query": query,
            "count": 0,
            "results": [],
            "note": "No corpus hits. Try a different query or web_search if enabled."
        })
        .to_string());
    }

    let results: Vec<Value> = hits
        .iter()
        .map(|h| {
            let id = format!("corpus:{}", h.chunk.id);
            let snippet = truncate(&h.chunk.content, 600);
            json!({
                "id": id,
                "title": h.chunk.title,
                "url": h.chunk.url,
                "section": h.chunk.section,
                "score": (h.score * 1000.0).round() / 1000.0,
                "snippet": snippet,
            })
        })
        .collect();

    Ok(json!({
        "ok": true,
        "tool": "search_corpus",
        "query": query,
        "count": results.len(),
        "results": results,
    })
    .to_string())
}

fn tool_web_search(ctx: &ToolContext<'_>, args: &Value) -> Result<String> {
    if !ctx.web_enabled {
        return Ok(json!({
            "ok": false,
            "tool": "web_search",
            "error": "web search is disabled (--no-web or config). Use search_corpus and finish."
        })
        .to_string());
    }
    let query = arg_str(args, "query")?;
    let k = arg_usize(args, "k", 5);
    let results = web::search(&query, k).unwrap_or_default();

    let mapped: Vec<Value> = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            json!({
                "id": format!("web:{}", i + 1),
                "title": r.title,
                "url": r.url,
                "snippet": truncate(&r.snippet, 400),
            })
        })
        .collect();

    Ok(json!({
        "ok": true,
        "tool": "web_search",
        "query": query,
        "count": mapped.len(),
        "results": mapped,
    })
    .to_string())
}

fn tool_fetch_url(args: &Value) -> Result<String> {
    let url = arg_str(args, "url")?;
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Ok(json!({
            "ok": false,
            "tool": "fetch_url",
            "error": "url must start with http:// or https://"
        })
        .to_string());
    }
    match web::fetch_excerpts(&url, 4000) {
        Ok(text) => Ok(json!({
            "ok": true,
            "tool": "fetch_url",
            "url": url,
            "text": text,
        })
        .to_string()),
        Err(e) => Ok(json!({
            "ok": false,
            "tool": "fetch_url",
            "url": url,
            "error": e.to_string(),
        })
        .to_string()),
    }
}

pub fn parse_finish_args(args: &Value) -> Result<FinishPayload> {
    // Accept either nested under args as the finish fields, or args wrapping a payload.
    let root = if args.get("tldr").is_some() || args.get("findings").is_some() {
        args.clone()
    } else if let Some(inner) = args.get("report").or_else(|| args.get("payload")) {
        inner.clone()
    } else {
        args.clone()
    };

    let mut payload: FinishPayload = serde_json::from_value(root).unwrap_or_default();

    // Re-parse findings manually if empty but raw array present.
    if payload.findings.is_empty() {
        if let Some(arr) = args.get("findings").and_then(|v| v.as_array()) {
            for item in arr {
                let text = item
                    .get("text")
                    .or_else(|| item.get("claim"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if text.is_empty() {
                    continue;
                }
                let conf = item
                    .get("confidence")
                    .and_then(|v| v.as_str())
                    .map(parse_confidence)
                    .unwrap_or(Confidence::Medium);
                let source_ids = item
                    .get("source_ids")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let conflict_note = item
                    .get("conflict_note")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                payload.findings.push(Finding {
                    text,
                    confidence: conf,
                    source_ids,
                    conflict_note,
                });
            }
        }
    }

    if payload.tldr.trim().is_empty() && payload.findings.is_empty() {
        return Err(anyhow!(
            "finish requires at least `tldr` or non-empty `findings`"
        ));
    }

    for s in &mut payload.sources {
        if s.kind.is_empty() {
            s.kind = if s.id.starts_with("web:") {
                "web".into()
            } else {
                "corpus".into()
            };
        }
    }

    Ok(payload)
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let cut: String = t.chars().take(max).collect();
    format!("{cut}…")
}

/// Parse a model reply into a tool call. Accepts raw JSON or fenced JSON.
pub fn parse_tool_call(text: &str) -> Result<ParsedToolCall> {
    let v = extract_json_value(text).ok_or_else(|| {
        anyhow!("model reply is not a JSON tool call (expected {{\"tool\":...}})")
    })?;

    // Shape: {"tool":"name","args":{...}}
    if let Some(name) = v.get("tool").and_then(|t| t.as_str()) {
        let args = v.get("args").cloned().unwrap_or_else(|| json!({}));
        return Ok(ParsedToolCall {
            name: name.to_string(),
            args,
        });
    }

    // Shape: {"name":"search_corpus","arguments":{...}}
    if let Some(name) = v.get("name").and_then(|t| t.as_str()) {
        let args = v
            .get("arguments")
            .or_else(|| v.get("args"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        return Ok(ParsedToolCall {
            name: name.to_string(),
            args,
        });
    }

    // Bare finish payload without wrapper.
    if v.get("tldr").is_some() || v.get("findings").is_some() {
        return Ok(ParsedToolCall {
            name: "finish".into(),
            args: v,
        });
    }

    Err(anyhow!(
        "JSON found but no tool name; got keys: {}",
        v.as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_default()
    ))
}

fn extract_json_value(text: &str) -> Option<Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        return Some(v);
    }
    // ```json ... ```
    if let Some(start) = t.find("```") {
        let after = &t[start + 3..];
        let after = after
            .strip_prefix("json")
            .or_else(|| after.strip_prefix("JSON"))
            .unwrap_or(after);
        let after = after.trim_start_matches(|c: char| c == '\r' || c == '\n');
        if let Some(end) = after.find("```") {
            let block = after[..end].trim();
            if let Ok(v) = serde_json::from_str::<Value>(block) {
                return Some(v);
            }
        }
    }
    // First { ... last }
    let start = t.find('{')?;
    let end = t.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&t[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{Chunk, Index};

    fn empty_index() -> Index {
        Index::build(vec![])
    }

    fn sample_index() -> Index {
        Index::build(vec![Chunk {
            id: "cd-1".into(),
            url: "https://developer.harness.io/docs/cd".into(),
            title: "Harness CD".into(),
            section: Some("Overview".into()),
            content: "Harness Continuous Delivery supports canary and blue/green deployments."
                .into(),
        }])
    }

    #[test]
    fn parse_tool_call_standard() {
        let c =
            parse_tool_call(r#"{"tool":"search_corpus","args":{"query":"canary","k":5}}"#).unwrap();
        assert_eq!(c.name, "search_corpus");
        assert_eq!(c.args["query"], "canary");
    }

    #[test]
    fn parse_bare_finish() {
        let c = parse_tool_call(r#"{"tldr":"hello","findings":[]}"#).unwrap();
        assert_eq!(c.name, "finish");
    }

    #[test]
    fn search_corpus_returns_ids() {
        let idx = sample_index();
        let ctx = ToolContext {
            index: &idx,
            web_enabled: false,
            retrieval_k: 5,
        };
        let out = dispatch(
            &ctx,
            &ParsedToolCall {
                name: "search_corpus".into(),
                args: json!({"query": "canary"}),
            },
        )
        .unwrap();
        match out {
            ToolOutcome::Continue(s) => {
                assert!(s.contains("corpus:cd-1") || s.contains("canary") || s.contains("count"));
            }
            _ => panic!("expected continue"),
        }
    }

    #[test]
    fn web_disabled_is_soft_error() {
        let idx = empty_index();
        let ctx = ToolContext {
            index: &idx,
            web_enabled: false,
            retrieval_k: 5,
        };
        let out = dispatch(
            &ctx,
            &ParsedToolCall {
                name: "web_search".into(),
                args: json!({"query": "test"}),
            },
        )
        .unwrap();
        match out {
            ToolOutcome::Continue(s) => assert!(s.contains("disabled")),
            _ => panic!("expected continue"),
        }
    }

    #[test]
    fn finish_requires_content() {
        let err = parse_finish_args(&json!({})).unwrap_err();
        assert!(err.to_string().contains("finish"));
    }

    #[test]
    fn finish_parses_findings() {
        let p = parse_finish_args(&json!({
            "tldr": "CD does canary",
            "findings": [{
                "text": "Canary is supported",
                "confidence": "high",
                "source_ids": ["corpus:cd-1"]
            }],
            "known_unknowns": [],
            "sources": [{
                "id": "corpus:cd-1",
                "title": "CD",
                "url": "https://example.com",
                "kind": "corpus"
            }]
        }))
        .unwrap();
        assert!(p.is_usable());
        assert_eq!(p.findings[0].confidence, Confidence::High);
    }
}
