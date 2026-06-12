use std::time::Instant;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::config::ScaffConfig;
use crate::corpus;
use crate::llm::{self, ChatMessage};
use crate::pipeline::{Claim, Confidence, Finding, Source, SourceKind, Stance, VerificationResult};
use crate::render;
use crate::search::{Index, SearchHit};
use crate::web::{self, WebResult};

pub struct ResearchOptions {
    pub retrieval_k: usize,
    pub web_enabled: bool,
    pub max_sub_questions: usize,
    pub max_search_queries: usize,
    pub model: String,
    pub temperature: f32,
    pub max_output_tokens: u32,
    pub show_cost: bool,
    /// Called as each finding is verified — enables progressive streaming.
    pub on_finding: Option<Box<dyn FnMut(&Finding)>>,
}

impl Default for ResearchOptions {
    fn default() -> Self {
        Self {
            retrieval_k: 8,
            web_enabled: true,
            max_sub_questions: 4,
            max_search_queries: 6,
            model: String::new(),
            temperature: 0.3,
            max_output_tokens: 2048,
            show_cost: false,
            on_finding: None,
        }
    }
}

impl std::fmt::Debug for ResearchOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResearchOptions")
            .field("retrieval_k", &self.retrieval_k)
            .field("web_enabled", &self.web_enabled)
            .field("max_sub_questions", &self.max_sub_questions)
            .field("max_search_queries", &self.max_search_queries)
            .field("model", &self.model)
            .field("temperature", &self.temperature)
            .field("max_output_tokens", &self.max_output_tokens)
            .field("show_cost", &self.show_cost)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResearchStats {
    pub sub_questions: usize,
    pub search_queries: usize,
    pub corpus_chunks_retrieved: usize,
    pub web_results_retrieved: usize,
    pub claims_extracted: usize,
    pub claims_verified: usize,
    pub planner_input_tokens: u32,
    pub planner_output_tokens: u32,
    pub synth_input_tokens: u32,
    pub synth_output_tokens: u32,
    pub verifier_input_tokens: u32,
    pub verifier_output_tokens: u32,
    pub elapsed_ms: u128,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone)]
pub struct ResearchOutput {
    pub report: String,
    pub tldr: String,
    pub body: String,
    pub stats: ResearchStats,
    pub corpus_hits: Vec<SearchHit>,
    pub web_results: Vec<WebResult>,
    pub claims: Vec<Claim>,
    pub verifications: Vec<VerificationResult>,
    pub findings: Vec<Finding>,
}

