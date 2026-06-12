use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::Digest;

use crate::corpus::RawChunk;
use crate::search::Chunk;

const EXTS: &[&str] = &["md", "markdown", "txt", "rst", "adoc", "text"];

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn ingest_file(path: &Path) -> Result<Vec<RawChunk>> {
    let body = fs::read_to_string(path).with_context(|| format!("read {path:?}"))?;
    let title = first_heading(&body).unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string()
    });
    let mut out = Vec::new();
    let url = path_to_url(path);
    for (i, section_content) in chunk_markdown(&body, 2000).into_iter().enumerate() {
        let id_input = format!("{}#{}", path.display(), i);
        let id = format!("{:x}", sha2::Sha256::digest(id_input.as_bytes()));
        out.push(RawChunk {
            id,
            url: url.clone(),
            title: title.clone(),
            section: Some(format!("section {i}")),
            content: section_content,
            source: "local".to_string(),
        });
    }
    if out.is_empty() {
        // file was too small to chunk; emit one chunk with the body
        let id_input = format!("{}#full", path.display());
        let id = format!("{:x}", sha2::Sha256::digest(id_input.as_bytes()));
        out.push(RawChunk {
            id,
            url,
            title,
            section: None,
            content: body,
            source: "local".to_string(),
        });
    }
    Ok(out)
}

pub fn ingest_directory(dir: &Path) -> Result<Vec<Chunk>> {
    let mut all_raw = Vec::new();
    walk(dir, &mut |p| {
        if is_supported(p) {
            if let Ok(chunks) = ingest_file(p) {
                all_raw.extend(chunks);
            }
        }
    })?;
    Ok(all_raw
        .into_iter()
        .map(|r| Chunk {
            id: r.id,
            url: r.url,
            title: r.title,
            section: r.section,
            content: r.content,
        })
        .collect())
}

pub fn add_directory_to_corpus(dir: &Path) -> Result<usize> {
    let conn = crate::corpus::open_db()?;
    let mut total = 0;
    let mut raw_batches: Vec<RawChunk> = Vec::new();
    walk(dir, &mut |p| {
        if is_supported(p) {
            if let Ok(chunks) = ingest_file(p) {
                total += chunks.len();
                raw_batches.extend(chunks);
            }
        }
    })?;
    crate::corpus::ingest_raw(&conn, &raw_batches)?;
    Ok(total)
}

fn walk<F: FnMut(&Path)>(dir: &Path, f: &mut F) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let read = fs::read_dir(dir).with_context(|| format!("read_dir {dir:?}"))?;
    for entry in read {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, f)?;
        } else if path.is_file() {
            f(&path);
        }
    }
    Ok(())
}

fn path_to_url(path: &Path) -> String {
    // Use a custom scheme so it's a valid URL but obviously not web.
    let abs = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    format!("local://{}", abs.display().to_string().replace('\\', "/"))
}

fn first_heading(body: &str) -> Option<String> {
    for line in body.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
        if t.starts_with("===") || t.starts_with("---") {
            continue;
        }
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    None
}

fn chunk_markdown(body: &str, max_chars: usize) -> Vec<String> {
    // Split on ATX headings; each chunk is the section between two headings.
    let mut sections: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in body.lines() {
        if line.trim_start().starts_with("# ") || line.trim_start().starts_with("## ") {
            if !current.trim().is_empty() {
                sections.push(std::mem::take(&mut current));
            }
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.trim().is_empty() {
        sections.push(current);
    }

    // Further split sections that exceed max_chars on sentence boundaries.
    let mut out = Vec::new();
    for s in sections {
        if s.len() <= max_chars {
            out.push(s);
        } else {
            let mut start = 0;
            while start < s.len() {
                let end = (start + max_chars).min(s.len());
                let mut cut = end;
                if end < s.len() {
                    if let Some(pos) = s[start..end].rfind(". ") {
                        cut = start + pos + 1;
                    }
                }
                out.push(s[start..cut].to_string());
                start = cut;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_markdown_splits_on_heading() {
        let body = "# Title\n\nA paragraph.\n\n## Section 2\n\nAnother.\n";
        let parts = chunk_markdown(body, 10_000);
        assert!(parts.len() >= 2);
    }

    #[test]
    fn is_supported_known_extensions() {
        assert!(is_supported(Path::new("a.md")));
        assert!(is_supported(Path::new("b.MARKDOWN")));
        assert!(!is_supported(Path::new("c.py")));
        assert!(!is_supported(Path::new("d")));
    }
}
