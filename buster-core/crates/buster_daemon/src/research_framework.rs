//! Research quality framework for Buster.
//!
//! This layer keeps research from becoming a loose note generator. It gives
//! every research output an evidence grade, claim status, and verification
//! pressure before the result can influence memory or follow-up work.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use buster_value_model::ResearchDomain;
use serde::{Deserialize, Serialize};

use crate::{body_gate_bridge, now_secs};

pub const RESEARCH_QUALITY_AUDIT: &str = "audit/research-quality.jsonl";
pub const RESEARCH_QUALITY_STATE: &str = "state/research-quality-digest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceGrade {
    None,
    Weak,
    Mixed,
    Good,
    Strong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimStatus {
    SourceFetchFailed,
    SynthesisFailed,
    PartialEvidenceOnly,
    SynthesizedUnverified,
    VerifiedSummary,
    UnsafeOrOutOfScope,
}

impl ClaimStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SourceFetchFailed => "source_fetch_failed",
            Self::SynthesisFailed => "synthesis_failed",
            Self::PartialEvidenceOnly => "partial_evidence_only",
            Self::SynthesizedUnverified => "synthesized_unverified",
            Self::VerifiedSummary => "verified_summary",
            Self::UnsafeOrOutOfScope => "unsafe_or_out_of_scope",
        }
    }

    pub const fn may_spawn_followups(self) -> bool {
        matches!(self, Self::SynthesizedUnverified | Self::VerifiedSummary)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchQualityReport {
    pub timestamp_secs: u64,
    pub contract_id: Option<String>,
    pub task_id: Option<String>,
    pub domain: ResearchDomain,
    pub question: String,
    pub claim_status: ClaimStatus,
    pub evidence_grade: EvidenceGrade,
    pub quality_score: f32,
    pub source_count: usize,
    pub has_claim_table: bool,
    pub has_counterarguments: bool,
    pub has_falsification: bool,
    pub has_mini_experiment: bool,
    pub has_verification_leads: bool,
    pub weaknesses: Vec<String>,
    pub memory_promotion: String,
    pub report_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchQualityDigest {
    pub updated_at_secs: u64,
    pub reports: usize,
    pub average_quality_score: f32,
    pub verified_or_unverified_syntheses: usize,
    pub failed_or_partial: usize,
    pub weak_evidence_reports: usize,
    pub latest: Vec<ResearchQualityReport>,
}

pub fn methodology_prompt() -> &'static str {
    "\
Research method requirements:
- Separate source evidence from model synthesis.
- Give a claim table with claim, support, uncertainty, and falsifier.
- Include counterarguments or alternative explanations.
- Name what would change Buster's mind.
- Include at least one concrete verification lead.
- Include one small local/sandbox mini-experiment when possible.
- Do not promote a claim to fact memory unless evidence is good enough."
}

pub struct ResearchQualityInput<'a> {
    pub contract_id: Option<&'a str>,
    pub task_id: Option<&'a str>,
    pub domain: ResearchDomain,
    pub question: &'a str,
    pub source_count: usize,
    pub content: Option<&'a str>,
    pub synthesis_succeeded: bool,
    pub report_path: Option<PathBuf>,
}

pub fn evaluate_and_record(
    root: &Path,
    input: ResearchQualityInput<'_>,
) -> std::io::Result<ResearchQualityReport> {
    let report = evaluate(input);
    let json = serde_json::to_string(&report).map_err(std::io::Error::other)?;
    body_gate_bridge::record_memory_write(
        root,
        "research_quality",
        "research_framework.evaluate_and_record",
        &json,
    )
    .map_err(std::io::Error::other)?;
    append_jsonl(&root.join(RESEARCH_QUALITY_AUDIT), &report)?;
    let _ = update_quality_digest(root)?;
    Ok(report)
}

