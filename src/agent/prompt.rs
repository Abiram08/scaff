//! Short system prompts — Pi-style: tiny core, tools carry the ACI.

use super::types::UseCase;

/// Core system prompt (keep short; frontier models already know how to use tools).
pub fn core_system_prompt() -> &'static str {
    r#"You are scaff, a Harness research agent.
Answer with citations and confidence. Prefer the local corpus over the web.
Do not invent product facts without sources. If sources conflict or coverage is thin, say so.

You have tools. Use them in a loop, then call finish.
- search_corpus first for Harness topics
- web_search only if corpus is thin
- fetch_url for detail on a strong source
- finish with tldr, findings (confidence + source_ids), known_unknowns, sources

Confidence: high = 2+ independent sources; medium = 1 solid source; low = weak; contested = disagreement.
Every finding needs source_ids that appear in sources.
If you cannot answer, finish and say so in known_unknowns."#
}

pub fn use_case_addendum(use_case: UseCase) -> &'static str {
    match use_case {
        UseCase::Ask => {
            "Use case ASK: clear TL;DR and scannable findings with citations."
        }
        UseCase::Compare => {
            "Use case COMPARE: cover shared goals, differences, trade-offs, when to pick each; mark contested points."
        }
        UseCase::Howto => {
            "Use case HOWTO: order findings as actionable steps; note prerequisites and failure modes in known_unknowns if unclear."
        }
    }
}

pub fn system_prompt(use_case: UseCase) -> String {
    format!(
        "{}\n{}",
        core_system_prompt().trim(),
        use_case_addendum(use_case).trim()
    )
}

pub fn user_message(question: &str) -> String {
    format!(
        "Research question:\n{}\n\nSearch the corpus if relevant, then finish with a cited report.",
        question.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_is_short() {
        let p = system_prompt(UseCase::Ask);
        // Pi-style: keep well under ~1k tokens (~4k chars)
        assert!(p.len() < 1500, "prompt too long: {} chars", p.len());
        assert!(p.contains("search_corpus") || p.contains("corpus"));
        assert!(p.contains("finish"));
    }

    #[test]
    fn compare_addendum_present() {
        let p = system_prompt(UseCase::Compare);
        assert!(p.to_lowercase().contains("compare"));
    }
}
