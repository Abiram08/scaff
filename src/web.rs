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
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        url_encode(query)
    );
    let resp = ureq::get(&url)
        .set("User-Agent", "scaff-research/1.0 (+https://harness.io)")
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| anyhow!("web search request failed: {e}"))?;
    let html = resp.into_string()?;
    parse_duckduckgo(&html, max_results)
}

pub fn parallel_search(queries: &[String], max_per_query: usize) -> Vec<WebResult> {
    let mut all: Vec<WebResult> = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (i, query) in queries.iter().enumerate() {
        // Rate limiting: 200ms delay between sequential requests to avoid 429s.
        if i > 0 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if let Ok(results) = search(query, max_per_query) {
            for r in results {
                if seen_urls.insert(r.url.clone()) {
                    all.push(r);
                }
            }
        }
    }

    all.sort_by(|a, b| b.snippet.len().cmp(&a.snippet.len()));
    all.truncate(max_per_query * queries.len().min(3));
    all
}

pub fn generate_search_queries(question: &str, sub_questions: &[String], max_queries: usize) -> Vec<String> {
    let mut queries: Vec<String> = Vec::new();
    queries.push(question.to_string());

    for sq in sub_questions.iter().take(max_queries.saturating_sub(1)) {
        if !queries.contains(sq) {
            queries.push(sq.clone());
        }
    }

    let key_terms = extract_key_terms(question);
    for term in key_terms.iter().take(2) {
        let q = format!("{term} explanation");
        if !queries.contains(&q) {
            queries.push(q);
        }
    }

    queries.truncate(max_queries);
    queries
}

fn extract_key_terms(question: &str) -> Vec<String> {
    let stop_words: std::collections::HashSet<&str> = [
        "what", "how", "why", "when", "where", "which", "who", "is", "are", "was", "were",
        "do", "does", "did", "can", "could", "should", "would", "will", "shall", "may",
        "might", "the", "a", "an", "in", "on", "at", "to", "for", "of", "with", "by",
        "from", "as", "into", "through", "during", "before", "after", "above", "below",
        "between", "and", "or", "not", "no", "but", "if", "then", "than", "that", "this",
        "these", "those", "it", "its", "i", "me", "my", "we", "our", "you", "your", "he",
        "him", "his", "she", "her", "they", "them", "their", "about", "like", "just",
    ]
    .iter()
    .cloned()
    .collect();

    let words: Vec<&str> = question.split_whitespace().collect();
    let mut terms: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();

    for word in words {
        let clean: String = word.chars().filter(|c| c.is_alphanumeric() || *c == '-').collect();
        let lower = clean.to_lowercase();
        if stop_words.contains(lower.as_str()) || lower.len() < 3 {
            if !current.is_empty() {
                terms.push(current.join(" "));
                current.clear();
            }
        } else {
            current.push(word);
        }
    }
    if !current.is_empty() {
        terms.push(current.join(" "));
    }

    terms.sort_by(|a, b| b.len().cmp(&a.len()));
    terms.dedup();
    terms
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

        out.push(WebResult { title, url, snippet });
    }
    Ok(out)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encode_spaces_and_specials() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn extract_key_terms_basic() {
        let terms = extract_key_terms("What is Harness Continuous Delivery?");
        assert!(!terms.is_empty());
        assert!(terms.iter().any(|t| t.to_lowercase().contains("harness")));
    }

    #[test]
    fn generate_search_queries_returns_varied() {
        let sub_q = vec![
            "How does Harness CD work?".to_string(),
            "What are canary deployments?".to_string(),
        ];
        let queries = generate_search_queries("What is Harness CD?", &sub_q, 5);
        assert!(queries.len() >= 3);
        assert!(queries.contains(&"What is Harness CD?".to_string()));
    }

    #[test]
    fn parallel_search_deduplicates() {
        let queries = vec![
            "test query one".to_string(),
            "test query two".to_string(),
        ];
        let results = parallel_search(&queries, 3);
        let urls: Vec<&str> = results.iter().map(|r| r.url.as_str()).collect();
        let mut deduped = urls.clone();
        deduped.dedup();
        assert_eq!(urls.len(), deduped.len(), "parallel_search should deduplicate URLs");
    }
}