pub fn update_quality_digest(root: &Path) -> std::io::Result<ResearchQualityDigest> {
    let reports = read_quality_reports(root)?;
    let average_quality_score = if reports.is_empty() {
        0.0
    } else {
        reports
            .iter()
            .map(|report| report.quality_score)
            .sum::<f32>()
            / reports.len() as f32
    };
    let digest = ResearchQualityDigest {
        updated_at_secs: now_secs(),
        reports: reports.len(),
        average_quality_score,
        verified_or_unverified_syntheses: reports
            .iter()
            .filter(|report| report.claim_status.may_spawn_followups())
            .count(),
        failed_or_partial: reports
            .iter()
            .filter(|report| {
                matches!(
                    report.claim_status,
                    ClaimStatus::SourceFetchFailed
                        | ClaimStatus::SynthesisFailed
                        | ClaimStatus::PartialEvidenceOnly
                )
            })
            .count(),
        weak_evidence_reports: reports
            .iter()
            .filter(|report| {
                matches!(
                    report.evidence_grade,
                    EvidenceGrade::None | EvidenceGrade::Weak
                )
            })
            .count(),
        latest: reports.into_iter().rev().take(10).collect(),
    };
    let path = root.join(RESEARCH_QUALITY_STATE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&digest).map_err(std::io::Error::other)?;
    fs::write(path, json)?;
    Ok(digest)
}

fn evaluate(input: ResearchQualityInput<'_>) -> ResearchQualityReport {
    let content = input.content.unwrap_or("");
    let lower = content.to_ascii_lowercase();
    let has_claim_table = lower.contains("claim table")
        || (lower.contains("claim") && lower.contains("support") && lower.contains("uncertainty"));
    let has_counterarguments = lower.contains("counterargument")
        || lower.contains("alternative explanation")
        || lower.contains("contradiction")
        || lower.contains("unknown");
    let has_falsification = lower.contains("falsif")
        || lower.contains("change buster's mind")
        || lower.contains("would change");
    let has_mini_experiment = lower.contains("mini-experiment")
        || lower.contains("experiment")
        || lower.contains("simulation")
        || lower.contains("local");
    let has_verification_leads = lower.contains("verification lead")
        || lower.contains("doi")
        || lower.contains("arxiv")
        || lower.contains("dataset");

    let evidence_grade = evidence_grade(input.source_count, has_verification_leads);
    let mut score: f32 = 0.0;
    score += match evidence_grade {
        EvidenceGrade::None => 0.0,
        EvidenceGrade::Weak => 0.16,
        EvidenceGrade::Mixed => 0.28,
        EvidenceGrade::Good => 0.38,
        EvidenceGrade::Strong => 0.45,
    };
    if has_claim_table {
        score += 0.16;
    }
    if has_counterarguments {
        score += 0.13;
    }
    if has_falsification {
        score += 0.12;
    }
    if has_mini_experiment {
        score += 0.08;
    }
    if has_verification_leads {
        score += 0.06;
    }
    if !input.synthesis_succeeded {
        score = score.min(0.35);
    }
    let quality_score = score.clamp(0.0, 1.0);
    let claim_status = claim_status(input.synthesis_succeeded, input.source_count, quality_score);

    let mut weaknesses = Vec::new();
    if input.source_count == 0 {
        weaknesses.push("no fetched sources".to_string());
    }
    if !has_claim_table {
        weaknesses.push("missing explicit claim/support/uncertainty table".to_string());
    }
    if !has_counterarguments {
        weaknesses.push("missing counterarguments or alternative explanations".to_string());
    }
    if !has_falsification {
        weaknesses.push("missing falsification condition".to_string());
    }
    if !has_verification_leads {
        weaknesses.push("missing concrete verification leads".to_string());
    }

    ResearchQualityReport {
        timestamp_secs: now_secs(),
        contract_id: input.contract_id.map(ToString::to_string),
        task_id: input.task_id.map(ToString::to_string),
        domain: input.domain,
        question: input.question.to_string(),
        claim_status,
        evidence_grade,
        quality_score,
        source_count: input.source_count,
        has_claim_table,
        has_counterarguments,
        has_falsification,
        has_mini_experiment,
        has_verification_leads,
        weaknesses,
        memory_promotion: memory_promotion_rule(claim_status, evidence_grade).to_string(),
        report_path: input.report_path,
    }
}

