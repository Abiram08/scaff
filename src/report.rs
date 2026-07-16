//! Stable Markdown report from a `finish` payload (AGENTS.md §6.5).

use crate::agent::types::{Confidence, FinishPayload};

/// Render the user-facing research report.
pub fn render(payload: &FinishPayload) -> String {
    let mut out = String::with_capacity(2048);

    out.push_str("## TL;DR\n\n");
    let tldr = payload.tldr.trim();
    if tldr.is_empty() {
        out.push_str("_No summary provided._\n\n");
    } else {
        out.push_str(tldr);
        out.push_str("\n\n");
    }

    out.push_str("## Confidence Summary\n\n");
    let (h, m, l, c) = count_confidence(payload);
    out.push_str(&format!(
        "✅ **{h}** high · ⚠️ **{m}** medium · 🔴 **{l}** low · ⚡ **{c}** contested\n\n"
    ));

    out.push_str("## Findings\n\n");
    if payload.findings.is_empty() {
        out.push_str("_No structured findings._\n\n");
    } else {
        for (i, f) in payload.findings.iter().enumerate() {
            let badge = format!("{} {}", f.confidence.emoji(), f.confidence.as_str());
            out.push_str(&format!("{}. **[{badge}]** {}\n", i + 1, f.text.trim()));
            if !f.source_ids.is_empty() {
                out.push_str(&format!("   - sources: {}\n", f.source_ids.join(", ")));
            }
            if let Some(note) = &f.conflict_note {
                if !note.trim().is_empty() {
                    out.push_str(&format!("   - conflict: {}\n", note.trim()));
                }
            }
            out.push('\n');
        }
    }

    out.push_str("## Known Unknowns\n\n");
    if payload.known_unknowns.is_empty() {
        out.push_str("None\n\n");
    } else {
        for u in &payload.known_unknowns {
            out.push_str(&format!("- {}\n", u.trim()));
        }
        out.push('\n');
    }

    out.push_str("## Sources\n\n");
    if payload.sources.is_empty() {
        // Fall back to listing ids mentioned in findings.
        let mut ids: Vec<String> = payload
            .findings
            .iter()
            .flat_map(|f| f.source_ids.clone())
            .collect();
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            out.push_str("_No sources listed._\n");
        } else {
            for id in ids {
                out.push_str(&format!("- `{id}`\n"));
            }
        }
    } else {
        for s in &payload.sources {
            let kind = if s.kind.is_empty() { "source" } else { &s.kind };
            out.push_str(&format!(
                "- **[{}]** {} — {} _({kind})_\n",
                s.id,
                if s.title.is_empty() { "(untitled)" } else { &s.title },
                if s.url.is_empty() { "—" } else { &s.url }
            ));
        }
    }

    out.push('\n');
    out
}

fn count_confidence(payload: &FinishPayload) -> (usize, usize, usize, usize) {
    let mut h = 0;
    let mut m = 0;
    let mut l = 0;
    let mut c = 0;
    for f in &payload.findings {
        match f.confidence {
            Confidence::High => h += 1,
            Confidence::Medium => m += 1,
            Confidence::Low => l += 1,
            Confidence::Contested => c += 1,
        }
    }
    (h, m, l, c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{Finding, SourceRef};

    #[test]
    fn render_includes_sections() {
        let p = FinishPayload {
            tldr: "Harness CD supports canary.".into(),
            findings: vec![Finding {
                text: "Canary deployments are first-class.".into(),
                confidence: Confidence::High,
                source_ids: vec!["corpus:cd-1".into()],
                conflict_note: None,
            }],
            known_unknowns: vec!["Rollback defaults vary by version.".into()],
            sources: vec![SourceRef {
                id: "corpus:cd-1".into(),
                title: "CD docs".into(),
                url: "https://developer.harness.io/docs/cd".into(),
                kind: "corpus".into(),
            }],
        };
        let md = render(&p);
        assert!(md.contains("## TL;DR"));
        assert!(md.contains("## Confidence Summary"));
        assert!(md.contains("## Findings"));
        assert!(md.contains("## Known Unknowns"));
        assert!(md.contains("## Sources"));
        assert!(md.contains("corpus:cd-1"));
        assert!(md.contains("canary") || md.contains("Canary"));
    }
}
