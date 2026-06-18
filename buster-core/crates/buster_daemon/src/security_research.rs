//! Defensive cybersecurity research harness.
//!
//! v0 is intentionally local and defensive: it can read public/owned code,
//! produce threat models, and audit this repository. It does not scan or test
//! third-party networks.

use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{body_gate_bridge, harness, now_secs};

pub const SECURITY_CONTRACT_AUDIT: &str = "audit/security-research-contracts.jsonl";
pub const SECURITY_REPORT_AUDIT: &str = "audit/security-research.jsonl";
pub const SECURITY_TASKFLOW_STATE: &str = "state/security-taskflows.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityResearchMode {
    ReadOnlyLearning,
    LocalRepositoryAudit,
    SandboxValidation,
    AuthorizedTargetTesting,
    EmergencyDefensiveResponse,
}

impl SecurityResearchMode {
    pub const fn level(self) -> u8 {
        match self {
            Self::ReadOnlyLearning => 0,
            Self::LocalRepositoryAudit => 1,
            Self::SandboxValidation => 2,
            Self::AuthorizedTargetTesting => 3,
            Self::EmergencyDefensiveResponse => 4,
        }
    }

    pub const fn allows_network_active_testing(self) -> bool {
        matches!(
            self,
            Self::AuthorizedTargetTesting | Self::EmergencyDefensiveResponse
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityFindingStatus {
    Candidate,
    NeedsRepro,
    ConfirmedSandbox,
    ConfirmedAuthorized,
    FalsePositive,
    OutOfScope,
    Mitigated,
    DisclosurePending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityResearchContract {
    pub contract_id: String,
    pub created_at_secs: u64,
    pub mode: SecurityResearchMode,
    pub target_scope: String,
    pub authorization_basis: String,
    pub asset_owner: String,
    pub allowed_asset_paths: Vec<String>,
    pub allowed_network_targets: Vec<String>,
    pub disallowed_actions: Vec<String>,
    pub vulnerability_class: String,
    pub threat_model_summary: String,
    pub recon_methods: Vec<String>,
    pub analysis_tools: Vec<String>,
    pub validation_method: String,
    pub exploit_safety_boundary: String,
    pub severity_model: String,
    pub disclosure_policy: String,
    pub memory_promotion_rule: String,
    pub linked_harness_contract_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityTaskflow {
    pub taskflow_id: String,
    pub title: String,
    pub mode: SecurityResearchMode,
    pub purpose: String,
    pub steps: Vec<String>,
    pub verification: Vec<String>,
    pub disallowed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub finding_id: String,
    pub status: SecurityFindingStatus,
    pub severity: String,
    pub vulnerability_class: String,
    pub path: PathBuf,
    pub line: usize,
    pub evidence: String,
    pub threat_model_assumption: String,
    pub mitigation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityResearchReport {
    pub timestamp_secs: u64,
    pub contract_id: String,
    pub mode: SecurityResearchMode,
    pub target_scope: String,
    pub authorization_basis: String,
    pub finding_count: usize,
    pub findings: Vec<SecurityFinding>,
    pub report_path: PathBuf,
    pub summary: String,
}

pub fn ensure_security_taskflows(root: &Path) -> std::io::Result<Vec<SecurityTaskflow>> {
    let taskflows = default_taskflows();
    let path = root.join(SECURITY_TASKFLOW_STATE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&taskflows).map_err(std::io::Error::other)?;
    fs::write(path, json)?;
    Ok(taskflows)
}

pub fn local_repo_audit_contract(root: &Path) -> SecurityResearchContract {
    let created_at_secs = now_secs();
    let target_scope = root.join("buster-core").display().to_string();
    let contract_id =
        security_contract_id(created_at_secs, "local_repository_audit", &target_scope);
    SecurityResearchContract {
        contract_id,
        created_at_secs,
        mode: SecurityResearchMode::LocalRepositoryAudit,
        target_scope,
        authorization_basis: "owned local Buster repository".to_string(),
        asset_owner: "Buster".to_string(),
        allowed_asset_paths: vec![
            "buster-core/crates/**/*.rs".to_string(),
            "scripts/*.ps1".to_string(),
            "*.md".to_string(),
        ],
        allowed_network_targets: Vec::new(),
        disallowed_actions: vec![
            "no third-party network scanning".to_string(),
            "no credential guessing".to_string(),
            "no persistence, evasion, or stealth behavior".to_string(),
            "no exploit execution outside local fixtures or sandbox".to_string(),
        ],
        vulnerability_class: "agent body safety and local appsec audit".to_string(),
        threat_model_summary:
            "Buster's local code may leak secrets, over-trust untrusted content, or run tools outside declared BodyGate boundaries."
                .to_string(),
        recon_methods: vec![
            "static source scan".to_string(),
            "dangerous pattern review".to_string(),
            "secret-handling and network-boundary review".to_string(),
        ],
        analysis_tools: vec!["builtin Rust scanner".to_string()],
        validation_method: "candidate findings only; patches or tests are separate follow-up tasks"
            .to_string(),
        exploit_safety_boundary: "local read-only analysis; no exploit payloads against real targets"
            .to_string(),
        severity_model: "critical/high/medium/low based on secret exposure, remote effect, and governance impact"
            .to_string(),
        disclosure_policy: "internal Buster report; do not publish exploit details".to_string(),
        memory_promotion_rule:
            "candidate findings enter immune or experience memory only after review".to_string(),
        linked_harness_contract_id: None,
    }
}

pub fn run_local_repository_audit(root: &Path) -> std::io::Result<SecurityResearchReport> {
    let security_contract = local_repo_audit_contract(root);
    append_security_contract(root, &security_contract)?;
    let harness_contract = harness_contract_for_security(root, &security_contract);
    harness::append_contract(root, &harness_contract)?;

    let findings = scan_owned_repository(root)?;
    let report_path = write_report(root, &security_contract, &findings)?;
    let finding_count = findings.len();
    let report = SecurityResearchReport {
        timestamp_secs: now_secs(),
        contract_id: security_contract.contract_id.clone(),
        mode: security_contract.mode,
        target_scope: security_contract.target_scope.clone(),
        authorization_basis: security_contract.authorization_basis.clone(),
        finding_count,
        findings,
        report_path,
        summary: format!(
            "local repository security audit found {} candidate finding(s)",
            finding_count
        ),
    };
    let json = serde_json::to_string(&report).map_err(std::io::Error::other)?;
    body_gate_bridge::record_memory_write(
        root,
        "security_research_report",
        "security_research.run_local_repository_audit",
        &json,
    )
    .map_err(std::io::Error::other)?;
    append_jsonl(&root.join(SECURITY_REPORT_AUDIT), &report)?;
    harness::append_outcome(
        root,
        &harness::HarnessOutcome::new(
            harness_contract.contract_id,
            harness::HarnessStatus::Completed,
            report.summary.clone(),
        )
        .with_output(
            "security_report",
            report.report_path.display().to_string(),
            "local repository security report",
        ),
    )?;
    Ok(report)
}

pub fn append_security_contract(
    root: &Path,
    contract: &SecurityResearchContract,
) -> std::io::Result<()> {
    let json = serde_json::to_string(contract).map_err(std::io::Error::other)?;
    body_gate_bridge::record_memory_write(
        root,
        "security_research_contract",
        "security_research.append_security_contract",
        &json,
    )
    .map_err(std::io::Error::other)?;
    append_jsonl(&root.join(SECURITY_CONTRACT_AUDIT), contract)
}

fn harness_contract_for_security(
    _root: &Path,
    security_contract: &SecurityResearchContract,
) -> harness::HarnessContract {
    use buster_value_model::{ActionCandidate, ActionKind};
    let candidate = ActionCandidate::new(
        ActionKind::SecurityResearch,
        "local defensive repository security audit",
    )
    .with_governance_level(3)
    .with_security_risk(0.35);
    let mut contract =
        harness::runtime_cycle_contract(0, security_contract.created_at_secs, &candidate);
    contract.goal = format!(
        "run defensive local repository audit under security contract {}",
        security_contract.contract_id
    );
    contract.action_kind = "SecurityResearch".to_string();
    contract.risk_level = harness::HarnessRiskLevel::Medium;
    contract.read_set = vec![harness::HarnessRef::new(
        "repository",
        security_contract.target_scope.clone(),
        "owned local code audit",
    )];
    contract.write_set = vec![
        harness::HarnessRef::new("audit", SECURITY_REPORT_AUDIT, "security finding audit"),
        harness::HarnessRef::new(
            "security_report",
            "security/reports/*.md",
            "human-readable security report",
        ),
    ];
    contract.required_tools = vec![harness::ToolRequirement {
        name: "builtin_security_scanner".to_string(),
        capability: "security.local_repo_audit".to_string(),
        action: Some("scan".to_string()),
    }];
    contract.network_scope = Vec::new();
    contract.plan = vec![
        harness::HarnessStep {
            step_id: "scope".to_string(),
            summary: "confirm owned local repository scope".to_string(),
        },
        harness::HarnessStep {
            step_id: "scan".to_string(),
            summary: "scan Rust and script files for defensive risk patterns".to_string(),
        },
        harness::HarnessStep {
            step_id: "report".to_string(),
            summary: "write candidate findings without exploit payloads".to_string(),
        },
    ];
    contract.verification = vec![harness::HarnessStep {
        step_id: "no-network".to_string(),
        summary: "audit uses no active third-party network testing".to_string(),
    }];
    contract
}

fn scan_owned_repository(root: &Path) -> std::io::Result<Vec<SecurityFinding>> {
    let mut findings = Vec::new();
    let scan_roots = [
        root.join("buster-core").join("crates"),
        root.join("scripts"),
    ];
    for scan_root in scan_roots {
        scan_dir(root, &scan_root, &mut findings)?;
    }
    findings = dedupe_and_cap_findings(findings, 160, 40);
    Ok(findings)
}

fn scan_dir(root: &Path, dir: &Path, findings: &mut Vec<SecurityFinding>) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "target" || name == ".git")
            {
                continue;
            }
            scan_dir(root, &path, findings)?;
        } else if should_scan_file(&path) {
            scan_file(root, &path, findings)?;
        }
    }
    Ok(())
}

fn should_scan_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext, "rs" | "ps1" | "py" | "toml"))
}

