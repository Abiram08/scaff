use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection};
use scraper::{element_ref::ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::config::ScaffConfig;
use crate::search::{Chunk, Index};

pub const DEFAULT_SEED_URL: &str =
    "https://raw.githubusercontent.com/harness-research/scaff-corpus/main/seed.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawChunk {
    pub id: String,
    pub url: String,
    pub title: String,
    pub section: Option<String>,
    pub content: String,
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "seed".to_string()
}

pub fn data_dir() -> PathBuf {
    ScaffConfig::config_dir().join("corpus")
}

pub fn db_path() -> PathBuf {
    ScaffConfig::corpus_db_path()
}

pub fn seed_path() -> PathBuf {
    data_dir().join("seed.jsonl")
}

pub fn open_db() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(&path).with_context(|| format!("opening {path:?}"))?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS chunks (
            id        TEXT PRIMARY KEY,
            url       TEXT NOT NULL,
            title     TEXT NOT NULL,
            section   TEXT,
            content   TEXT NOT NULL,
            source    TEXT NOT NULL DEFAULT 'seed',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_chunks_url    ON chunks(url);
        CREATE INDEX IF NOT EXISTS idx_chunks_source ON chunks(source);
        "#,
    )?;
    Ok(conn)
}

pub fn load_seed_file(path: &Path) -> Result<Vec<RawChunk>> {
    let f = fs::File::open(path).with_context(|| format!("opening {path:?}"))?;
    let reader = BufReader::new(f);
    let mut out = Vec::new();
    for (lineno, line) in reader.lines().enumerate() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let chunk: RawChunk = serde_json::from_str(trimmed)
            .with_context(|| format!("parsing line {} of {path:?}", lineno + 1))?;
        out.push(chunk);
    }
    Ok(out)
}

pub fn ingest_raw(conn: &Connection, raw: &[RawChunk]) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut count = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO chunks (id, url, title, section, content, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for r in raw {
            stmt.execute(params![
                r.id,
                r.url,
                r.title,
                r.section,
                r.content,
                r.source,
            ])?;
            count += 1;
        }
    }
    tx.commit()?;
    Ok(count)
}

pub fn ingest_seed_path(conn: &Connection, path: &Path) -> Result<usize> {
    let raw = load_seed_file(path)?;
    ingest_raw(conn, &raw)
}

pub fn total_chunks(conn: &Connection) -> Result<u64> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
    Ok(n as u64)
}

pub fn load_all_chunks(conn: &Connection) -> Result<Vec<Chunk>> {
    let mut stmt = conn.prepare("SELECT id, url, title, section, content FROM chunks")?;
    let rows = stmt.query_map([], |r| {
        Ok(Chunk {
            id: r.get(0)?,
            url: r.get(1)?,
            title: r.get(2)?,
            section: r.get(3)?,
            content: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn build_index(conn: &Connection) -> Result<Index> {
    let chunks = load_all_chunks(conn)?;
    Ok(Index::build(chunks))
}

/// First-run bootstrap: copies the bundled seed into ~/.scaff/corpus/seed.jsonl
/// and ingests it into the SQLite db if neither exists.
pub fn ensure_seeded() -> Result<usize> {
    let dir = data_dir();
    fs::create_dir_all(&dir).ok();
    let dest = seed_path();
    if !dest.exists() {
        if let Some(bundled) = bundled_seed_path() {
            if bundled.exists() {
                fs::copy(&bundled, &dest)
                    .with_context(|| format!("copying bundled seed from {bundled:?}"))?;
            }
        }
    }
    let conn = open_db()?;
    let n = total_chunks(&conn)?;
    if n == 0 && dest.exists() {
        return ingest_seed_path(&conn, &dest);
    }
    Ok(n as usize)
}

pub fn bundled_seed_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SCAFF_CORPUS_DIR") {
        let candidate = PathBuf::from(p).join("seed.jsonl");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    // 1) Beside the executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for rel in ["corpus/seed.jsonl", "../corpus/seed.jsonl", "../../corpus/seed.jsonl"] {
                let cand = dir.join(rel);
                if cand.exists() {
                    return Some(cand);
                }
            }
        }
    }
    // 2) CWD
    let cwd = PathBuf::from("corpus/seed.jsonl");
    if cwd.exists() {
        return Some(cwd);
    }
    None
}

pub fn download_seed(url: &str, dest: &Path) -> Result<usize> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).ok();
    }
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(60))
        .call()
        .map_err(|e| anyhow!("downloading {url}: {e}"))?;
    let mut reader = resp.into_reader();
    let mut f = fs::File::create(dest).with_context(|| format!("creating {dest:?}"))?;
    std::io::copy(&mut reader, &mut f)?;
    f.flush()?;
    drop(f);
    let conn = open_db()?;
    ingest_seed_path(&conn, dest)
}

/// Fetch a single URL, extract readable text, chunk by headings, return RawChunks.
pub fn crawl_url(url: &str, source: &str) -> Result<Vec<RawChunk>> {
    let resp = ureq::get(url)
        .set("User-Agent", "scaff-research/1.0 (+https://harness.io)")
        .timeout(std::time::Duration::from_secs(60))
        .call()
        .map_err(|e| anyhow!("GET {url}: {e}"))?;
    let html = resp.into_string()?;
    parse_html(&html, url, source)
}