pub fn run_research(
    question: &str,
    _config: &ScaffConfig,
    provider: &crate::config::ProviderSpec,
    api_key: Option<&str>,
    index: &Index,
    options: &mut ResearchOptions,
) -> Result<ResearchOutput> {
    let started = Instant::now();
    let mut stats = ResearchStats::default();

    // Validate input — empty questions go nowhere.
    let question = question.trim();
    if question.is_empty() || question.len() < 3 {
        anyhow::bail!("Question must be at least 3 characters");
    }
    if question.len() > 5000 {
        anyhow::bail!("Question is too long ({} chars, max 5000)", question.len());
    }

    let sub_questions = retry_llm(
        || plan(question, provider, api_key, options, &mut stats),
        "plan",
        1,
    )?;
    stats.sub_questions = sub_questions.len();

    let search_queries = web::generate_search_queries(question, &sub_questions, options.max_search_queries);
    stats.search_queries = search_queries.len();

    let mut all_hits: Vec<SearchHit> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for sq in &sub_questions {
        let hits = index.search(sq, options.retrieval_k);
        for h in hits {
            if seen.insert(h.chunk.id.clone()) {
                all_hits.push(h);
            }
        }
    }

    let primary = index.search(question, options.retrieval_k);
    for h in primary {
        if seen.insert(h.chunk.id.clone()) {
            all_hits.push(h);
        }
    }
    all_hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all_hits.truncate(options.retrieval_k);
    stats.corpus_chunks_retrieved = all_hits.len();

    let web_results: Vec<WebResult> = if options.web_enabled {
        let results = web::parallel_search(&search_queries, 5);
        stats.web_results_retrieved = results.len();
        results
    } else {
        Vec::new()
    };

    let (claims, verifications) = if options.web_enabled || stats.corpus_chunks_retrieved > 2 {
        match retry_llm(
            || verify_claims(question, &sub_questions, &all_hits, &web_results, provider, api_key, options, &mut stats),
            "verify",
            1,
        ) {
            Ok((c, v)) => {
                stats.claims_extracted = c.len();
                stats.claims_verified = v.iter().filter(|vr| vr.supported).count();
                (c, v)
            }
            Err(e) => {
                // Don't fail — synthesize from raw sources instead.
                eprintln!("  {} verification failed (continuing with raw sources): {e}", console::style("warn").yellow());
                (Vec::new(), Vec::new())
            }
        }
    } else {
        (Vec::new(), Vec::new())
    };

    // Build sources list early so findings can reference them.
    let mut sources: Vec<Source> = all_hits
        .iter()
        .enumerate()
        .map(|(i, h)| Source {
            id: i,
            title: h.chunk.title.clone(),
            url: h.chunk.url.clone(),
            kind: SourceKind::Corpus,
            score: h.score,
        })
        .collect();
    let src_offset = sources.len();
    for (i, w) in web_results.iter().enumerate() {
        sources.push(Source {
            id: src_offset + i,
            title: w.title.clone(),
            url: w.url.clone(),
            kind: SourceKind::Web,
            score: 0.0,
        });
    }

    // Stream findings immediately as they're verified.
    let findings: Vec<Finding> = claims
        .iter()
        .enumerate()
        .map(|(i, claim)| {
            let claim_sources: Vec<Source> = claim
                .source_ids
                .iter()
                .filter_map(|&sid| sources.iter().find(|s| s.id == sid).cloned())
                .collect();
            Finding {
                index: i + 1,
                total: claims.len(),
                confidence: claim.confidence,
                content: claim.text.clone(),
                sources: claim_sources,
                conflict_note: claim.conflict_note.clone(),
            }
        })
        .collect();

    // Call the finding callback for each verified finding.
    if let Some(ref mut cb) = options.on_finding {
        for f in &findings {
            cb(f);
        }
    }

    let (tldr, body) = retry_llm(
        || synthesize(question, &sub_questions, &all_hits, &web_results, &claims, &verifications, provider, api_key, options, &mut stats),
        "synthesize",
        1,
    )
    .unwrap_or_else(|e| {
        eprintln!("  {} synthesis failed: {e}", console::style("warn").yellow());
        ("Synthesis failed — see sources below.".to_string(), format!("An error occurred during synthesis: {e}"))
    });

    let report = render::render_report(question, &tldr, &body, &all_hits, &web_results, &claims);

    let total_in = stats.planner_input_tokens + stats.synth_input_tokens + stats.verifier_input_tokens;
    let total_out = stats.planner_output_tokens + stats.synth_output_tokens + stats.verifier_output_tokens;
    stats.estimated_cost_usd = llm::approximate_cost(&options.model, total_in, total_out);
    stats.elapsed_ms = started.elapsed().as_millis();

    Ok(ResearchOutput {
        report,
        tldr,
        body,
        stats,
        corpus_hits: all_hits,
        web_results,
        claims,
        verifications,
        findings,
    })
}