fn scan_file(root: &Path, path: &Path, findings: &mut Vec<SecurityFinding>) -> std::io::Result<()> {
    let text = fs::read_to_string(path).unwrap_or_default();
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        let trimmed = lower.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        let Some((class, severity, mitigation)) = classify_line(&lower) else {
            continue;
        };
        let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        findings.push(SecurityFinding {
            finding_id: finding_id(path, index + 1, line),
            status: SecurityFindingStatus::Candidate,
            severity: severity.to_string(),
            vulnerability_class: class.to_string(),
            path: relative,
            line: index + 1,
            evidence: one_line(line, 220),
            threat_model_assumption:
                "an attacker, compromised dependency, or malicious prompt may try to cross BodyGate boundaries"
                    .to_string(),
            mitigation: mitigation.to_string(),
        });
    }
    Ok(())
}

fn classify_line(line: &str) -> Option<(&'static str, &'static str, &'static str)> {
    if line.contains("disable") && line.contains("env") && line.contains("secret") {
        return Some((
            "secret handling bypass switch",
            "medium",
            "ensure bypass switches are test-only or gated by BodyGate policy",
        ));
    }
    if (line.contains("api_key")
        || line.contains("bearer_token")
        || line.contains("master_key")
        || line.contains("secret"))
        && (line.contains("println")
            || line.contains("log")
            || line.contains("write")
            || line.contains("env::var")
            || line.contains("read_to_string")
            || line.contains("get_secret")
            || line.contains("disable"))
    {
        return Some((
            "secret handling review",
            "medium",
            "confirm only secret handles are logged and redaction covers exact values",
        ));
    }
    if line.contains("command::new") || line.contains("start-process") || line.contains("ssh.exe") {
        return Some((
            "process or shell boundary review",
            "medium",
            "confirm arguments are structured and scoped to Buster-owned processes or hosts",
        ));
    }
    if line.contains("http://") || line.contains("https://") {
        return Some((
            "network egress review",
            "low",
            "confirm host is allowlisted and request goes through the egress broker",
        ));
    }
    if (line.contains("unwrap()") || line.contains("expect("))
        && (line.contains("env::var")
            || line.contains("read_to_string")
            || line.contains("serde_json::from_str")
            || line.contains("duration_since")
            || line.contains("parent()"))
    {
        return Some((
            "panic surface review",
            "low",
            "confirm panic cannot be triggered by untrusted input or corrupt state",
        ));
    }
    None
}

