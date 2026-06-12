use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub url: String,
    pub title: String,
    pub section: Option<String>,
    pub content: String,
}

pub struct Index {
    chunks: Vec<Chunk>,
    doc_len: Vec<u32>,
    avg_doc_len: f64,
    inverted: HashMap<String, Vec<(usize, u32)>>,
    df: HashMap<String, u32>,
    total_docs: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub chunk: Chunk,
    pub score: f64,
    pub matched_terms: Vec<String>,
}

const K1: f64 = 1.5;
const B: f64 = 0.75;

pub fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(text.len() / 5);
    let mut buf = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            for low in ch.to_lowercase() {
                buf.push(low);
            }
        } else if !buf.is_empty() {
            if buf.len() >= 2 {
                out.push(std::mem::take(&mut buf));
            } else {
                buf.clear();
            }
        }
    }
    if buf.len() >= 2 {
        out.push(buf);
    }
    out
}

impl Index {
    pub fn build(chunks: Vec<Chunk>) -> Self {
        let total = chunks.len();
        let mut doc_len: Vec<u32> = Vec::with_capacity(total);
        let mut df: HashMap<String, u32> = HashMap::new();
        let mut inverted: HashMap<String, Vec<(usize, u32)>> = HashMap::new();

        for (i, chunk) in chunks.iter().enumerate() {
            let haystack = format!(
                "{} {} {} {}",
                chunk.title,
                chunk.section.clone().unwrap_or_default(),
                chunk.content,
                chunk.url
            );
            let tokens = tokenize(&haystack);
            let mut tf: HashMap<String, u32> = HashMap::new();
            for t in tokens {
                *tf.entry(t).or_insert(0) += 1;
            }
            let len = tf.values().sum::<u32>().max(1);
            doc_len.push(len);
            for (term, freq) in &tf {
                *df.entry(term.clone()).or_insert(0) += 1;
                inverted.entry(term.clone()).or_default().push((i, *freq));
            }
        }

        let avg = if total == 0 {
            1.0
        } else {
            doc_len.iter().sum::<u32>() as f64 / total as f64
        };

        Self {
            chunks,
            doc_len,
            avg_doc_len: avg,
            inverted,
            df,
            total_docs: total as u32,
        }
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn search(&self, query: &str, k: usize) -> Vec<SearchHit> {
        if self.chunks.is_empty() {
            return Vec::new();
        }
        let q_terms = tokenize(query);
        if q_terms.is_empty() {
            return Vec::new();
        }

        // Aggregate scores per doc
        let mut scores: HashMap<usize, (f64, Vec<String>)> = HashMap::new();
        for term in &q_terms {
            let df = match self.df.get(term) {
                Some(d) => *d,
                None => continue,
            };
            let idf = (((self.total_docs as f64) - df as f64 + 0.5)
                / (df as f64 + 0.5)
                + 1.0)
                .ln();
            if let Some(postings) = self.inverted.get(term) {
                for (doc_idx, tf) in postings {
                    let dl = self.doc_len[*doc_idx] as f64;
                    let tf_norm = (*tf as f64 * (K1 + 1.0))
                        / (*tf as f64 + K1 * (1.0 - B + B * dl / self.avg_doc_len));
                    let contrib = idf * tf_norm;
                    let entry = scores
                        .entry(*doc_idx)
                        .or_insert((0.0, Vec::new()));
                    entry.0 += contrib;
                    if !entry.1.contains(term) {
                        entry.1.push(term.clone());
                    }
                }
            }
        }

        let mut hits: Vec<SearchHit> = scores
            .into_iter()
            .map(|(idx, (score, matched))| SearchHit {
                chunk: self.chunks[idx].clone(),
                score,
                matched_terms: matched,
            })
            .collect();
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(k);
        hits
    }

    /// Return up to `k` chunks, optionally restricted to those whose url starts with `url_prefix`.
    pub fn search_in(&self, query: &str, k: usize, url_prefix: Option<&str>) -> Vec<SearchHit> {
        let mut hits = self.search(query, k.max(1) * 4);
        if let Some(prefix) = url_prefix {
            hits.retain(|h| h.chunk.url.starts_with(prefix));
        }
        hits.truncate(k);
        hits
    }

    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(id: &str, title: &str, content: &str) -> Chunk {
        Chunk {
            id: id.to_string(),
            url: format!("https://example.com/{id}"),
            title: title.to_string(),
            section: None,
            content: content.to_string(),
        }
    }

    #[test]
    fn tokenize_basic() {
        let t = tokenize("Hello, World! The Harness platform.");
        assert_eq!(t, vec!["hello", "world", "the", "harness", "platform"]);
    }

    #[test]
    fn tokenize_skips_short_tokens() {
        let t = tokenize("a an the harness");
        assert_eq!(t, vec!["an", "the", "harness"]);
    }

    #[test]
    fn search_finds_relevant_chunk() {
        let chunks = vec![
            make_chunk("a", "Pipelines overview", "Harness pipelines run build and test stages."),
            make_chunk("b", "Cloud cost", "Cloud cost management tracks spend across AWS and GCP."),
            make_chunk("c", "Canary deploys", "Canary deployments use a percentage-based rollout strategy."),
        ];
        let idx = Index::build(chunks);
        let hits = idx.search("canary deployment", 2);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].chunk.id, "c");
    }

    #[test]
    fn search_empty_index() {
        let idx = Index::build(vec![]);
        assert!(idx.search("anything", 5).is_empty());
    }

    #[test]
    fn search_no_match() {
        let chunks = vec![make_chunk("a", "x", "alpha beta gamma")];
        let idx = Index::build(chunks);
        assert!(idx.search("nonexistentwordzzz", 5).is_empty());
    }

    #[test]
    fn search_in_with_url_filter() {
        let chunks = vec![
            make_chunk("a", "Pipelines", "harness pipeline canary"),
            make_chunk("b", "Cost", "harness cost spend"),
        ];
        let idx = Index::build(chunks);
        let hits = idx.search_in("harness", 5, Some("https://example.com/a"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk.id, "a");
    }
}