fn plan(
    question: &str,
    provider: &crate::config::ProviderSpec,
    api_key: Option<&str>,
    options: &ResearchOptions,
    stats: &mut ResearchStats,
) -> Result<Vec<String>> {
    let max = options.max_sub_questions.max(1);
    let system = ChatMessage::system(format!(
        "You are a research planner for a deep-research agent. \
         Decompose the user's research question into {max} or fewer specific, self-contained \
         sub-questions that, when answered individually, would let you synthesize a complete \
         answer to the original question. Each sub-question should be search-friendly and \
         specific. Return ONLY valid JSON in the form {{\"sub_questions\": [\"...\"]}}."
    ));
    let user = ChatMessage::user(format!(
        "Research question: {question}\n\nReturn JSON only."
    ));
    let resp = llm::chat(
        provider,
        api_key,
        &options.model,
        &[system, user],
        0.2,
        options.max_output_tokens.min(800),
    )?;
    stats.planner_input_tokens += resp.usage.input_tokens.unwrap_or(0);
    stats.planner_output_tokens += resp.usage.output_tokens.unwrap_or(0);

    let parsed: PlannerJson = extract_json(&resp.content)
        .ok_or_else(|| anyhow!("planner returned no parseable JSON: {}", trim(&resp.content, 400)))?;
    let mut subs = parsed.sub_questions;
    if subs.len() > max {
        subs.truncate(max);
    }
    if subs.is_empty() {
        subs.push(question.to_string());
    }
    Ok(subs)
}

fn verify_claims(
    question: &str,
    sub_questions: &[String],
    hits: &[SearchHit],
    web_results: &[WebResult],
    provider: &crate::config::ProviderSpec,
    api_key: Option<&str>,
    options: &ResearchOptions,
    stats: &mut ResearchStats,
) -> Result<(Vec<Claim>, Vec<VerificationResult>)> {
    let sources_text = format_sources_text(hits, web_results);

    let system = ChatMessage::system(
        "You are a claim extractor and verifier for a deep-research agent. \
         Given a research question and sources, extract the key factual claims and \
         identify which sources support each claim.\n\nReturn ONLY valid JSON:\n\
         {{\n  \"claims\": [\n    {{\n      \"text\": \"claim text\",\n      \"source_ids\": [0, 1],\n      \"confidence\": 0.9\n    }}\n  ]\n}}\n\n\
         Confidence rules:\n\
         - 0.9-1.0: Explicitly stated in 2+ independent sources\n\
         - 0.7-0.89: Stated in 1 source or implied by multiple sources\n\
         - 0.5-0.69: Partially supported, some ambiguity\n\
         - Below 0.5: Speculative or contradicted"
    );

    let sub_list = sub_questions.iter().map(|s| format!("- {s}")).collect::<Vec<_>>().join("\n");
    let user = ChatMessage::user(format!(
        "Question: {question}\n\nSub-questions:\n{sub_list}\n\nSources:\n{sources_text}\n\nExtract claims with confidence scores. Return JSON only."
    ));

    let resp = llm::chat(
        provider,
        api_key,
        &options.model,
        &[system, user],
        0.1,
        options.max_output_tokens.min(1500),
    )?;
    stats.verifier_input_tokens += resp.usage.input_tokens.unwrap_or(0);
    stats.verifier_output_tokens += resp.usage.output_tokens.unwrap_or(0);

    let claims = extract_claims_json(&resp.content);

    let mut verifications: Vec<VerificationResult> = Vec::new();
    for claim in &claims {
        for &sid in &claim.source_ids {
            let supported = claim.confidence != Confidence::Low;
            let stance = if supported {
                Stance::Supports
            } else if claim.conflict_note.is_some() {
                Stance::Contradicts
            } else {
                Stance::Neutral
            };
            verifications.push(VerificationResult {
                claim_id: claim.id,
                source_id: sid,
                supported,
                evidence: if supported {
                    "Source supports this claim".to_string()
                } else if stance == Stance::Contradicts {
                    "Source contradicts this claim".to_string()
                } else {
                    "Source does not clearly support this claim".to_string()
                },
                stance,
            });
        }
    }

    Ok((claims, verifications))
}