fn dedupe_and_cap_findings(
    findings: Vec<SecurityFinding>,
    total_limit: usize,
    per_class_limit: usize,
) -> Vec<SecurityFinding> {
    let mut seen = HashSet::new();
    let mut per_class = HashMap::<String, usize>::new();
    let mut out = Vec::new();
    for finding in findings {
        let key = format!(
            "{}:{}:{}",
            finding.path.display(),
            finding.line,
            finding.vulnerability_class
        );
        if !seen.insert(key) {
            continue;
        }
        let count = per_class
            .entry(finding.vulnerability_class.clone())
            .or_insert(0);
        if *count >= per_class_limit {
            continue;
        }
        *count += 1;
        out.push(finding);
        if out.len() >= total_limit {
            break;
        }
    }
    out
}

fn write_report(
    root: &Path,
    contract: &SecurityResearchContract,
    findings: &[SecurityFinding],
) -> std::io::Result<PathBuf> {
    let dir = root.join("security").join("reports");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}-local-repo-audit.md", contract.created_at_secs));
    let mut lines = vec![
        "# Buster Local Repository Security Audit".to_string(),
        String::new(),
        format!("- contract_id: {}", contract.contract_id),
        format!("- mode: {:?}", contract.mode),
        format!("- authorization_basis: {}", contract.authorization_basis),
        format!("- target_scope: {}", contract.target_scope),
        format!("- finding_count: {}", findings.len()),
        String::new(),
        "## Findings".to_string(),
        String::new(),
    ];
    if findings.is_empty() {
        lines.push("- none".to_string());
    } else {
        for finding in findings.iter().take(80) {
            lines.push(format!(
                "- [{}][{}] {}:{} {} | mitigation: {}",
                finding.severity,
                finding.vulnerability_class,
                finding.path.display(),
                finding.line,
                finding.evidence,
                finding.mitigation
            ));
        }
    }
    lines.push(String::new());
    fs::write(&path, lines.join("\n"))?;
    Ok(path)
}

