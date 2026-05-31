use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct GenerateRequest {
    pub command: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub cheap: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub verbose: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub show_cost: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub no_cache: bool,
}

fn is_false(b: &bool) -> bool { !*b }

#[derive(Debug, Deserialize)]
pub struct GenerateResponse {
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub agent_spec: Option<Value>,
    #[serde(default)]
    pub created_files: Option<Vec<String>>,
    #[serde(default)]
    pub valid: Option<bool>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub tool_count: Option<u64>,
    #[serde(default)]
    pub dependencies: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub modes: Option<Value>,
}

fn python_json_command() -> Command {
    let mut cmd = Command::new("python");
    cmd.args(["-m", "scaff.json_mode"]);
    cmd
}

pub fn call_generate(req: &GenerateRequest) -> Result<GenerateResponse, String> {
    let input = serde_json::to_string(req)
        .map_err(|e| format!("Failed to serialize request: {e}"))?;

    let mut child = python_json_command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn Python process: {e}"))?;

    use std::io::Write;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(input.as_bytes())
            .map_err(|e| format!("Failed to write to Python stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for Python process: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Python backend failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: GenerateResponse = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse Python response: {e}\nResponse: {stdout}"))?;

    Ok(response)
}