fn synthesize(
    question: &str,
    sub_questions: &[String],
    hits: &[SearchHit],
    web_results: &[WebResult],
    claims: &[Claim],
    verifications: &[VerificationResult],
    provider: &crate::config::ProviderSpec,
    api_key: Option<&str>,
    options: &ResearchOptions,
    stats: &mut ResearchStats,
) -> Result<(String, String)> {
    let mut sources_text = format_sources_text(hits, web_results);

    if !claims.is_empty() {
        sources_text.push_str("\n\n## Verified Claims (from verification stage)\n\n");
        for claim in claims {
            let supported_count = verifications
                .iter()
                .filter(|v| v.claim_id == claim.id && v.supported)
                .count();
            let status = if supported_count >= 2 {
                "HIGH CONFIDENCE"
            } else if supported_count == 1 {
                "MEDIUM CONFIDENCE"
            } else {
                "LOW CONFIDENCE"
            };
            sources_text.push_str(&format!(
                "- [{}] {} (confidence: {}, sources: {}, status: {})\n",
                claim.id, claim.text, claim.confidence, claim.source_ids.len(), status
            ));
        }
    }

    let sub_list = sub_questions
        .iter()
        .map(|s| format!("- {s}"))
        .collect::<Vec<_>>()
        .join("\n");

    let system = ChatMessage::system(
        "You are a research synthesizer for a deep-research agent. You will be given a \
         research question, a list of sub-questions, and a set of numbered sources (some \
         from an internal corpus, some from the web). You will also receive verified claims \
         with confidence scores from the verification stage.\n\nRules:\n\
         - Every non-trivial claim must have an inline [N] citation that matches a source.\n\
         - If a source does not actually support a claim, do not cite it.\n\
         - Use the verified claims to guide your confidence: HIGH claims can be stated firmly, \
         MEDIUM claims should note the evidence, LOW claims should be hedged or omitted.\n\
         - If sources do not cover part of the question, say so in 'Known Unknowns'.\n\
         - Keep the report focused and well-organized.\n\
         - Use ## headings only.\n\n\
         Output exactly:\n\
         ## TL;DR\n<1-3 sentences>\n\n\
         ## Confidence Summary\n<bullet list of key findings with confidence levels>\n\n\
         ## Findings\n<body with [N] inline citations>\n\n\
         ## Known Unknowns\n<bulleted list, or 'None'>"
    );

    let user = ChatMessage::user(format!(
        "Question: {question}\n\nSub-questions to address:\n{sub_list}\n\nSources:\n{sources_text}\n\nProduce the Markdown report now."
    ));

    let resp = llm::chat(
        provider,
        api_key,
        &options.model,
        &[system, user],
        options.temperature,
        options.max_output_tokens,
    )?;
    stats.synth_input_tokens += resp.usage.input_tokens.unwrap_or(0);
    stats.synth_output_tokens += resp.usage.output_tokens.unwrap_or(0);

    Ok(parse_synthesized(&resp.content))
}

fn format_sources_text(hits: &[SearchHit], web_results: &[WebResult]) -> String {
    let mut sources_text = String::new();
    for (i, h) in hits.iter().enumerate() {
        sources_text.push_str(&format!(
            "[{}] {}\nURL: {}\nSection: {}\nContent:\n{}\n\n",
            i + 1,
            h.chunk.title,
            h.chunk.url,
            h.chunk.section.clone().unwrap_or_else(|| "(none)".to_string()),
            truncate_chars(&h.chunk.content, 1800)
        ));
    }
    for (i, w) in web_results.iter().enumerate() {
        let id = hits.len() + i + 1;
        sources_text.push_str(&format!(
            "[{}] {}\nURL: {}\nSnippet: {}\n\n",
            id, w.title, w.url, w.snippet
        ));
    }
    sources_text
}

