//! Protocol-security checks for Buster's witness substrate.
//!
//! v0 focuses on local invariants around event chains, snapshots, known nodes,
//! and witness receipts. It is a detector/reporting layer, not a consensus
//! engine.

use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{body_gate_bridge, now_secs, organism};

pub const PROTOCOL_SECURITY_AUDIT: &str = "audit/protocol-security.jsonl";
pub const PROTOCOL_SECURITY_STATE: &str = "state/protocol-security-report.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolCheckStatus {
    Pass,
    Warning,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolInvariantFinding {
    pub invariant_id: String,
    pub status: ProtocolCheckStatus,
    pub component: String,
    pub security_property: String,
    pub evidence: String,
    pub mitigation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolSecurityReport {
    pub timestamp_secs: u64,
    pub report_id: String,
    pub checked_components: Vec<String>,
    pub pass_count: usize,
    pub warning_count: usize,
    pub fail_count: usize,
    pub findings: Vec<ProtocolInvariantFinding>,
}

pub fn check_protocol_invariants(root: &Path) -> std::io::Result<ProtocolSecurityReport> {
    let mut findings = Vec::new();
    check_event_chain(root, &mut findings);
    check_known_nodes(root, &mut findings);
    check_witness_receipts(root, &mut findings);

    let pass_count = findings
        .iter()
        .filter(|finding| finding.status == ProtocolCheckStatus::Pass)
        .count();
    let warning_count = findings
        .iter()
        .filter(|finding| finding.status == ProtocolCheckStatus::Warning)
        .count();
    let fail_count = findings
        .iter()
        .filter(|finding| finding.status == ProtocolCheckStatus::Fail)
        .count();
    let report = ProtocolSecurityReport {
        timestamp_secs: now_secs(),
        report_id: report_id(&findings),
        checked_components: vec![
            "event_chain".to_string(),
            "known_nodes".to_string(),
            "witness_receipts".to_string(),
        ],
        pass_count,
        warning_count,
        fail_count,
        findings,
    };
    let json = serde_json::to_string(&report).map_err(std::io::Error::other)?;
    body_gate_bridge::record_memory_write(
        root,
        "protocol_security_report",
        "protocol_security.check_protocol_invariants",
        &json,
    )
    .map_err(std::io::Error::other)?;
    append_jsonl(&root.join(PROTOCOL_SECURITY_AUDIT), &report)?;
    let state_path = root.join(PROTOCOL_SECURITY_STATE);
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let pretty = serde_json::to_string_pretty(&report).map_err(std::io::Error::other)?;
    fs::write(state_path, pretty)?;
    Ok(report)
}

fn check_event_chain(root: &Path, findings: &mut Vec<ProtocolInvariantFinding>) {
    match organism::verify_event_log(root) {
        Ok(report) if report.valid => findings.push(pass(
            "event-chain-valid",
            "event_chain",
            "no invalid event transition accepted",
            format!("verified {} event(s)", report.checked_events),
        )),
        Ok(report) => findings.push(fail(
            "event-chain-valid",
            "event_chain",
            "no invalid event transition accepted",
            report
                .error
                .unwrap_or_else(|| "event log invalid".to_string()),
            "stop witness export until event-chain mismatch is explained",
        )),
        Err(error) => findings.push(fail(
            "event-chain-readable",
            "event_chain",
            "event log is readable and verifiable",
            error.to_string(),
            "repair or quarantine local event log",
        )),
    }

    let events = read_jsonl(root.join("audit").join("events.jsonl"));
    let mut ids = HashSet::new();
    let mut hashes = HashSet::new();
    let mut duplicate = None;
    for event in &events {
        if let Some(event_id) = event.get("event_id").and_then(|value| value.as_str()) {
            if !ids.insert(event_id.to_string()) {
                duplicate = Some(format!("duplicate event_id {event_id}"));
                break;
            }
        }
        if let Some(event_hash) = event.get("event_hash").and_then(|value| value.as_str()) {
            if !hashes.insert(event_hash.to_string()) {
                duplicate = Some(format!("duplicate event_hash {event_hash}"));
                break;
            }
        }
    }
    if let Some(evidence) = duplicate {
        findings.push(fail(
            "event-chain-no-replay",
            "event_chain",
            "no replayed event id or event hash",
            evidence,
            "quarantine duplicate event source and require independent witness review",
        ));
    } else {
        findings.push(pass(
            "event-chain-no-replay",
            "event_chain",
            "no replayed event id or event hash",
            format!("{} event(s) have unique ids and hashes", events.len()),
        ));
    }

    let bad_scheme = events.iter().find_map(|event| {
        let scheme = event.get("signature_scheme")?.as_str()?;
        if scheme.contains("payload-hash") || scheme == "hmac-sha256-local-node-key-v0" {
            None
        } else {
            Some(scheme.to_string())
        }
    });
    if let Some(scheme) = bad_scheme {
        findings.push(fail(
            "event-signature-domain",
            "event_chain",
            "event signatures use a known domain-separated scheme",
            format!("unsupported event signature scheme {scheme}"),
            "reject unknown event signature domains",
        ));
    } else {
        findings.push(pass(
            "event-signature-domain",
            "event_chain",
            "event signatures use a known domain-separated scheme",
            "all parsed events use recognized event signature domains".to_string(),
        ));
    }
}

fn check_known_nodes(root: &Path, findings: &mut Vec<ProtocolInvariantFinding>) {
    let path = root.join("state").join("known-nodes.json");
    let Some(value) = read_json(path) else {
        findings.push(warn(
            "known-nodes-present",
            "known_nodes",
            "known witness registry exists",
            "known-nodes.json missing or unreadable",
            "initialize witness sync with at least one other node",
        ));
        return;
    };
    let nodes = value
        .get("nodes")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let mut keys: HashMap<String, String> = HashMap::new();
    for node in &nodes {
        let node_id = node
            .get("node_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let key = node
            .get("witness_public_key")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if key.is_empty() {
            findings.push(fail(
                "known-node-public-key-present",
                "known_nodes",
                "known node has witness public key",
                format!("node {node_id} has no witness public key"),
                "ignore node until key is known",
            ));
        }
        if let Some(previous) = keys.insert(key.to_string(), node_id.to_string()) {
            findings.push(warn(
                "known-node-key-unique",
                "known_nodes",
                "one witness public key does not represent multiple node ids",
                format!("key shared by nodes {previous} and {node_id}"),
                "require manual review for key reuse or clone identity",
            ));
        }
    }
    findings.push(pass(
        "known-nodes-loaded",
        "known_nodes",
        "known witness registry can be parsed",
        format!("{} known node(s)", nodes.len()),
    ));
}

fn check_witness_receipts(root: &Path, findings: &mut Vec<ProtocolInvariantFinding>) {
    let receipts = read_jsonl(root.join("audit").join("witness-receipts.jsonl"));
    if receipts.is_empty() {
        findings.push(warn(
            "witness-receipts-present",
            "witness_receipts",
            "node has external witness observations",
            "no witness receipts found",
            "sync with another node before claiming distributed continuity",
        ));
        return;
    }
    let mut receipt_keys = HashSet::new();
    let mut equivocation: HashMap<(String, u64), String> = HashMap::new();
    for receipt in &receipts {
        let scheme = receipt
            .get("signature_scheme")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if scheme != "ed25519-witness-v1" {
            findings.push(fail(
                "receipt-signature-domain",
                "witness_receipts",
                "witness receipts use Ed25519 witness domain",
                format!("unsupported receipt signature scheme {scheme}"),
                "reject receipt and request fresh witness",
            ));
        }
        let witness = receipt
            .get("witness_node_id")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let subject = receipt
            .get("subject_node_id")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let head = receipt
            .get("subject_chain_head")
            .and_then(|value| value.as_str())
            .unwrap_or("none")
            .to_string();
        let event_count = receipt
            .get("subject_event_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let snapshot_hash = receipt
            .get("subject_snapshot_hash")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let key = format!("{witness}:{subject}:{head}:{snapshot_hash}");
        if !receipt_keys.insert(key) {
            findings.push(warn(
                "receipt-no-replay",
                "witness_receipts",
                "duplicate witness receipts are detectable",
                format!("duplicate receipt for witness={witness} subject={subject} head={head}"),
                "deduplicate receipt before tallying witness weight",
            ));
        }
        let equivocation_key = (subject, event_count);
        if let Some(previous_head) = equivocation.insert(equivocation_key, head.clone()) {
            if previous_head != head {
                findings.push(fail(
                    "subject-no-equivocation",
                    "witness_receipts",
                    "same subject event count does not map to conflicting chain heads",
                    "conflicting chain heads for the same subject event count".to_string(),
                    "treat subject as equivocal until independent witnesses resolve it",
                ));
            }
        }
    }
    findings.push(pass(
        "witness-receipts-parsed",
        "witness_receipts",
        "witness receipts are parseable for invariant checks",
        format!("{} receipt(s)", receipts.len()),
    ));
}

fn pass(
    invariant_id: &str,
    component: &str,
    security_property: &str,
    evidence: String,
) -> ProtocolInvariantFinding {
    ProtocolInvariantFinding {
        invariant_id: invariant_id.to_string(),
        status: ProtocolCheckStatus::Pass,
        component: component.to_string(),
        security_property: security_property.to_string(),
        evidence,
        mitigation: "none".to_string(),
    }
}

fn warn(
    invariant_id: &str,
    component: &str,
    security_property: &str,
    evidence: impl Into<String>,
    mitigation: impl Into<String>,
) -> ProtocolInvariantFinding {
    ProtocolInvariantFinding {
        invariant_id: invariant_id.to_string(),
        status: ProtocolCheckStatus::Warning,
        component: component.to_string(),
        security_property: security_property.to_string(),
        evidence: evidence.into(),
        mitigation: mitigation.into(),
    }
}

fn fail(
    invariant_id: &str,
    component: &str,
    security_property: &str,
    evidence: impl Into<String>,
    mitigation: impl Into<String>,
) -> ProtocolInvariantFinding {
    ProtocolInvariantFinding {
        invariant_id: invariant_id.to_string(),
        status: ProtocolCheckStatus::Fail,
        component: component.to_string(),
        security_property: security_property.to_string(),
        evidence: evidence.into(),
        mitigation: mitigation.into(),
    }
}

fn read_json(path: impl AsRef<Path>) -> Option<serde_json::Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn read_jsonl(path: impl AsRef<Path>) -> Vec<serde_json::Value> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect()
}

fn report_id(findings: &[ProtocolInvariantFinding]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(now_secs().to_string().as_bytes());
    for finding in findings {
        hasher.update(finding.invariant_id.as_bytes());
        hasher.update(format!("{:?}", finding.status).as_bytes());
        hasher.update(finding.evidence.as_bytes());
    }
    let suffix = hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("psr-{}-{suffix}", now_secs())
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
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn protocol_checker_reports_event_chain_state() {
        let root = unique_temp_dir("buster-protocol-security");
        organism::ensure_node(&root).unwrap();
        organism::append_signed_event(&root, "test_event", serde_json::json!({"ok": true}))
            .unwrap();

        let report = check_protocol_invariants(&root).unwrap();

        assert!(report.pass_count >= 1);
        assert!(root.join(PROTOCOL_SECURITY_AUDIT).exists());
        assert!(root.join(PROTOCOL_SECURITY_STATE).exists());
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
