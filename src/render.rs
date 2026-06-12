use std::collections::HashMap;

use crate::pipeline::{Claim, Confidence};
use crate::search::SearchHit;
use crate::web::WebResult;

#[derive(Debug, Clone)]
pub struct Source {
    pub id: usize,
    pub title: String,
    pub url: String,
    pub source_kind: SourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Corpus,
    Web,
}

fn emoji_for_confidence(c: &Confidence) -> &'static str {
    match c {
        Confidence::High => "✅",
        Confidence::Medium => "⚠️",
        Confidence::Low => "🔴",
        Confidence::Contested => "⚡",
    }
}

fn label_for_confidence(c: &Confidence) -> &'static str {
    match c {
        Confidence::High => "High confidence",
        Confidence::Medium => "Medium confidence",
        Confidence::Low => "Low confidence",
        Confidence::Contested => "Contested",
    }
}

pub fn render_report(
    question: &str,
    tldr: &str,
    body: &str,
    corpus_hits: &[SearchHit],
    web_results: &[WebResult],
    claims: &[Claim],
) -> String {
    let mut sources: Vec<Source> = Vec::new();
    let mut dedupe: HashMap<String, usize> = HashMap::new();

    for h in corpus_hits {
        if let Some(&existing) = dedupe.get(&h.chunk.url) {
            let _ = existing;
            continue;
        }
        let id = sources.len() + 1;
        dedupe.insert(h.chunk.url.clone(), id);
        sources.push(Source {
            id,
            title: h.chunk.title.clone(),
            url: h.chunk.url.clone(),
            source_kind: SourceKind::Corpus,
        });
    }
    for w in web_results {
        if dedupe.contains_key(&w.url) {
            continue;
        }
        let id = sources.len() + 1;
        dedupe.insert(w.url.clone(), id);
        sources.push(Source {
            id,
            title: w.title.clone(),
            url: w.url.clone(),
            source_kind: SourceKind::Web,
        });
    }

    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", question.trim()));

    // TL;DR
    if !tldr.trim().is_empty() {
        out.push_str("## TL;DR\n\n");
        out.push_str(tldr.trim());
        out.push_str("\n\n");
    }

    // Confidence summary
    if !claims.is_empty() {
        out.push_str("## Confidence Summary\n\n");
        let high = claims.iter().filter(|c| c.confidence == Confidence::High).count();
        let medium = claims.iter().filter(|c| c.confidence == Confidence::Medium).count();
        let low = claims.iter().filter(|c| c.confidence == Confidence::Low).count();
        let contested = claims.iter().filter(|c| c.confidence == Confidence::Contested).count();

        if high > 0 {
            out.push_str(&format!("| {} {} | {} claims from 2+ sources |\n", emoji_for_confidence(&Confidence::High), label_for_confidence(&Confidence::High), high));
        }
        if medium > 0 {
            out.push_str(&format!("| {} {} | {} claims from single source |\n", emoji_for_confidence(&Confidence::Medium), label_for_confidence(&Confidence::Medium), medium));
        }
        if low > 0 {
            out.push_str(&format!("| {} {} | {} limited-corroboration claims |\n", emoji_for_confidence(&Confidence::Low), label_for_confidence(&Confidence::Low), low));
        }
        if contested > 0 {
            out.push_str(&format!("| {} {} | {} conflicting-source claims |\n", emoji_for_confidence(&Confidence::Contested), label_for_confidence(&Confidence::Contested), contested));
        }
        out.push('\n');
    }

    // Findings
    out.push_str("## Findings\n\n");
    out.push_str(body.trim());
    out.push_str("\n\n");

    // Inline claim annotations
    if !claims.is_empty() {
        out.push_str("## Key Claims\n\n");
        for claim in claims {
            let emoji = emoji_for_confidence(&claim.confidence);
            let label = label_for_confidence(&claim.confidence);
            let sources_str = if claim.source_ids.is_empty() {
                String::new()
            } else {
                let ids: Vec<String> = claim.source_ids.iter().map(|id| format!("[{}]", id + 1)).collect();
                format!(" — source(s): {}", ids.join(", "))
            };
            let note = claim.conflict_note.as_deref().unwrap_or("");
            out.push_str(&format!(
                "- {} **{}**{}",
                emoji, claim.text, sources_str
            ));
            if !note.is_empty() {
                out.push_str(&format!("\n  - ⚡ Conflict: {}", note));
            }
            out.push_str(&format!("\n  - _{}_\n\n", label));
        }
    }

    // Sources
    out.push_str("## Sources\n\n");
    if sources.is_empty() {
        out.push_str("_No sources were retrieved for this question._\n");
    } else {
        for s in &sources {
            let kind = match s.source_kind {
                SourceKind::Corpus => "corpus",
                SourceKind::Web => "web",
            };
            out.push_str(&format!(
                "[{}] **{}** — <{}> _(via {})_\n",
                s.id, s.title, s.url, kind
            ));
        }
    }
    out.push('\n');
    out
}

pub fn format_citation_indices(hits: &[SearchHit]) -> String {
    let mut ids = Vec::new();
    for (i, _) in hits.iter().enumerate() {
        ids.push(format!("[{}]", i + 1));
    }
    ids.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Chunk;

    fn hit(url: &str, title: &str, content: &str) -> SearchHit {
        SearchHit {
            chunk: Chunk {
                id: "x".into(),
                url: url.into(),
                title: title.into(),
                section: None,
                content: content.into(),
            },
            score: 1.0,
            matched_terms: vec![],
        }
    }

    #[test]
    fn render_report_includes_sections() {
        let body = "Canary deploys split traffic [1]. See docs [2].";
        let hits = vec![
            hit("https://docs/a", "A", "x"),
            hit("https://docs/b", "B", "y"),
        ];
        let r = render_report("What is canary?", "Short answer.", body, &hits, &[], &[]);
        assert!(r.contains("# What is canary?"));
        assert!(r.contains("TL;DR"));
        assert!(r.contains("Short answer."));
        assert!(r.contains("[1]"));
        assert!(r.contains("Sources"));
    }

    #[test]
    fn render_report_empty_sources_says_so() {
        let r = render_report("q", "t", "body", &[], &[], &[]);
        assert!(r.contains("No sources were retrieved"));
    }

    #[test]
    fn render_report_shows_confidence_summary() {
        use crate::pipeline::Confidence;
        let claims = vec![
            crate::pipeline::Claim { id: 0, text: "Harness supports canary".into(), source_ids: vec![0, 1], confidence: Confidence::High, conflict_note: None },
            crate::pipeline::Claim { id: 1, text: "Rollback is automatic".into(), source_ids: vec![2], confidence: Confidence::Medium, conflict_note: None },
        ];
        let r = render_report("q", "t", "body", &[], &[], &claims);
        assert!(r.contains("Confidence Summary"));
        assert!(r.contains("High confidence"));
        assert!(r.contains("Medium confidence"));
        assert!(r.contains("Key Claims"));
    }
}
