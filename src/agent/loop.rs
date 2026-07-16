//! Pi-style agent loop: LLM ↔ native tools until finish or max_steps.
//! Anthropic: simple composable loop, transparent steps, well-documented tools.

use std::time::Instant;

use anyhow::{anyhow, Result};
use serde_json::json;

use crate::config::ProviderSpec;
use crate::display;
use crate::llm::{self, ChatMessage, ToolCallMsg};
use crate::report;
use crate::search::Index;

use super::prompt;
use super::tools::{self, ToolContext, ToolOutcome};
use super::types::{AgentOptions, AgentResult, AgentStats, FinishPayload, ParsedToolCall};

/// Run the research agent to completion.
pub fn run(
    question: &str,
    provider: &ProviderSpec,
    api_key: Option<&str>,
    index: &Index,
    options: &AgentOptions,
) -> Result<AgentResult> {
    let question = question.trim();
    if question.len() < 3 {
        anyhow::bail!("Question must be at least 3 characters");
    }
    if question.len() > 5000 {
        anyhow::bail!("Question is too long ({} chars, max 5000)", question.len());
    }

    let started = Instant::now();
    let mut stats = AgentStats::default();
    let mut steps_log: Vec<String> = Vec::new();
    let tool_specs = tools::tool_specs();

    let system = prompt::system_prompt(options.use_case);
    let mut messages = vec![
        ChatMessage::system(system),
        ChatMessage::user(prompt::user_message(question)),
    ];

    let ctx = ToolContext {
        index,
        web_enabled: options.web_enabled,
        retrieval_k: options.retrieval_k.max(1),
    };

    let max_steps = options.max_steps.clamp(1, 16);
    let mut did_search = false;

    for step in 1..=max_steps {
        stats.steps = step;
        if options.show_stages {
            display::show_stage("think", &format!("step {step}/{max_steps}"));
        }

        let resp = llm::chat(
            provider,
            api_key,
            &options.model,
            &messages,
            options.temperature,
            options.max_output_tokens,
            &tool_specs,
        )
        .map_err(|e| anyhow!("LLM request failed: {e}"))?;

        stats.input_tokens += resp.usage.input_tokens.unwrap_or(0);
        stats.output_tokens += resp.usage.output_tokens.unwrap_or(0);

        // Prefer native tool calls; fall back to JSON-in-text for weaker models.
        let calls = resolve_tool_calls(&resp.tool_calls, &resp.content);

        if calls.is_empty() {
            // Model returned prose — nudge once, then best-effort finish.
            if step < max_steps && !resp.content.trim().is_empty() {
                steps_log.push(format!("step {step}: no tool call — nudging"));
                messages.push(ChatMessage::assistant_text(resp.content.clone()));
                messages.push(ChatMessage::user(
                    "You must call a tool (search_corpus, web_search, fetch_url, or finish). \
                     Do not reply with free text only.",
                ));
                continue;
            }
            if !resp.content.trim().is_empty() {
                steps_log.push(format!("step {step}: free-text finish (fallback)"));
                let payload = FinishPayload::from_free_text(&resp.content);
                return Ok(finalize(payload, stats, steps_log, started, &options.model));
            }
            anyhow::bail!("model returned empty response at step {step}");
        }

        // Record assistant turn with native tool calls (or synthetic ids for text JSON).
        let native_msgs: Vec<ToolCallMsg> = calls
            .iter()
            .map(|(id, c)| ToolCallMsg {
                id: id.clone(),
                kind: "function".into(),
                function: llm::FunctionCall {
                    name: c.name.clone(),
                    arguments: c.args.to_string(),
                },
            })
            .collect();
        messages.push(ChatMessage::assistant_tools(native_msgs));

        // Execute tools (serial; finish ends the run).
        let mut finished: Option<FinishPayload> = None;
        for (call_id, call) in &calls {
            stats.tool_calls += 1;
            let tool_name = call.name.as_str();
            if options.show_stages {
                display::show_stage(tool_name, "running");
            }
            steps_log.push(format!("step {step}: → {tool_name}"));

            match tools::dispatch(&ctx, call) {
                Ok(ToolOutcome::Finished(payload)) => {
                    if options.show_stages {
                        display::stage_ok("finish", "report ready");
                    }
                    // Soft quality gate: prefer having searched if corpus has data.
                    if !did_search && !index.is_empty() && step < max_steps {
                        steps_log.push(format!(
                            "step {step}: finish without corpus search — accepting"
                        ));
                    }
                    steps_log.push(format!("step {step}: finish ok"));
                    finished = Some(payload);
                    break;
                }
                Ok(ToolOutcome::Continue(result_text)) => {
                    match tool_name {
                        "search_corpus" => {
                            stats.corpus_searches += 1;
                            did_search = true;
                        }
                        "web_search" => {
                            stats.web_searches += 1;
                            did_search = true;
                        }
                        "fetch_url" => stats.fetches += 1,
                        _ => {}
                    }
                    if options.show_stages {
                        let preview: String = result_text.chars().take(72).collect();
                        display::stage_ok(tool_name, &preview);
                    }
                    messages.push(ChatMessage::tool_result(call_id, result_text));
                }
                Err(e) => {
                    let err = e.to_string();
                    steps_log.push(format!("step {step}: {tool_name} error — {err}"));
                    if options.show_stages {
                        display::stage_ok(tool_name, &format!("error: {err}"));
                    }
                    let err_json = json!({"ok": false, "error": err}).to_string();
                    messages.push(ChatMessage::tool_result(call_id, err_json));
                }
            }
        }

        if let Some(payload) = finished {
            return Ok(finalize(payload, stats, steps_log, started, &options.model));
        }
    }

    // Max steps: force a structured finish from whatever we have.
    steps_log.push(format!("max_steps ({max_steps}) — forcing finish"));
    if options.show_stages {
        display::show_stage("finish", "max steps — requesting final report");
    }
    messages.push(ChatMessage::user(
        "You have reached the step limit. Call finish now with the best report you can \
         from the evidence so far. Include known_unknowns for gaps.",
    ));

    let resp = llm::chat(
        provider,
        api_key,
        &options.model,
        &messages,
        0.1,
        options.max_output_tokens,
        &tool_specs,
    )
    .map_err(|e| anyhow!("LLM request failed on forced finish: {e}"))?;
    stats.input_tokens += resp.usage.input_tokens.unwrap_or(0);
    stats.output_tokens += resp.usage.output_tokens.unwrap_or(0);
    stats.steps += 1;

    let calls = resolve_tool_calls(&resp.tool_calls, &resp.content);
    for (_, call) in calls {
        if call.name == "finish" {
            if let Ok(ToolOutcome::Finished(payload)) = tools::dispatch(&ctx, &call) {
                steps_log.push("forced finish ok".into());
                return Ok(finalize(payload, stats, steps_log, started, &options.model));
            }
        }
    }

    if !resp.content.trim().is_empty() {
        let payload = FinishPayload::from_free_text(&resp.content);
        return Ok(finalize(payload, stats, steps_log, started, &options.model));
    }

    Err(anyhow!(
        "agent exceeded max steps ({max_steps}) without finishing. \
         Try a narrower question, scaff doctor, or --stages to debug."
    ))
}

