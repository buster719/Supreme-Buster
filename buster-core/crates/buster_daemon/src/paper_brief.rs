//! Lightweight paper reading fallback for Buster research.
//!
//! This does not replace PaperQA's citation-grounded RAG. It gives Buster a
//! bounded way to read already acquired open papers and write a traceable brief
//! even when an embedding provider is not available yet.

use std::fs;
use std::path::{Component, Path, PathBuf};

use buster_body::llm::{LlmMessage, LlmPrompt};
use buster_value_model::ResearchDomain;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::paper_acquisition::{PAPER_MANIFEST, PAPER_TEXT_DIR};
use crate::research_framework::{self, ResearchQualityInput};
use crate::{append_daemon_jsonl, body_gate_bridge, executor, harness, now_secs};

pub const PAPER_BRIEF_AUDIT: &str = "audit/paper-brief.jsonl";
pub const PAPER_BRIEF_DIR: &str = "research/paperqa/briefs";
const MAX_SOURCE_CHARS: usize = 42_000;
const MAX_BRIEF_QUESTION_CHARS: usize = 1_200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperBriefRequest {
    pub question: String,
    pub domain: ResearchDomain,
    pub task_id: Option<String>,
    pub max_papers: usize,
}

impl PaperBriefRequest {
    pub fn new(question: impl Into<String>) -> Self {
        Self {
            question: question.into(),
            domain: ResearchDomain::Other,
            task_id: None,
            max_papers: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperBriefRecord {
    pub timestamp_secs: u64,
    pub contract_id: Option<String>,
    pub task_id: Option<String>,
    pub domain: ResearchDomain,
    pub question: String,
    pub status: String,
    pub selected_papers: Vec<BriefPaperSource>,
    pub brief_path: Option<PathBuf>,
    pub summary: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub total_tokens: Option<u64>,
    pub research_quality_score: Option<f32>,
    pub research_claim_status: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BriefPaperSource {
    pub title: String,
    pub source_id: String,
    pub sha256: String,
    pub text_path: PathBuf,
    pub text_chars: usize,
    pub pdf_url: Option<String>,
    pub landing_url: Option<String>,
}

pub fn brief_latest(root: &Path, request: PaperBriefRequest) -> std::io::Result<PaperBriefRecord> {
    ensure_layout(root)?;
    let timestamp_secs = now_secs();
    let question = truncate_chars(request.question.trim(), MAX_BRIEF_QUESTION_CHARS);
    let contract = harness::paper_brief_contract(timestamp_secs, request.domain, &question);
    let contract_id = contract.contract_id.clone();
    harness::append_contract(root, &contract)?;

    let papers = select_papers(root, request.domain, request.max_papers.clamp(1, 3))?;
    if papers.is_empty() {
        let record = PaperBriefRecord {
            timestamp_secs,
            contract_id: Some(contract_id.clone()),
            task_id: request.task_id,
            domain: request.domain,
            question,
            status: "no_extracted_text".to_string(),
            selected_papers: Vec::new(),
            brief_path: None,
            summary: "No extracted paper text is available; acquire papers first.".to_string(),
            model: None,
            provider: None,
            total_tokens: None,
            research_quality_score: None,
            research_claim_status: None,
            error: None,
        };
        append_daemon_jsonl(&root.join(PAPER_BRIEF_AUDIT), &record)?;
        harness::append_outcome(
            root,
            &harness::HarnessOutcome::new(
                contract_id,
                harness::HarnessStatus::Partial,
                record.summary.clone(),
            )
            .with_output("audit", PAPER_BRIEF_AUDIT, "paper brief audit"),
        )?;
        return Ok(record);
    }

    let corpus = read_corpus_excerpt(&papers)?;
    let prompt = paper_brief_prompt(&question, &papers, &corpus);
    let completion = executor::synthesize_with_body_gate(
        root,
        prompt,
        "research.paper_brief",
        "paper brief synthesis over extracted open-paper text",
    );

    let (status, brief, model, provider, total_tokens, error) = match completion {
        Ok(completion) => (
            "ok".to_string(),
            completion.content,
            Some(completion.model),
            completion.provider,
            completion.total_tokens,
            None,
        ),
        Err(error) => (
            "extractive_fallback".to_string(),
            extractive_fallback_brief(&question, &papers, &corpus, &error.to_string()),
            None,
            None,
            None,
            Some(error.to_string()),
        ),
    };

    let brief_path = write_brief(root, timestamp_secs, request.domain, &question, &brief)?;
    let quality = research_framework::evaluate_and_record(
        root,
        ResearchQualityInput {
            contract_id: Some(&contract_id),
            task_id: request.task_id.as_deref(),
            domain: request.domain,
            question: &question,
            source_count: papers.len(),
            content: Some(&brief),
            synthesis_succeeded: status == "ok",
            report_path: Some(brief_path.clone()),
        },
    )?;
    let summary = first_lines(&brief, 4);
    let record = PaperBriefRecord {
        timestamp_secs,
        contract_id: Some(contract_id.clone()),
        task_id: request.task_id,
        domain: request.domain,
        question,
        status: status.clone(),
        selected_papers: papers,
        brief_path: Some(brief_path.clone()),
        summary,
        model,
        provider,
        total_tokens,
        research_quality_score: Some(quality.quality_score),
        research_claim_status: Some(quality.claim_status.as_str().to_string()),
        error,
    };
    append_daemon_jsonl(&root.join(PAPER_BRIEF_AUDIT), &record)?;
    let mut outcome = harness::HarnessOutcome::new(
        contract_id,
        if status == "ok" {
            harness::HarnessStatus::Completed
        } else {
            harness::HarnessStatus::Partial
        },
        format!("paper brief {status}: {}", record.summary),
    )
    .with_output(
        "paper_brief",
        brief_path.display().to_string(),
        "paper reading brief",
    )
    .with_output("audit", PAPER_BRIEF_AUDIT, "paper brief audit");
    for paper in &record.selected_papers {
        outcome = outcome.with_evidence(
            "extracted_text",
            paper.text_path.display().to_string(),
            "bounded source excerpt read for brief",
        );
    }
    if let Some(error) = &record.error {
        outcome = outcome.with_error(error.clone());
    }
    harness::append_outcome(root, &outcome)?;
    Ok(record)
}

pub fn tail(root: &Path, max_lines: usize) -> std::io::Result<Vec<String>> {
    let text = fs::read_to_string(root.join(PAPER_BRIEF_AUDIT)).unwrap_or_default();
    let mut lines = text.lines().map(ToString::to_string).collect::<Vec<_>>();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    Ok(lines)
}

fn ensure_layout(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join(PAPER_BRIEF_DIR))?;
    fs::create_dir_all(root.join(PAPER_TEXT_DIR))?;
    fs::create_dir_all(root.join("audit"))?;
    Ok(())
}

fn select_papers(
    root: &Path,
    domain: ResearchDomain,
    limit: usize,
) -> std::io::Result<Vec<BriefPaperSource>> {
    let manifest = root.join(PAPER_MANIFEST);
    let raw = fs::read_to_string(&manifest).unwrap_or_default();
    let mut papers = Vec::new();
    for line in raw.lines().rev() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if domain != ResearchDomain::Other {
            let Ok(record_domain) =
                serde_json::from_value::<ResearchDomain>(value["domain"].clone())
            else {
                continue;
            };
            if record_domain != domain {
                continue;
            }
        }
        let Some(text_path) = value
            .get("text_path")
            .and_then(Value::as_str)
            .map(|path| resolve_workspace_path(root, path))
            .filter(|path| path.is_file())
        else {
            continue;
        };
        let source = BriefPaperSource {
            title: string_field(&value, "title").unwrap_or_else(|| "untitled paper".to_string()),
            source_id: string_field(&value, "source_id").unwrap_or_default(),
            sha256: string_field(&value, "sha256").unwrap_or_default(),
            text_chars: value.get("text_chars").and_then(Value::as_u64).unwrap_or(0) as usize,
            pdf_url: string_field(&value, "pdf_url"),
            landing_url: string_field(&value, "landing_url"),
            text_path,
        };
        papers.push(source);
        if papers.len() >= limit {
            break;
        }
    }
    Ok(papers)
}

fn resolve_workspace_path(root: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() || path.exists() {
        return path;
    }
    let joined = root.join(&path);
    if joined.exists() {
        return joined;
    }
    root.join(strip_parent_prefixes(&path))
}

fn strip_parent_prefixes(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir | Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn read_corpus_excerpt(papers: &[BriefPaperSource]) -> std::io::Result<String> {
    let per_paper = (MAX_SOURCE_CHARS / papers.len().max(1)).max(8_000);
    let mut out = String::new();
    for paper in papers {
        let text = fs::read_to_string(&paper.text_path)?;
        out.push_str(&format!(
            "\n\n=== Paper: {} ===\nsource_id: {}\nsha256: {}\npdf_url: {}\n\n",
            paper.title,
            paper.source_id,
            paper.sha256,
            paper.pdf_url.clone().unwrap_or_default()
        ));
        out.push_str(&truncate_chars(&text, per_paper));
    }
    Ok(out)
}

fn paper_brief_prompt(question: &str, papers: &[BriefPaperSource], corpus: &str) -> LlmPrompt {
    let system = "\
Runtime identity:
This execution is a cognitive process of Buster.
Buster is reading extracted text from legally acquired open papers.

Research brief rules:
- Answer in Chinese.
- Do not claim the whole literature is settled.
- Separate paper evidence from your synthesis.
- Include a claim table with claim, support, uncertainty, and falsifier.
- Include counterarguments or alternative explanations.
- Include what would change Buster's mind.
- Include concrete verification leads.
- Include one small local/sandbox mini-experiment when possible.
- Preserve uncertainty and cite paper titles/hash identifiers, not invented citations.";
    let sources = papers
        .iter()
        .map(|paper| {
            format!(
                "- {} | sha256={} | text_chars={}",
                paper.title, paper.sha256, paper.text_chars
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let user = format!(
        "Question:\n{question}\n\nSelected papers:\n{sources}\n\nExtracted source text excerpt:\n{corpus}\n\nWrite a compact research brief that Buster can remember and later verify."
    );
    LlmPrompt {
        messages: vec![LlmMessage::system(system), LlmMessage::user(user)],
        max_tokens: Some(1_800),
    }
}

fn extractive_fallback_brief(
    question: &str,
    papers: &[BriefPaperSource],
    corpus: &str,
    error: &str,
) -> String {
    let titles = papers
        .iter()
        .map(|paper| format!("- {} | sha256={}", paper.title, paper.sha256))
        .collect::<Vec<_>>()
        .join("\n");
    let snippets = extractive_snippets(question, corpus, 6)
        .into_iter()
        .enumerate()
        .map(|(index, snippet)| format!("{}. {}", index + 1, snippet))
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "# Paper Brief: Extractive Fallback\n\nQuestion: {question}\n\nSelected papers:\n{titles}\n\nLLM synthesis status: failed, so this brief is an extractive fallback rather than a full interpretation.\n\nFailure: {error}\n\n## Claim table\n\n| Claim | Support | Uncertainty | Falsifier |\n| --- | --- | --- | --- |\n| The selected paper contains relevant material for the question. | Keyword-matched snippets below. | This is not yet a semantic synthesis. | A later LLM or PaperQA pass finds these snippets irrelevant or contradicted. |\n| Buster has preserved a retryable research trail. | Paper title/hash and extracted text path are recorded in audit. | No cross-paper comparison yet. | Manifest/hash mismatch or missing extracted text. |\n\n## Extracted snippets\n\n{snippets}\n\n## Counterarguments / alternative explanations\n\n- Keyword overlap can select introductory or peripheral passages rather than milestone evidence.\n- A single paper may be outdated or insufficient for current fault-tolerant quantum-computing milestones.\n\n## What would change Buster's mind\n\n- A successful PaperQA/LLM pass over a larger corpus.\n- Newer arXiv or review papers that identify different logical-qubit milestone criteria.\n- Direct source evidence from experimental roadmaps or hardware benchmark papers.\n\n## Verification leads\n\n- Re-run `papers brief` when the configured LLM endpoint is healthy.\n- Add 2-3 newer arXiv review or roadmap papers to the corpus.\n- Compare extracted claims against recent surface-code and logical-qubit demonstration papers.\n\n## Mini-experiment\n\n- Build a small local table with columns: paper, year, code family, physical qubits, logical qubits, logical error rate, and break-even criterion.\n"
    )
}

fn extractive_snippets(question: &str, corpus: &str, limit: usize) -> Vec<String> {
    let terms = question_terms(question);
    let mut scored = corpus
        .split("\n\n")
        .map(str::trim)
        .filter(|chunk| chunk.chars().count() >= 80)
        .map(|chunk| {
            let lower = chunk.to_ascii_lowercase();
            let score = terms
                .iter()
                .filter(|term| lower.contains(term.as_str()))
                .count();
            (score, chunk)
        })
        .filter(|(score, _)| *score > 0)
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.cmp(&left.0));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, chunk)| truncate_chars(chunk, 900))
        .collect()
}

fn question_terms(question: &str) -> Vec<String> {
    question
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::trim)
        .filter(|term| term.len() >= 4)
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

fn write_brief(
    root: &Path,
    timestamp_secs: u64,
    domain: ResearchDomain,
    question: &str,
    brief: &str,
) -> std::io::Result<PathBuf> {
    let path = root.join(PAPER_BRIEF_DIR).join(format!(
        "{}-{:?}-{}.md",
        timestamp_secs,
        domain,
        safe_file_stem(question)
    ));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    body_gate_bridge::record_memory_write(root, "paper_brief", "paper_brief.write_brief", brief)
        .map_err(std::io::Error::other)?;
    fs::write(&path, brief)?;
    Ok(path)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .filter(|value| !value.trim().is_empty())
}

fn first_lines(value: &str, count: usize) -> String {
    let lines = value
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(count)
        .collect::<Vec<_>>()
        .join("\n");
    truncate_chars(&lines, 900)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut out = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        out.push_str("\n[truncated]");
    }
    out
}

fn safe_file_stem(value: &str) -> String {
    let stem = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let stem = stem.trim_matches('-');
    if stem.is_empty() {
        "paper-brief".to_string()
    } else {
        stem.chars().take(64).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_parent_prefixes_for_workspace_paths() {
        assert_eq!(
            strip_parent_prefixes(Path::new("../research/paperqa/extracted/a.txt")),
            PathBuf::from("research/paperqa/extracted/a.txt")
        );
    }

    #[test]
    fn safe_stem_keeps_ascii_identifiers() {
        assert_eq!(
            safe_file_stem("fault tolerant quantum computing"),
            "fault-tolerant-quantum-computing"
        );
    }

    #[test]
    fn extractive_snippets_selects_question_terms() {
        let snippets = extractive_snippets(
            "logical qubit milestones",
            "unrelated paragraph\n\nlogical qubit milestone evidence with enough words to pass the minimum paragraph filter for this test case",
            2,
        );

        assert_eq!(snippets.len(), 1);
        assert!(snippets[0].contains("logical qubit"));
    }
}