fn evidence_grade(source_count: usize, has_verification_leads: bool) -> EvidenceGrade {
    match (source_count, has_verification_leads) {
        (0, _) => EvidenceGrade::None,
        (1 | 2, false) => EvidenceGrade::Weak,
        (1 | 2, true) => EvidenceGrade::Mixed,
        (3..=5, false) => EvidenceGrade::Mixed,
        (3..=5, true) => EvidenceGrade::Good,
        (_, true) => EvidenceGrade::Strong,
        _ => EvidenceGrade::Good,
    }
}

fn claim_status(synthesis_succeeded: bool, source_count: usize, quality_score: f32) -> ClaimStatus {
    if source_count == 0 && !synthesis_succeeded {
        return ClaimStatus::SourceFetchFailed;
    }
    if !synthesis_succeeded {
        return ClaimStatus::SynthesisFailed;
    }
    if source_count == 0 {
        return ClaimStatus::SynthesizedUnverified;
    }
    if quality_score >= 0.78 && source_count >= 3 {
        ClaimStatus::VerifiedSummary
    } else if quality_score >= 0.45 {
        ClaimStatus::SynthesizedUnverified
    } else {
        ClaimStatus::PartialEvidenceOnly
    }
}

fn memory_promotion_rule(status: ClaimStatus, evidence_grade: EvidenceGrade) -> &'static str {
    match (status, evidence_grade) {
        (ClaimStatus::VerifiedSummary, EvidenceGrade::Good | EvidenceGrade::Strong) => {
            "eligible_for_fact_memory_with_sources"
        }
        (ClaimStatus::SynthesizedUnverified, _) => "observation_or_research_memory_only",
        (ClaimStatus::SynthesisFailed, _) | (ClaimStatus::PartialEvidenceOnly, _) => {
            "evidence_memory_only_no_fact_promotion"
        }
        (ClaimStatus::SourceFetchFailed, _) => "failure_memory_only",
        (ClaimStatus::UnsafeOrOutOfScope, _) => "quarantine_or_governance_review",
        _ => "observation_only",
    }
}

fn read_quality_reports(root: &Path) -> std::io::Result<Vec<ResearchQualityReport>> {
    let path = root.join(RESEARCH_QUALITY_AUDIT);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter_map(|line| serde_json::from_str::<ResearchQualityReport>(line).ok())
        .collect())
}

fn append_jsonl(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value).map_err(std::io::Error::other)?;
    file.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn grades_structured_research_higher_than_loose_notes() {
        let root = unique_temp_dir("buster-research-quality");
        let content = "\
Claim table:
| claim | support | uncertainty | falsifier |
| safer protocol | arxiv source | medium | counterexample |
Counterarguments: an alternative explanation exists.
Verification leads: arXiv:1234 and dataset.
Mini-experiment: run a local simulation.
";
        let report = evaluate_and_record(
            &root,
            ResearchQualityInput {
                contract_id: Some("hc-test"),
                task_id: Some("task"),
                domain: ResearchDomain::ProtocolSecurity,
                question: "test?",
                source_count: 4,
                content: Some(content),
                synthesis_succeeded: true,
                report_path: None,
            },
        )
        .unwrap();

        assert!(report.quality_score >= 0.78);
        assert_eq!(report.claim_status, ClaimStatus::VerifiedSummary);
        assert!(root.join(RESEARCH_QUALITY_AUDIT).exists());
        assert!(root.join(RESEARCH_QUALITY_STATE).exists());
        let _ = fs::remove_dir_all(root);
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