fn parse_synthesized(text: &str) -> (String, String) {
    let mut tldr = String::new();
    let mut body = String::new();
    let mut known = String::new();

    let mut section: &str = "none";
    for line in text.lines() {
        let trimmed = line.trim_end();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("## tldr") || lower.starts_with("# tldr") {
            section = "tldr";
            continue;
        }
        if lower.starts_with("## findings") || lower.starts_with("# findings") {
            section = "body";
            continue;
        }
        if lower.starts_with("## known unknowns") || lower.starts_with("# known unknowns") {
            section = "known";
            continue;
        }
        if lower.starts_with("## confidence") || lower.starts_with("# confidence") {
            section = "body";
            continue;
        }
        if lower.starts_with("# ") || lower.starts_with("## ") {
            section = "body";
        }
        match section {
            "tldr" => {
                if !trimmed.is_empty() {
                    if !tldr.is_empty() {
                        tldr.push(' ');
                    }
                    tldr.push_str(trimmed);
                }
            }
            "known" => {
                known.push_str(trimmed);
                known.push('\n');
            }
            _ => {
                body.push_str(trimmed);
                body.push('\n');
            }
        }
    }
    if !known.trim().is_empty() && !known.trim().eq_ignore_ascii_case("none") {
        body.push_str("\n\n## Known Unknowns\n\n");
        body.push_str(known.trim());
    }
    (tldr.trim().to_string(), body.trim().to_string())
}

#[derive(Deserialize)]
struct PlannerJson {
    sub_questions: Vec<String>,
}

#[derive(Deserialize)]
struct ClaimsJson {
    claims: Vec<ClaimJson>,
}

#[derive(Deserialize)]
struct ClaimJson {
    text: String,
    source_ids: Vec<usize>,
    confidence: f64,
    conflict_note: Option<String>,
}

fn extract_json(text: &str) -> Option<PlannerJson> {
    if let Ok(p) = serde_json::from_str::<PlannerJson>(text) {
        return Some(p);
    }
    if let Some(start) = text.find("```") {
        let rest = &text[start + 3..];
        if let Some(end) = rest.find("```") {
            let block = rest[..end].trim();
            let inner_owned;
            let inner: &str = if block.starts_with("json") {
                inner_owned = block.replacen("json", "", 1).trim().to_string();
                &inner_owned
            } else {
                block
            };
            if let Ok(p) = serde_json::from_str::<PlannerJson>(inner) {
                return Some(p);
            }
        }
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if let Ok(p) = serde_json::from_str::<PlannerJson>(&text[start..=end]) {
                return Some(p);
            }
        }
    }
    None
}

fn extract_claims_json(text: &str) -> Vec<Claim> {
    let parsed: Option<ClaimsJson> = if let Ok(p) = serde_json::from_str::<ClaimsJson>(text) {
        Some(p)
    } else if let Some(start) = text.find("```") {
        let rest = &text[start + 3..];
        if let Some(end) = rest.find("```") {
            let block = rest[..end].trim();
            let inner_owned;
            let inner: &str = if block.starts_with("json") {
                inner_owned = block.replacen("json", "", 1).trim().to_string();
                &inner_owned
            } else {
                block
            };
            serde_json::from_str::<ClaimsJson>(inner).ok()
        } else {
            None
        }
    } else if let Some(start) = text.find('{') {
        text.rfind('}').and_then(|end| {
            serde_json::from_str::<ClaimsJson>(&text[start..=end]).ok()
        })
    } else {
        None
    };

    parsed
        .map(|c| {
            c.claims
                .into_iter()
                .enumerate()
                .map(|(i, cj)| {
                    let has_contradiction = cj.conflict_note.is_some();
                    Claim {
                        id: i,
                        text: cj.text,
                        source_ids: cj.source_ids,
                        confidence: Confidence::from_score(cj.confidence, has_contradiction),
                        conflict_note: cj.conflict_note,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push_str("...");
    out
}

fn trim(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}\u{2026}", &s[..n])
    }
}

pub fn ensure_corpus_ready() -> Result<()> {
    corpus::ensure_seeded()?;
    Ok(())
}

/// Retry an LLM operation once on JSON parse failures.
fn retry_llm<F, T>(mut f: F, _stage: &str, max_retries: u32) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let mut delay_ms = 500u64;
    for attempt in 0..=max_retries {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) if attempt < max_retries => {
                let is_json = e.to_string().contains("parseable")
                    || e.to_string().contains("no parseable")
                    || e.to_string().contains("JSON");
                if is_json {
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    delay_ms *= 2;
                    continue;
                }
                return Err(e);
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
