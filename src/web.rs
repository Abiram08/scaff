//! Web search and page fetch tools for the agent.

use anyhow::{anyhow, Result};
use scraper::{Html, Selector};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct WebResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub fn search(query: &str, max_results: usize) -> Result<Vec<WebResult>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("https://html.duckduckgo.com/html/?q={}", url_encode(query));
    let resp = ureq::get(&url)
        .set("User-Agent", "scaff-research/1.0 (+https://harness.io)")
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| anyhow!("web search request failed: {e}"))?;
    let html = resp.into_string()?;
    parse_duckduckgo(&html, max_results)
}

pub fn fetch_excerpts(url: &str, max_chars: usize) -> Result<String> {
    let resp = ureq::get(url)
        .set("User-Agent", "scaff-research/1.0 (+https://harness.io)")
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| anyhow!("fetch {url}: {e}"))?;
    let html = resp.into_string()?;
    let doc = Html::parse_document(&html);
    let body_sel = Selector::parse("body").map_err(|e| anyhow!("{e:?}"))?;
    let mut buf = String::new();
    if let Some(body) = doc.select(&body_sel).next() {
        for t in body.text() {
            let t = t.trim();
            if !t.is_empty() {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(t);
            }
            if buf.len() > max_chars {
                break;
            }
        }
    }
    if buf.len() > max_chars {
        buf.truncate(max_chars);
        buf.push_str("...");
    }
    Ok(buf)
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn parse_duckduckgo(html: &str, max_results: usize) -> Result<Vec<WebResult>> {
    let doc = Html::parse_document(html);
    let result_sel = Selector::parse(".result").map_err(|e| anyhow!("{e:?}"))?;
    let title_sel = Selector::parse(".result__a").map_err(|e| anyhow!("{e:?}"))?;
    let snippet_sel = Selector::parse(".result__snippet").map_err(|e| anyhow!("{e:?}"))?;
    let url_sel = Selector::parse(".result__url").map_err(|e| anyhow!("{e:?}"))?;

    let mut out = Vec::new();
    for r in doc.select(&result_sel) {
        if out.len() >= max_results {
            break;
        }
        let title = r
            .select(&title_sel)
            .next()
            .map(|n| n.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let href = r
            .select(&title_sel)
            .next()
            .and_then(|n| n.value().attr("href"))
            .map(|s| s.to_string())
            .unwrap_or_default();
        let snippet = r
            .select(&snippet_sel)
            .next()
            .map(|n| n.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let fallback_url = r
            .select(&url_sel)
            .next()
            .map(|n| n.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let url = if href.is_empty() { fallback_url } else { href };

        if title.is_empty() || url.is_empty() {
            continue;
        }

        out.push(WebResult {
            title,
            url,
            snippet,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encode_spaces_and_specials() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn search_empty_query() {
        let r = search("  ", 5).unwrap();
        assert!(r.is_empty());
    }
}
