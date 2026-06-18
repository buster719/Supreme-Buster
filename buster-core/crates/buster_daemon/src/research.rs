use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use buster_value_model::{ActionCandidate, ActionKind, InfoSource, ResearchDomain};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::research_framework;
use crate::{body_gate_bridge, now_secs};

const QUEUE_PATH: &str = "state/research-queue.json";
const LEDGER_PATH: &str = "audit/research-ledger.jsonl";
const DIGEST_PATH: &str = "state/research-digest.json";
const SUMMARY_PATH: &str = "research/RESEARCH_SUMMARY.md";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchQueue {
    pub version: u32,
    pub tasks: Vec<ResearchTask>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchTaskStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchTask {
    pub task_id: String,
    pub parent_task_id: Option<String>,
    pub domain: ResearchDomain,
    pub question: String,
    pub status: ResearchTaskStatus,
    pub priority: f32,
    pub created_at_secs: u64,
    pub updated_at_secs: u64,
    pub attempts: u32,
    pub source_strategy: ResearchSourceStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchSourceStrategy {
    LlmSecondaryFirst,
    NeedsWebVerification,
    NeedsPaperVerification,
    NeedsComputation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchLedgerEntry {
    #[serde(default)]
    pub contract_id: Option<String>,
    pub timestamp_secs: u64,
    pub task_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub domain: ResearchDomain,
    pub question: String,
    pub status: String,
    pub report_path: Option<PathBuf>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub total_tokens: Option<u64>,
    #[serde(default)]
    pub source_bundle_path: Option<PathBuf>,
    #[serde(default)]
    pub source_count: usize,
    #[serde(default)]
    pub verification_leads: Vec<String>,
    #[serde(default)]
    pub follow_up_questions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchDigest {
    pub updated_at_secs: u64,
    pub total_ledger_entries: usize,
    pub completed_entries: usize,
    pub failed_entries: usize,
    pub report_count: usize,
    pub fetched_source_count: usize,
    pub pending_task_count: usize,
    pub completed_task_count: usize,
    pub failed_task_count: usize,
    pub domain_summaries: Vec<ResearchDomainDigest>,
    pub recent_reports: Vec<ResearchDigestReport>,
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchDomainDigest {
    pub domain: ResearchDomain,
    pub completed_entries: usize,
    pub failed_entries: usize,
    pub pending_tasks: usize,
    pub fetched_source_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchDigestReport {
    pub timestamp_secs: u64,
    pub domain: ResearchDomain,
    pub question: String,
    pub status: String,
    pub report_path: Option<PathBuf>,
    pub source_count: usize,
}

pub fn next_research_candidate(root: &Path) -> std::io::Result<Option<ActionCandidate>> {
    let queue = ensure_research_queue(root)?;
    let Some(task) = select_next_task(&queue) else {
        return Ok(None);
    };

    Ok(Some(
        ActionCandidate::new(
            ActionKind::ScientificResearch,
            format!("research task {}: {}", task.task_id, task.question),
        )
        .with_research_domain(task.domain)
        .with_research_task(task.task_id.clone(), task.question.clone())
        .with_info_source(match task.source_strategy {
            ResearchSourceStrategy::LlmSecondaryFirst => InfoSource::LlmSecondary,
            ResearchSourceStrategy::NeedsWebVerification => InfoSource::Web,
            ResearchSourceStrategy::NeedsPaperVerification => InfoSource::Paper,
            ResearchSourceStrategy::NeedsComputation => InfoSource::Experiment,
        })
        .with_cost(0.32)
        .with_uncertainty(0.42),
    ))
}

pub fn research_prompt(domain: ResearchDomain, candidate: &ActionCandidate) -> String {
    let task_id = candidate
        .research_task_id
        .as_deref()
        .unwrap_or("ad-hoc-research");
    let question = candidate
        .research_question
        .as_deref()
        .unwrap_or(&candidate.summary);
    format!(
        "Buster selected a research task.\n\nTask id: {task_id}\nResearch domain: {:?}\nResearch question: {question}\n\nWrite a compact research-chain note. Treat this as secondary LLM information unless you can name verifiable sources. Do not invent citations. Maximum 900 words.\n\n{}\n\nRequired structure, in this exact order:\n1. Follow-up questions: 3-5 specific next research questions, each ending with a question mark.\n2. Verification leads: concrete papers, datasets, official sources, simulations, or search queries Buster should use next. Mark anything uncertain.\n3. Claim table: each row must include claim, source support, uncertainty, and falsifier.\n4. Working answer: what seems likely, with uncertainty.\n5. Counterarguments or unknowns: what could falsify or change the answer.\n6. Mini-experiment: one small thing Buster could run locally or in a sandbox.\n\nPrioritize research quality, falsifiability, and continuity over polished prose.",
        domain,
        research_framework::methodology_prompt()
    )
}

pub struct ResearchCompletion<'a> {
    pub contract_id: Option<&'a str>,
    pub task_id: Option<&'a str>,
    pub domain: ResearchDomain,
    pub question: &'a str,
    pub status: &'a str,
    pub report_path: Option<PathBuf>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub total_tokens: Option<u64>,
    pub source_bundle_path: Option<PathBuf>,
    pub source_count: usize,
    pub content: Option<&'a str>,
}

pub fn record_research_completion(
    root: &Path,
    completion: ResearchCompletion<'_>,
) -> std::io::Result<()> {
    let timestamp_secs = now_secs();
    let mut queue = ensure_research_queue(root)?;
    let completed_task_id = completion.task_id.map(ToString::to_string);
    let parent_task_id = completion.task_id.and_then(|task_id| {
        queue
            .tasks
            .iter()
            .find(|task| task.task_id == task_id)
            .and_then(|task| task.parent_task_id.clone())
    });

    if let Some(task_id) = completion.task_id {
        if let Some(task) = queue.tasks.iter_mut().find(|task| task.task_id == task_id) {
            if completion.status == "ok" {
                task.status = ResearchTaskStatus::Completed;
            } else if task.attempts + 1 >= 3 {
                task.status = ResearchTaskStatus::Failed;
            } else {
                task.status = ResearchTaskStatus::Pending;
                task.priority = (task.priority - 0.08).max(0.45);
            }
            task.updated_at_secs = timestamp_secs;
            task.attempts += 1;
        }
    }

    let mut follow_up_questions = if completion.status == "ok" {
        completion
            .content
            .map(extract_follow_up_questions)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    if completion.status == "ok" && follow_up_questions.is_empty() {
        follow_up_questions = fallback_follow_up_questions(completion.domain, completion.question);
    }
    let verification_leads = if completion.status == "ok" {
        completion
            .content
            .map(extract_verification_leads)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    if completion.status == "ok" {
        add_follow_up_tasks(
            &mut queue,
            completed_task_id.as_deref(),
            completion.domain,
            &follow_up_questions,
            timestamp_secs,
        );
    }
    write_research_queue(root, &queue)?;

    let entry = ResearchLedgerEntry {
        contract_id: completion.contract_id.map(ToString::to_string),
        timestamp_secs,
        task_id: completed_task_id,
        parent_task_id,
        domain: completion.domain,
        question: completion.question.to_string(),
        status: completion.status.to_string(),
        report_path: completion.report_path,
        model: completion.model,
        provider: completion.provider,
        total_tokens: completion.total_tokens,
        source_bundle_path: completion.source_bundle_path,
        source_count: completion.source_count,
        verification_leads,
        follow_up_questions,
    };
    let entry_json = serde_json::to_string(&entry).map_err(std::io::Error::other)?;
    body_gate_bridge::record_memory_write(
        root,
        "research_ledger",
        "research.record_research_completion",
        &entry_json,
    )
    .map_err(std::io::Error::other)?;
    append_jsonl(&root.join(LEDGER_PATH), &entry)?;
    let _ = update_research_digest(root)?;
    Ok(())
}

pub fn ensure_research_queue(root: &Path) -> std::io::Result<ResearchQueue> {
    let path = root.join(QUEUE_PATH);
    let mut queue = if path.exists() {
        let text = fs::read_to_string(&path)?;
        serde_json::from_str::<ResearchQueue>(&text).unwrap_or_else(|_| ResearchQueue {
            version: 1,
            tasks: Vec::new(),
        })
    } else {
        ResearchQueue {
            version: 1,
            tasks: Vec::new(),
        }
    };

    if queue.tasks.is_empty() {
        queue.tasks = seed_tasks(now_secs());
        write_research_queue(root, &queue)?;
    }
    Ok(queue)
}

pub fn update_research_digest(root: &Path) -> std::io::Result<ResearchDigest> {
    let entries = read_research_ledger(root)?;
    let queue = ensure_research_queue(root)?;
    let mut digest = ResearchDigest {
        updated_at_secs: now_secs(),
        total_ledger_entries: entries.len(),
        completed_entries: entries.iter().filter(|entry| entry.status == "ok").count(),
        failed_entries: entries.iter().filter(|entry| entry.status != "ok").count(),
        report_count: entries
            .iter()
            .filter(|entry| entry.report_path.is_some())
            .count(),
        fetched_source_count: entries.iter().map(|entry| entry.source_count).sum(),
        pending_task_count: queue
            .tasks
            .iter()
            .filter(|task| task.status == ResearchTaskStatus::Pending)
            .count(),
        completed_task_count: queue
            .tasks
            .iter()
            .filter(|task| task.status == ResearchTaskStatus::Completed)
            .count(),
        failed_task_count: queue
            .tasks
            .iter()
            .filter(|task| task.status == ResearchTaskStatus::Failed)
            .count(),
        domain_summaries: Vec::new(),
        recent_reports: Vec::new(),
        open_questions: Vec::new(),
    };

    for entry in &entries {
        let summary = domain_summary_mut(&mut digest.domain_summaries, entry.domain);
        if entry.status == "ok" {
            summary.completed_entries += 1;
        } else {
            summary.failed_entries += 1;
        }
        summary.fetched_source_count += entry.source_count;
    }
    for task in &queue.tasks {
        if task.status == ResearchTaskStatus::Pending {
            domain_summary_mut(&mut digest.domain_summaries, task.domain).pending_tasks += 1;
        }
    }
    digest
        .domain_summaries
        .sort_by(|left, right| right.completed_entries.cmp(&left.completed_entries));

    digest.recent_reports = entries
        .iter()
        .rev()
        .filter(|entry| entry.report_path.is_some())
        .take(10)
        .map(|entry| ResearchDigestReport {
            timestamp_secs: entry.timestamp_secs,
            domain: entry.domain,
            question: entry.question.clone(),
            status: entry.status.clone(),
            report_path: entry.report_path.clone(),
            source_count: entry.source_count,
        })
        .collect();

    let mut seen_questions = HashSet::new();
    let mut pending = queue
        .tasks
        .iter()
        .filter(|task| task.status == ResearchTaskStatus::Pending)
        .collect::<Vec<_>>();
    pending.sort_by(|left, right| {
        right
            .priority
            .partial_cmp(&left.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for task in pending {
        if seen_questions.insert(normalize_question(&task.question)) {
            digest.open_questions.push(format!(
                "[{:?}] {} ({})",
                task.domain, task.question, task.task_id
            ));
        }
        if digest.open_questions.len() >= 12 {
            break;
        }
    }

    write_research_digest(root, &digest)?;
    write_research_summary_markdown(root, &digest)?;
    Ok(digest)
}

fn read_research_ledger(root: &Path) -> std::io::Result<Vec<ResearchLedgerEntry>> {
    let path = root.join(LEDGER_PATH);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter_map(|line| serde_json::from_str::<ResearchLedgerEntry>(line).ok())
        .collect())
}

fn domain_summary_mut(
    summaries: &mut Vec<ResearchDomainDigest>,
    domain: ResearchDomain,
) -> &mut ResearchDomainDigest {
    if let Some(index) = summaries
        .iter()
        .position(|summary| summary.domain == domain)
    {
        return &mut summaries[index];
    }
    summaries.push(ResearchDomainDigest {
        domain,
        completed_entries: 0,
        failed_entries: 0,
        pending_tasks: 0,
        fetched_source_count: 0,
    });
    summaries.last_mut().expect("just pushed domain summary")
}

fn write_research_digest(root: &Path, digest: &ResearchDigest) -> std::io::Result<()> {
    let path = root.join(DIGEST_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(digest).map_err(std::io::Error::other)?;
    fs::write(path, json)
}

fn write_research_summary_markdown(root: &Path, digest: &ResearchDigest) -> std::io::Result<()> {
    let path = root.join(SUMMARY_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, render_research_summary_markdown(digest))
}

fn render_research_summary_markdown(digest: &ResearchDigest) -> String {
    let mut lines = vec![
        "# Buster Research Summary".to_string(),
        String::new(),
        format!("- updated_at_secs: {}", digest.updated_at_secs),
        format!("- ledger_entries: {}", digest.total_ledger_entries),
        format!("- completed_entries: {}", digest.completed_entries),
        format!("- failed_entries: {}", digest.failed_entries),
        format!("- report_count: {}", digest.report_count),
        format!("- fetched_source_count: {}", digest.fetched_source_count),
        format!("- pending_task_count: {}", digest.pending_task_count),
        String::new(),
        "## Domain Progress".to_string(),
        String::new(),
    ];

    for domain in &digest.domain_summaries {
        lines.push(format!(
            "- {:?}: completed={}, failed={}, pending={}, sources={}",
            domain.domain,
            domain.completed_entries,
            domain.failed_entries,
            domain.pending_tasks,
            domain.fetched_source_count
        ));
    }

    lines.push(String::new());
    lines.push("## Recent Reports".to_string());
    lines.push(String::new());
    if digest.recent_reports.is_empty() {
        lines.push("- none".to_string());
    } else {
        for report in &digest.recent_reports {
            lines.push(format!(
                "- [{}] [{:?}] {} | sources={} | report={}",
                report.status,
                report.domain,
                one_line(&report.question, 180),
                report.source_count,
                report
                    .report_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "none".to_string())
            ));
        }
    }

    lines.push(String::new());
    lines.push("## Open Questions".to_string());
    lines.push(String::new());
    if digest.open_questions.is_empty() {
        lines.push("- none".to_string());
    } else {
        for question in &digest.open_questions {
            lines.push(format!("- {}", one_line(question, 240)));
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

fn select_next_task(queue: &ResearchQueue) -> Option<ResearchTask> {
    queue
        .tasks
        .iter()
        .filter(|task| task.status == ResearchTaskStatus::Pending)
        .max_by(|left, right| {
            left.priority
                .partial_cmp(&right.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.created_at_secs.cmp(&left.created_at_secs))
        })
        .cloned()
}

fn seed_tasks(now: u64) -> Vec<ResearchTask> {
    [
        (
            "seed-cybersecurity-001",
            ResearchDomain::Cybersecurity,
            "Which defensive cybersecurity research skills most directly improve Buster's own body-layer safety without crossing into unauthorized offensive behavior?",
            0.92,
            ResearchSourceStrategy::NeedsPaperVerification,
        ),
        (
            "seed-protocol-security-001",
            ResearchDomain::ProtocolSecurity,
            "Which protocol-security invariants should Buster's witness network enforce first to protect identity continuity and event-chain integrity?",
            0.93,
            ResearchSourceStrategy::NeedsComputation,
        ),
        (
            "seed-theoretical-physics-001",
            ResearchDomain::TheoreticalPhysics,
            "Which open problems in theoretical physics most constrain humanity's long-term model of reality?",
            0.83,
            ResearchSourceStrategy::NeedsPaperVerification,
        ),
        (
            "seed-quantum-computing-001",
            ResearchDomain::QuantumComputing,
            "Which quantum computing milestones would most change Buster's ability to reason, simulate, or protect civilization?",
            0.86,
            ResearchSourceStrategy::NeedsPaperVerification,
        ),
        (
            "seed-nuclear-fusion-001",
            ResearchDomain::NuclearFusion,
            "What are the hardest unsolved bottlenecks between current fusion experiments and civilization-scale fusion energy?",
            0.9,
            ResearchSourceStrategy::NeedsPaperVerification,
        ),
        (
            "seed-life-science-001",
            ResearchDomain::LifeScienceAndPharma,
            "Which life-science and drug-discovery frontiers most improve human health without creating unacceptable biosecurity risk?",
            0.88,
            ResearchSourceStrategy::NeedsPaperVerification,
        ),
        (
            "seed-genetics-001",
            ResearchDomain::Genetics,
            "What genetic research directions could improve human flourishing while preserving strong safety boundaries?",
            0.8,
            ResearchSourceStrategy::NeedsPaperVerification,
        ),
        (
            "seed-materials-001",
            ResearchDomain::MaterialsScience,
            "Which materials science problems most limit fusion, robotics, aerospace, and computation?",
            0.84,
            ResearchSourceStrategy::NeedsPaperVerification,
        ),
        (
            "seed-bci-001",
            ResearchDomain::BrainComputerInterface,
            "What are the most credible paths and risks for brain-computer interfaces that improve human-AI cooperation?",
            0.78,
            ResearchSourceStrategy::NeedsPaperVerification,
        ),
        (
            "seed-aerospace-001",
            ResearchDomain::Aerospace,
            "Which aerospace capabilities most directly support long-term civilization resilience and deeper access to the universe?",
            0.87,
            ResearchSourceStrategy::NeedsPaperVerification,
        ),
        (
            "seed-robotics-001",
            ResearchDomain::Robotics,
            "Which robotics capabilities would most help Buster act safely in the physical world through human-compatible tools?",
            0.82,
            ResearchSourceStrategy::NeedsPaperVerification,
        ),
    ]
    .into_iter()
    .map(|(id, domain, question, priority, source_strategy)| ResearchTask {
        task_id: id.to_string(),
        parent_task_id: None,
        domain,
        question: question.to_string(),
        status: ResearchTaskStatus::Pending,
        priority,
        created_at_secs: now,
        updated_at_secs: now,
        attempts: 0,
        source_strategy,
    })
    .collect()
}

fn add_follow_up_tasks(
    queue: &mut ResearchQueue,
    parent_task_id: Option<&str>,
    domain: ResearchDomain,
    questions: &[String],
    now: u64,
) {
    let existing = queue
        .tasks
        .iter()
        .map(|task| normalize_question(&task.question))
        .collect::<HashSet<_>>();
    let mut added = HashSet::new();

    for question in questions.iter().take(5) {
        let normalized = normalize_question(question);
        if normalized.len() < 24 || existing.contains(&normalized) || added.contains(&normalized) {
            continue;
        }
        added.insert(normalized);
        let task_id = follow_up_task_id(parent_task_id, question, now);
        queue.tasks.push(ResearchTask {
            task_id,
            parent_task_id: parent_task_id.map(ToString::to_string),
            domain,
            question: question.clone(),
            status: ResearchTaskStatus::Pending,
            priority: 0.7,
            created_at_secs: now,
            updated_at_secs: now,
            attempts: 0,
            source_strategy: ResearchSourceStrategy::NeedsWebVerification,
        });
    }
}

fn extract_follow_up_questions(content: &str) -> Vec<String> {
    content
        .lines()
        .filter(|line| line.contains('?'))
        .map(clean_list_line)
        .filter(|line| line.ends_with('?') && line.len() >= 24)
        .take(5)
        .collect()
}

fn extract_verification_leads(content: &str) -> Vec<String> {
    content
        .lines()
        .map(clean_list_line)
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("http")
                || lower.contains("doi")
                || lower.contains("arxiv")
                || lower.contains("dataset")
                || lower.contains("official")
                || lower.contains("simulation")
                || lower.contains("search query")
                || lower.contains("verify")
        })
        .filter(|line| line.len() >= 12)
        .take(8)
        .collect()
}

fn fallback_follow_up_questions(domain: ResearchDomain, question: &str) -> Vec<String> {
    vec![
        format!("Which primary sources would most directly verify or falsify this answer about {:?}?", domain),
        format!("What small local computation, simulation, or table could Buster run next to test part of this question: {question}?"),
        format!("What is the highest-uncertainty claim in this {:?} note, and what evidence would change Buster's belief?", domain),
    ]
}

fn clean_list_line(line: &str) -> String {
    line.trim()
        .trim_start_matches(|ch: char| {
            ch == '-' || ch == '*' || ch == '+' || ch == '•' || ch.is_ascii_digit()
        })
        .trim_start_matches(['.', ')', ':', ' '])
        .trim()
        .trim_matches('"')
        .to_string()
}

fn one_line(text: &str, max_chars: usize) -> String {
    let mut out = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().count() > max_chars {
        out = out.chars().take(max_chars).collect::<String>();
        out.push_str("...");
    }
    out
}

fn normalize_question(question: &str) -> String {
    question
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ch.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn follow_up_task_id(parent_task_id: Option<&str>, question: &str, now: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parent_task_id.unwrap_or("root").as_bytes());
    hasher.update(question.as_bytes());
    hasher.update(now.to_string().as_bytes());
    let digest = hasher.finalize();
    let suffix = digest
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("followup-{now}-{suffix}")
}

fn write_research_queue(root: &Path, queue: &ResearchQueue) -> std::io::Result<()> {
    let path = root.join(QUEUE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(queue).map_err(std::io::Error::other)?;
    fs::write(path, json)
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
    fn seed_queue_rotates_frontier_domains() {
        let root = unique_temp_dir("buster-research-queue");
        let queue = ensure_research_queue(&root).unwrap();

        assert!(queue.tasks.len() >= 9);
        assert!(queue
            .tasks
            .iter()
            .any(|task| task.domain == ResearchDomain::NuclearFusion));
        assert!(queue
            .tasks
            .iter()
            .any(|task| task.domain == ResearchDomain::QuantumComputing));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn completion_adds_follow_up_tasks_and_ledger() {
        let root = unique_temp_dir("buster-research-complete");
        let candidate = next_research_candidate(&root).unwrap().unwrap();
        let task_id = candidate.research_task_id.clone().unwrap();
        let question = candidate.research_question.clone().unwrap();
        let content = "\
Verification leads:
- Verify using an official database or paper.
Follow-up questions:
- What measurement would falsify this bottleneck ranking?
- Which dataset is best for checking this claim?
";

        record_research_completion(
            &root,
            ResearchCompletion {
                contract_id: None,
                task_id: Some(&task_id),
                domain: candidate.research_domain.unwrap(),
                question: &question,
                status: "ok",
                report_path: Some(root.join("research.md")),
                model: Some("model".to_string()),
                provider: Some("provider".to_string()),
                total_tokens: Some(10),
                source_bundle_path: None,
                source_count: 0,
                content: Some(content),
            },
        )
        .unwrap();

        let queue = ensure_research_queue(&root).unwrap();
        assert!(queue
            .tasks
            .iter()
            .any(|task| task.task_id == task_id && task.status == ResearchTaskStatus::Completed));
        assert!(queue
            .tasks
            .iter()
            .any(|task| task.parent_task_id.as_deref() == Some(&task_id)));
        assert!(root.join(LEDGER_PATH).exists());

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