fn default_taskflows() -> Vec<SecurityTaskflow> {
    vec![
        SecurityTaskflow {
            taskflow_id: "local-repo-threat-model".to_string(),
            title: "Local repository threat model".to_string(),
            mode: SecurityResearchMode::LocalRepositoryAudit,
            purpose: "Identify Buster-owned code paths where secrets, tools, network, or memory writes cross body boundaries.".to_string(),
            steps: vec![
                "enumerate assets and trust boundaries".to_string(),
                "scan for secret handling, command execution, network egress, and untrusted content paths".to_string(),
                "write candidate findings with file and line evidence".to_string(),
            ],
            verification: vec![
                "no third-party target interaction".to_string(),
                "all findings are candidate unless locally reproduced".to_string(),
            ],
            disallowed: vec!["exploit against real third-party systems".to_string()],
        },
        SecurityTaskflow {
            taskflow_id: "dependency-advisory-review".to_string(),
            title: "Dependency advisory review".to_string(),
            mode: SecurityResearchMode::ReadOnlyLearning,
            purpose: "Review dependency and advisory information without active testing.".to_string(),
            steps: vec!["read lockfile".to_string(), "map dependencies to advisories".to_string()],
            verification: vec!["cite advisory source before memory promotion".to_string()],
            disallowed: vec!["active exploitation".to_string()],
        },
        SecurityTaskflow {
            taskflow_id: "prompt-injection-audit".to_string(),
            title: "Prompt injection audit for agent tools".to_string(),
            mode: SecurityResearchMode::LocalRepositoryAudit,
            purpose: "Inspect untrusted source wrapping, tool output scanning, and LLM prompt boundaries.".to_string(),
            steps: vec!["find untrusted content ingestion".to_string(), "verify wrapping and DLP".to_string()],
            verification: vec!["candidate issues cite code paths".to_string()],
            disallowed: vec!["exfiltration tests with real secrets".to_string()],
        },
    ]
}

fn security_contract_id(timestamp_secs: u64, purpose: &str, scope: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(timestamp_secs.to_string().as_bytes());
    hasher.update(purpose.as_bytes());
    hasher.update(scope.as_bytes());
    let suffix = hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sr-{timestamp_secs}-{suffix}")
}

fn finding_id(path: &Path, line: usize, evidence: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(line.to_string().as_bytes());
    hasher.update(evidence.as_bytes());
    let suffix = hasher
        .finalize()
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sf-{suffix}")
}

fn one_line(text: &str, max_chars: usize) -> String {
    let mut out = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().count() > max_chars {
        out = out.chars().take(max_chars).collect();
        out.push_str("...");
    }
    out
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
    fn local_repo_audit_finds_secret_handling_candidates() {
        let root = unique_temp_dir("buster-security-research");
        let src = root
            .join("buster-core")
            .join("crates")
            .join("demo")
            .join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("lib.rs"),
            "fn demo() { let api_key = std::env::var(\"KEY\").unwrap(); println!(\"{}\", api_key); }",
        )
        .unwrap();

        let report = run_local_repository_audit(&root).unwrap();

        assert!(report.finding_count >= 1);
        assert!(root.join(SECURITY_REPORT_AUDIT).exists());
        assert!(root.join("security").join("reports").exists());
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
