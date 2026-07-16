use std::io::{self, BufRead, Write};

use anyhow::Result;
use serde_json::{json, Value};

use crate::agent::{self, AgentOptions, UseCase};
use crate::config::{self, ScaffConfig};
use crate::corpus;
use crate::search::Index;

/// Run the MCP stdio server. Exits when stdin closes.
pub fn run() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = stdin.lock();

    let mut initialized = false;
    let mut buffer = String::new();
    while reader.read_line(&mut buffer)? != 0 {
        let line = buffer.trim().to_string();
        buffer.clear();
        if line.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let id = msg.get("id").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => {
                initialized = true;
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "serverInfo": { "name": "scaff", "version": env!("CARGO_PKG_VERSION") },
                        "capabilities": { "tools": {} }
                    }
                });
                write_msg(&mut stdout, &resp)?;
            }
            "notifications/initialized" => {
                // No response needed.
            }
            "tools/list" => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [
                            {
                                "name": "research",
                                "description": "Run a Harness research query. Returns a cited Markdown report sourced from the local corpus (and optionally the web).",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "question": {
                                            "type": "string",
                                            "description": "The research question to answer."
                                        }
                                    },
                                    "required": ["question"]
                                }
                            }
                        ]
                    }
                });
                write_msg(&mut stdout, &resp)?;
            }
            "tools/call" if initialized => {
                let params = msg.get("params").cloned().unwrap_or_default();
                let tool = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_default();
                if tool != "research" {
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32602, "message": format!("unknown tool: {tool}") }
                    });
                    write_msg(&mut stdout, &resp)?;
                    continue;
                }
                let question = args
                    .get("question")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if question.is_empty() {
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32602, "message": "missing 'question' argument" }
                    });
                    write_msg(&mut stdout, &resp)?;
                    continue;
                }
                match handle_research(&question) {
                    Ok(report) => {
                        let resp = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [
                                    { "type": "text", "text": report }
                                ]
                            }
                        });
                        write_msg(&mut stdout, &resp)?;
                    }
                    Err(e) => {
                        let resp = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32000, "message": format!("research failed: {e}") }
                        });
                        write_msg(&mut stdout, &resp)?;
                    }
                }
            }
            "tools/call" => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32001, "message": "server not initialized" }
                });
                write_msg(&mut stdout, &resp)?;
            }
            "ping" => {
                let resp = json!({ "jsonrpc": "2.0", "id": id, "result": {} });
                write_msg(&mut stdout, &resp)?;
            }
            _ => {
                if !id.is_null() {
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32601, "message": format!("method not found: {method}") }
                    });
                    write_msg(&mut stdout, &resp)?;
                }
            }
        }
    }
    Ok(())
}

fn handle_research(question: &str) -> Result<String> {
    let cfg = ScaffConfig::load();
    let provider =
        config::resolve_provider(Some(&cfg.default_provider)).map_err(anyhow::Error::msg)?;
    let api_key = config::require_api_key(provider, &cfg).map_err(anyhow::Error::msg)?;
    let model = config::resolve_model(provider, Some(&cfg.model), cfg.cheap);
    let conn = corpus::open_db()?;
    let chunks = corpus::load_all_chunks(&conn)?;
    let index = Index::build(chunks);
    let options = AgentOptions {
        use_case: UseCase::Ask,
        retrieval_k: cfg.retrieval_k,
        web_enabled: cfg.web_enabled,
        model,
        temperature: 0.2,
        max_output_tokens: 2048,
        max_steps: 8,
        show_stages: false,
        show_cost: false,
    };
    let out = agent::run(question, provider, Some(&api_key), &index, &options)?;
    Ok(out.report)
}

fn write_msg<W: Write>(w: &mut W, value: &Value) -> io::Result<()> {
    let s = serde_json::to_string(value).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    writeln!(w, "{s}")?;
    w.flush()
}
