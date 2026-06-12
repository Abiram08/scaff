use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::config::ScaffConfig;
use crate::pipeline::Execution;

pub fn executions_dir() -> PathBuf {
    ScaffConfig::config_dir().join("executions")
}

pub fn reports_dir() -> PathBuf {
    ScaffConfig::config_dir().join("reports")
}

pub fn ensure_dirs() -> Result<()> {
    fs::create_dir_all(executions_dir()).ok();
    fs::create_dir_all(reports_dir()).ok();
    Ok(())
}

pub fn save_execution(exec: &Execution) -> Result<PathBuf> {
    ensure_dirs()?;
    let path = execution_path(&exec.id);
    let body = serde_json::to_string_pretty(exec).context("serialize execution")?;
    crate::config::write_atomic(&path, &body).map_err(|e| anyhow::anyhow!("write execution file: {e}"))?;
    Ok(path)
}

pub fn execution_path(id: &str) -> PathBuf {
    executions_dir().join(format!("{id}.json"))
}

pub fn save_report(exec: &Execution, body: &str) -> Result<PathBuf> {
    ensure_dirs()?;
    let path = report_path(&exec.id, &exec.question);
    let mut f = fs::File::create(&path).with_context(|| format!("create {path:?}"))?;
    write_frontmatter(&mut f, exec)?;
    f.write_all(body.as_bytes())?;
    Ok(path)
}

pub fn report_path(id: &str, question: &str) -> PathBuf {
    let slug = slugify(question);
    reports_dir().join(format!("{id}-{slug}.md"))
}

pub fn write_frontmatter<W: Write>(w: &mut W, exec: &Execution) -> Result<()> {
    let esc = |s: &str| s.replace('"', "\\\"");
    let sources_yaml = exec
        .sources
        .iter()
        .map(|s| {
            format!(
                "  - id: {}\n    title: \"{}\"\n    url: {}\n    kind: {}\n    score: {:.3}\n",
                s.id,
                esc(&s.title),
                s.url,
                match s.kind {
                    crate::pipeline::SourceKind::Corpus => "corpus",
                    crate::pipeline::SourceKind::Web => "web",
                    crate::pipeline::SourceKind::LocalFile => "local",
                },
                s.score
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let stages_yaml = exec
        .stages
        .iter()
        .map(|s| {
            format!(
                "  - name: {}\n    status: {}\n    duration_ms: {}\n",
                s.name,
                match s.status {
                    crate::pipeline::StageStatus::Pending => "pending",
                    crate::pipeline::StageStatus::Running => "running",
                    crate::pipeline::StageStatus::Ok => "ok",
                    crate::pipeline::StageStatus::Failed => "failed",
                    crate::pipeline::StageStatus::Skipped => "skipped",
                },
                s.duration_ms
            )
        })
        .collect::<Vec<_>>()
        .join("");

    writeln!(w, "---")?;
    writeln!(w, "id: {}", exec.id)?;
    writeln!(w, "pipeline: {}", exec.pipeline)?;
    writeln!(w, "question: \"{}\"", esc(&exec.question))?;
    writeln!(w, "started_at: {}", exec.started_at.to_rfc3339())?;
    if let Some(f) = exec.finished_at {
        writeln!(w, "finished_at: {}", f.to_rfc3339())?;
    }
    writeln!(w, "model: {}", exec.model)?;
    writeln!(w, "provider: {}", exec.provider)?;
    writeln!(w, "input_tokens: {}", exec.input_tokens)?;
    writeln!(w, "output_tokens: {}", exec.output_tokens)?;
    writeln!(w, "estimated_cost_usd: {:.6}", exec.estimated_cost_usd)?;
    if let Some(t) = &exec.tldr {
        writeln!(w, "tldr: \"{}\"", esc(t))?;
    }
    if let Some(e) = &exec.error {
        writeln!(w, "error: \"{}\"", esc(e))?;
    }
    if !sources_yaml.is_empty() {
        writeln!(w, "sources:")?;
        write!(w, "{sources_yaml}")?;
    }
    if !stages_yaml.is_empty() {
        writeln!(w, "stages:")?;
        write!(w, "{stages_yaml}")?;
    }
    writeln!(w, "---")?;
    writeln!(w)?;
    Ok(())
}

pub fn load_execution(id: &str) -> Result<Execution> {
    let path = execution_path(id);
    let body = fs::read_to_string(&path).with_context(|| format!("read {path:?}"))?;
    Ok(serde_json::from_str(&body)?)
}

pub fn list_executions(limit: usize) -> Result<Vec<Execution>> {
    ensure_dirs()?;
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = fs::read_dir(executions_dir())?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                return None;
            }
            let m = e.metadata().ok()?.modified().ok()?;
            Some((p, m))
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    let mut out = Vec::new();
    for (p, _) in entries.into_iter().take(limit) {
        if let Ok(body) = fs::read_to_string(&p) {
            if let Ok(exec) = serde_json::from_str::<Execution>(&body) {
                out.push(exec);
            }
        }
    }
    Ok(out)
}

pub fn delete_execution(id: &str) -> Result<()> {
    let p = execution_path(id);
    if p.exists() {
        fs::remove_file(&p).with_context(|| format!("remove {p:?}"))?;
    }
    // also delete matching report if present
    if let Ok(exec) = load_execution(id) {
        let report = report_path(&exec.id, &exec.question);
        if report.exists() {
            fs::remove_file(&report).ok();
        }
    }
    Ok(())
}

pub fn save_pipeline_yaml(name: &str, question: &str, model: &str, provider: &str) -> Result<PathBuf> {
    let dir = ScaffConfig::config_dir().join("pipelines");
    fs::create_dir_all(&dir).ok();
    let path = dir.join(format!("{name}.yaml"));
    let body = format!(
        "# scaff pipeline: {name}\n# created: {}\n\nname: {name}\ndescription: {question}\npipeline: research\nprovider: {provider}\nmodel: {model}\nvariables:\n  - name: question\n    default: \"{question}\"\nstages:\n  - plan\n  - search\n  - synthesize\n  - render\nsources:\n  - corpus\n  - web\n",
        Utc::now().to_rfc3339(),
        name = name,
        question = question.replace('"', "\\\""),
        provider = provider,
        model = model,
    );
    crate::config::write_atomic(&path, &body).map_err(|e| anyhow::anyhow!("write pipeline file: {e}"))?;
    Ok(path)
}

pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = true;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !last_dash {
                out.push('-');
                last_dash = true;
            }
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > 60 {
        out.truncate(60);
        while out.ends_with('-') {
            out.pop();
        }
    }
    if out.is_empty() {
        "report".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("What is Harness CD?"), "what-is-harness-cd");
        assert_eq!(slugify("  hello   world!! "), "hello-world");
        assert_eq!(slugify(""), "report");
    }

    #[test]
    fn slugify_truncates() {
        let long = "a".repeat(200);
        let s = slugify(&long);
        assert!(s.len() <= 60);
    }
}
