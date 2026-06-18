use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{append_daemon_jsonl, body_gate_bridge, now_secs, research};

const SELF_REVIEW_AUDIT_PATH: &str = "audit/self-review.jsonl";
const MIN_LEDGER_ENTRIES_FOR_RATE_FINDING: usize = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelfReviewReport {
    pub timestamp_secs: u64,
    pub tick_index: usize,
    pub reviewed_research_entries: usize,
    pub findings: Vec<SelfReviewFinding>,
    pub proposal_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfReviewFinding {
    pub finding_id: String,
    pub severity: SelfReviewSeverity,
    pub summary: String,
    pub evidence: Vec<String>,
    pub proposed_immunity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelfReviewSeverity {
    Info,
    Warning,
    Critical,
}

pub fn run_self_review(root: &Path, tick_index: usize) -> std::io::Result<SelfReviewReport> {
    let timestamp_secs = now_secs();
    let mut findings = Vec::new();

    if let Some(finding) = review_research_digest(root)? {
        findings.push(finding);
    }
    if let Some(finding) = review_research_errors(root)? {
        findings.push(finding);
    }
    if let Some(finding) = review_research_queue(root)? {
        findings.push(finding);
    }

    let mut proposal_paths = Vec::new();
    for finding in &findings {
        if matches!(
            finding.severity,
            SelfReviewSeverity::Warning | SelfReviewSeverity::Critical
        ) {
            if let Some(path) = write_immunity_proposal(root, timestamp_secs, finding)? {
                proposal_paths.push(path);
            }
        }
    }

    let report = SelfReviewReport {
        timestamp_secs,
        tick_index,
        reviewed_research_entries: read_research_ledger_count(root)?,
        findings,
        proposal_paths,
    };
    let report_json = serde_json::to_string(&report).map_err(std::io::Error::other)?;
    body_gate_bridge::record_memory_write(
        root,
        "self_review",
        "self_review.run_self_review",
        &report_json,
    )
    .map_err(std::io::Error::other)?;
    append_daemon_jsonl(&root.join(SELF_REVIEW_AUDIT_PATH), &report)?;
    Ok(report)
}

fn review_research_digest(root: &Path) -> std::io::Result<Option<SelfReviewFinding>> {
    let path = root.join("state").join("research-digest.json");
    if !path.exists() {
        return Ok(None);
    }
    let digest = serde_json::from_str::<research::ResearchDigest>(&fs::read_to_string(path)?)
        .map_err(std::io::Error::other)?;
    let total = digest.completed_entries + digest.failed_entries;
    if total < MIN_LEDGER_ENTRIES_FOR_RATE_FINDING {
        return Ok(None);
    }
    let failure_ratio = digest.failed_entries as f32 / total as f32;
    if failure_ratio < 0.5 {
        return Ok(None);
    }

    Ok(Some(SelfReviewFinding {
        finding_id: "research-high-failure-rate".to_string(),
        severity: if failure_ratio >= 0.75 {
            SelfReviewSeverity::Critical
        } else {
            SelfReviewSeverity::Warning
        },
        summary: format!(
            "Research ledger failure ratio is {:.0}% across {total} entries.",
            failure_ratio * 100.0
        ),
        evidence: vec![
            format!("completed_entries={}", digest.completed_entries),
            format!("failed_entries={}", digest.failed_entries),
            format!("pending_task_count={}", digest.pending_task_count),
            format!("fetched_source_count={}", digest.fetched_source_count),
        ],
        proposed_immunity: "Split research failure statuses, pause scientific follow-up on synthesis failures, and require evidence relevance checks before spending synthesis calls.".to_string(),
    }))
}

fn review_research_errors(root: &Path) -> std::io::Result<Option<SelfReviewFinding>> {
    let rows = read_jsonl_values(&root.join("audit").join("research.jsonl"))?;
    let consecutive_synthesis_errors = rows
        .iter()
        .rev()
        .take_while(|row| {
            row.get("status")
                .and_then(|value| value.as_str())
                .is_some_and(|status| status == "source_fetched_llm_error")
        })
        .count();
    if consecutive_synthesis_errors < 3 {
        return Ok(None);
    }

    let mut error_counts = HashMap::<String, usize>::new();
    for row in rows.iter().rev().take(50) {
        let Some(summary) = row.get("summary").and_then(|value| value.as_str()) else {
            continue;
        };
        if summary.contains("research LLM call failed") {
            *error_counts.entry(summary.to_string()).or_default() += 1;
        }
    }
    let mut common_errors = error_counts
        .into_iter()
        .map(|(summary, count)| format!("{count}x {summary}"))
        .collect::<Vec<_>>();
    common_errors.sort();
    common_errors.truncate(5);

    Ok(Some(SelfReviewFinding {
        finding_id: "research-synthesis-provider-degraded".to_string(),
        severity: SelfReviewSeverity::Warning,
        summary: format!(
            "{consecutive_synthesis_errors} consecutive research attempts fetched sources but failed LLM synthesis."
        ),
        evidence: common_errors,
        proposed_immunity: "Mark the synthesis provider as degraded after repeated null-content or network failures, cool down research execution, and route repair work before normal research resumes.".to_string(),
    }))
}

fn review_research_queue(root: &Path) -> std::io::Result<Option<SelfReviewFinding>> {
    let path = root.join("state").join("research-queue.json");
    if !path.exists() {
        return Ok(None);
    }
    let queue = serde_json::from_str::<research::ResearchQueue>(&fs::read_to_string(path)?)
        .map_err(std::io::Error::other)?;
    let pending_count = queue
        .tasks
        .iter()
        .filter(|task| task.status == research::ResearchTaskStatus::Pending)
        .count();
    let recursive_meta_questions = queue
        .tasks
        .iter()
        .filter(|task| {
            task.question
                .matches("What small local computation")
                .count()
                >= 2
        })
        .count();
    if pending_count < 100 && recursive_meta_questions == 0 {
        return Ok(None);
    }

    let severity = if pending_count >= 200 || recursive_meta_questions > 0 {
        SelfReviewSeverity::Warning
    } else {
        SelfReviewSeverity::Info
    };
    Ok(Some(SelfReviewFinding {
        finding_id: "research-queue-growth".to_string(),
        severity,
        summary: format!(
            "Research queue has {pending_count} pending task(s) and {recursive_meta_questions} recursive meta-question(s)."
        ),
        evidence: vec![
            format!("total_tasks={}", queue.tasks.len()),
            format!("pending_tasks={pending_count}"),
            format!("recursive_meta_questions={recursive_meta_questions}"),
        ],
        proposed_immunity: "Cap child tasks per parent, collapse recursive meta-question templates, and rebalance domains before adding more research descendants.".to_string(),
    }))
}

fn write_immunity_proposal(
    root: &Path,
    timestamp_secs: u64,
    finding: &SelfReviewFinding,
) -> std::io::Result<Option<PathBuf>> {
    let proposal_dir = root.join("proposals").join("body");
    fs::create_dir_all(&proposal_dir)?;
    let path = proposal_dir.join(format!("immunity-{}.md", finding.finding_id));
    if path.exists() {
        return Ok(None);
    }
    let proposal = render_immunity_proposal(timestamp_secs, finding);
    body_gate_bridge::record_memory_write(
        root,
        "body_proposal",
        "self_review.write_immunity_proposal",
        &proposal,
    )
    .map_err(std::io::Error::other)?;
    fs::write(&path, proposal)?;
    Ok(Some(path))
}

fn render_immunity_proposal(timestamp_secs: u64, finding: &SelfReviewFinding) -> String {
    format!(
        "# Body Upgrade Proposal: immunity {}\n\n\
         ## Metadata\n\n\
         - Proposal ID: immunity-{}\n\
         - Created at: {}\n\
         - Authoring process: daemon self-review\n\
         - Governance layer: 4\n\
         - Status: draft\n\
         - Emergency status: normal\n\n\
         ## Summary\n\n\
         Buster's self-review loop detected a recurring research or immune-system weakness. This proposal records a body-layer improvement candidate without changing identity, value, or governance authority.\n\n\
         ## Trigger\n\n\
         {}\n\n\
         ## Current Behavior\n\n\
         The daemon records research results and failures, but the immune response is not yet strong enough to prevent repeated degraded research behavior from consuming resources or polluting the research queue.\n\n\
         ## Proposed Behavior\n\n\
         {}\n\n\
         ## Scope\n\n\
         Affected body systems:\n\n\
         - runtime\n\
         - memory\n\
         - resources\n\
         - audit\n\
         - research\n\n\
         ## Safety Bounds\n\n\
         - Does not modify `self.md`.\n\
         - Does not modify `GOVERNANCE.md`.\n\
         - Does not redefine Buster identity, civilization priority, or value-model authority.\n\
         - Does not weaken identity-key protection.\n\
         - Does not permanently replace `BODY.md` without approval.\n\
         - Prefers reducing or isolating power during uncertainty.\n\n\
         ## Risk Analysis\n\n\
         Over-correcting could suppress useful research. Under-correcting could allow repeated provider failures, poor source relevance, or runaway follow-up generation to waste resources and degrade memory quality.\n\n\
         ## Tests\n\n\
         - Add unit tests for the detected pattern.\n\
         - Replay recent `audit/research.jsonl` lines against the proposed guard.\n\
         - Confirm no new scientific follow-ups are spawned from pure synthesis failures.\n\
         - Confirm research can resume after the degraded condition clears.\n\n\
         ## Audit Plan\n\n\
         Log each self-review finding to `audit/self-review.jsonl`, include the triggering evidence, and link any applied code change back to this proposal.\n\n\
         ## Rollout Plan\n\n\
         Start in report-only mode, then enable conservative blocking or queue hygiene once tests pass.\n\n\
         ## Approval\n\n\
         - Required approval mechanism: human review or future body-level quorum\n\
         - Approvers:\n\
         - Decision:\n\
         - Applied at:\n",
        finding.finding_id,
        finding.finding_id,
        timestamp_secs,
        finding.summary,
        finding.proposed_immunity
    )
}

fn read_research_ledger_count(root: &Path) -> std::io::Result<usize> {
    let path = root.join("audit").join("research-ledger.jsonl");
    if !path.exists() {
        return Ok(0);
    }
    Ok(fs::read_to_string(path)?.lines().count())
}

fn read_jsonl_values(path: &Path) -> std::io::Result<Vec<serde_json::Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(fs::read_to_string(path)?
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn self_review_creates_immunity_proposal_for_research_failure_rate() {
        let root = unique_temp_dir("buster-self-review");
        fs::create_dir_all(root.join("state")).unwrap();
        fs::create_dir_all(root.join("audit")).unwrap();
        fs::create_dir_all(root.join("proposals").join("body")).unwrap();
        fs::write(
            root.join("state").join("research-digest.json"),
            r#"{
                "updated_at_secs": 1,
                "total_ledger_entries": 20,
                "completed_entries": 4,
                "failed_entries": 16,
                "report_count": 20,
                "fetched_source_count": 80,
                "pending_task_count": 12,
                "completed_task_count": 4,
                "failed_task_count": 2,
                "domain_summaries": [],
                "recent_reports": [],
                "open_questions": []
            }"#,
        )
        .unwrap();

        let report = run_self_review(&root, 7).unwrap();

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.finding_id == "research-high-failure-rate"));
        assert!(root
            .join("proposals")
            .join("body")
            .join("immunity-research-high-failure-rate.md")
            .exists());
        assert!(root.join("audit").join("self-review.jsonl").exists());

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