pub fn parse_html(html: &str, url: &str, source: &str) -> Result<Vec<RawChunk>> {
    let doc = Html::parse_document(html);
    let title_sel = Selector::parse("title").unwrap();
    let title = doc
        .select(&title_sel)
        .next()
        .map(|n| n.text().collect::<String>().trim().to_string())
        .unwrap_or_else(|| url.to_string());

    let main_sel = Selector::parse("main, article, [role=main], body").unwrap();
    let root = doc.select(&main_sel).next();

    // Collect heading + text pairs by walking the DOM in document order.
    let mut out: Vec<RawChunk> = Vec::new();
    let mut current_section: Option<String> = None;
    let mut current_buf = String::new();

    let flush = |section: &mut Option<String>, buf: &mut String, out: &mut Vec<RawChunk>, url: &str, title: &str, source: &str| {
        let text = buf.trim();
        if text.is_empty() {
            return;
        }
        for slice in chunk_text(text, 2000) {
            let id_source = format!("{}#{}", url, section.clone().unwrap_or_else(|| "body".to_string()));
            let id = format!("{:x}", sha2::Sha256::digest(id_source.as_bytes()));
            out.push(RawChunk {
                id,
                url: url.to_string(),
                title: title.to_string(),
                section: section.clone(),
                content: slice,
                source: source.to_string(),
            });
        }
        buf.clear();
    };

    if let Some(root) = root {
        for node in root.descendants() {
            if let Some(elem) = node.value().as_element() {
                let tag = elem.name();
                if matches!(tag, "h1" | "h2" | "h3" | "h4") {
                    flush(&mut current_section, &mut current_buf, &mut out, url, &title, source);
                    let elem_ref = ElementRef::wrap(node).unwrap();
                    let text: String = elem_ref.text().collect::<String>().trim().to_string();
                    if !text.is_empty() {
                        current_section = Some(text);
                    }
                }
            } else if let Some(text) = node.value().as_text() {
                let t = text.trim();
                if !t.is_empty() {
                    if !current_buf.is_empty() {
                        current_buf.push(' ');
                    }
                    current_buf.push_str(t);
                }
            }
        }
    }
    flush(&mut current_section, &mut current_buf, &mut out, url, &title, source);
    Ok(out)
}

fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = (start + max_chars).min(text.len());
        // Try to break on a sentence boundary
        let mut cut = end;
        if end < text.len() {
            if let Some(pos) = text[start..end].rfind(". ") {
                cut = start + pos + 1;
            }
        }
        out.push(text[start..cut].trim().to_string());
        start = cut;
    }
    out
}

pub fn stats(conn: &Connection) -> Result<CorpusStats> {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
    let sources: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT source) FROM chunks",
        [],
        |r| r.get(0),
    )?;
    let urls: i64 = conn.query_row("SELECT COUNT(DISTINCT url) FROM chunks", [], |r| r.get(0))?;
    let size: i64 = conn.query_row(
        "SELECT COALESCE(SUM(LENGTH(content)), 0) FROM chunks",
        [],
        |r| r.get(0),
    )?;
    Ok(CorpusStats {
        total: total as u64,
        unique_urls: urls as u64,
        unique_sources: sources as u64,
        total_bytes: size as u64,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct CorpusStats {
    pub total: u64,
    pub unique_urls: u64,
    pub unique_sources: u64,
    pub total_bytes: u64,
}

pub fn clear(conn: &Connection) -> Result<usize> {
    let n = conn.execute("DELETE FROM chunks", [])?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_html_basic() {
        let html = r#"
            <html><head><title>Harness pipelines</title></head><body>
              <main>
                <h1>Overview</h1>
                <p>Pipelines run build and test stages sequentially or in parallel.</p>
                <h2>Build stage</h2>
                <p>The build stage compiles source code and produces artifacts.</p>
              </main>
            </body></html>
        "#;
        let chunks = parse_html(html, "https://example.com/pipelines", "crawl").unwrap();
        assert!(chunks.len() >= 2);
        assert!(chunks[0].title.contains("Harness pipelines"));
        assert!(chunks.iter().any(|c| c.content.contains("build and test")));
    }

    #[test]
    fn chunk_text_short_unchanged() {
        let s = "short text";
        assert_eq!(chunk_text(s, 100), vec!["short text".to_string()]);
    }

    #[test]
    fn chunk_text_long_splits() {
        let s = "a. ".repeat(500);
        let parts = chunk_text(&s, 200);
        assert!(parts.len() > 1);
        for p in &parts {
            assert!(p.len() <= 250);
        }
    }

    #[test]
    fn stats_reports_counts() {
        let conn = open_db().unwrap();
        clear(&conn).ok();
        let raw = vec![RawChunk {
            id: "x".into(),
            url: "https://e.test/".into(),
            title: "t".into(),
            section: None,
            content: "hello world".into(),
            source: "test".into(),
        }];
        ingest_raw(&conn, &raw).unwrap();
        let s = stats(&conn).unwrap();
        assert_eq!(s.total, 1);
        assert_eq!(s.unique_urls, 1);
    }
}