/// Prefer native tool_calls; else parse JSON-in-text as a single tool call.
fn resolve_tool_calls(native: &[ToolCallMsg], content: &str) -> Vec<(String, ParsedToolCall)> {
    if !native.is_empty() {
        return native
            .iter()
            .filter_map(|tc| {
                let call = tools::from_native(&tc.function.name, &tc.function.arguments).ok()?;
                Some((tc.id.clone(), call))
            })
            .collect();
    }
    if let Ok(call) = tools::parse_tool_call(content) {
        return vec![("text_json_0".into(), call)];
    }
    Vec::new()
}

fn finalize(
    payload: FinishPayload,
    mut stats: AgentStats,
    steps_log: Vec<String>,
    started: Instant,
    model: &str,
) -> AgentResult {
    stats.elapsed_ms = started.elapsed().as_millis();
    stats.estimated_cost_usd =
        llm::approximate_cost(model, stats.input_tokens, stats.output_tokens);
    let report = report::render(&payload);
    AgentResult {
        report,
        payload,
        stats,
        steps_log,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Index;

    #[test]
    fn rejects_short_question() {
        let idx = Index::build(vec![]);
        let opts = AgentOptions::default();
        // PROVIDERS[0] is openai which requires a key — short-question fails first.
        let err = run("hi", &crate::config::PROVIDERS[0], None, &idx, &opts).unwrap_err();
        assert!(err.to_string().contains("3 characters"));
    }

    #[test]
    fn resolve_prefers_native() {
        let native = vec![ToolCallMsg {
            id: "c1".into(),
            kind: "function".into(),
            function: llm::FunctionCall {
                name: "search_corpus".into(),
                arguments: r#"{"query":"canary"}"#.into(),
            },
        }];
        let out = resolve_tool_calls(&native, r#"{"tool":"finish","args":{}}"#);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1.name, "search_corpus");
    }
}
