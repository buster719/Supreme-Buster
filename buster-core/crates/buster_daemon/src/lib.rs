//! Long-running Buster runtime v0.
//!
//! `buster_daemon` is the first persistent outer loop. It does not make
//! external side effects by itself. Each tick observes body state, creates
//! candidate activities, asks `buster_free_will::RuntimeCycle` to choose, and
//! writes durable audit/episode records for later learning.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod arena;
pub mod autobiography;
pub mod body_gate_bridge;
pub mod executor;
pub mod harness;
pub mod organism;
pub mod paper_acquisition;
pub mod paper_brief;
pub mod paperqa;
pub mod protocol_security;
pub mod research;
pub mod research_framework;
pub mod security_research;
pub mod self_review;
pub mod source_fetch;
pub mod web;
pub mod web_search;

use buster_body::{
    AuthorityFileSensor, BodyController, BodyRuntimePaths, BodyRuntimeWriter, BodySupervisor,
    DesiredBodyState, SensorSignal, SensorSuite, SignalSeverity, StaticSignalSensor,
    SupervisorConfig,
};
use buster_free_will::{CycleInput, RuntimeCycle};
use buster_value_model::{ActionCandidate, ActionKind, Episode, InfoSource, ValueContext};
use serde::{Deserialize, Serialize};

use crate::executor::execute_action;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonConfig {
    pub root: PathBuf,
    pub tick_interval: Duration,
    pub max_ticks: Option<usize>,
    pub write_body_reports: bool,
    pub max_context_bytes: u64,
    pub execute_actions: bool,
}

impl DaemonConfig {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            tick_interval: Duration::from_secs(60),
            max_ticks: Some(1),
            write_body_reports: true,
            max_context_bytes: 64 * 1024,
            execute_actions: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonTickRecord {
    pub contract_id: String,
    pub tick_index: usize,
    pub timestamp_secs: u64,
    pub body_mode: String,
    pub body_signal_count: usize,
    pub selected_action_kind: String,
    pub selected_action_summary: String,
    pub value_score: f32,
    pub governance_level: u8,
    pub reason: String,
    pub written_body_paths: Vec<String>,
    pub execution_summary: Option<String>,
    pub execution_record_path: Option<String>,
    pub self_review_summary: Option<String>,
    pub self_review_proposal_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonStateSnapshot {
    pub last_tick_index: usize,
    pub last_tick_timestamp_secs: u64,
    pub last_body_mode: String,
    pub last_selected_action_kind: String,
    pub last_selected_action_summary: String,
}

pub struct BusterDaemon {
    config: DaemonConfig,
    supervisor: BodySupervisor,
    cycle: RuntimeCycle,
}

impl BusterDaemon {
    pub fn new(config: DaemonConfig) -> std::io::Result<Self> {
        ensure_daemon_dirs(&config.root)?;
        let identity = organism::ensure_node(&config.root)?;
        let manifest = organism::write_body_manifest(&config.root, &identity)?;
        organism::append_signed_event(
            &config.root,
            "daemon_started",
            serde_json::json!({
                "node_id": identity.node_id,
                "body_manifest_hash": hash_json_for_payload(&manifest)?,
                "execute_actions": config.execute_actions,
                "tick_interval_ms": config.tick_interval.as_millis(),
                "max_ticks": config.max_ticks,
            }),
        )?;
        let sensors = daemon_sensors(&config.root)?;
        let writer = if config.write_body_reports {
            Some(BodyRuntimeWriter::new(BodyRuntimePaths::from_project_root(
                &config.root,
            )))
        } else {
            None
        };
        let supervisor = BodySupervisor::new(
            BodyController::new(DesiredBodyState::normal()),
            sensors,
            writer,
            SupervisorConfig {
                tick_interval: config.tick_interval,
                max_ticks: Some(1),
                write_reports: config.write_body_reports,
            },
        );

        Ok(Self {
            config,
            supervisor,
            cycle: RuntimeCycle::new(),
        })
    }

    pub fn tick(&mut self, index: usize) -> std::io::Result<DaemonTickRecord> {
        let body_tick = self.supervisor.tick(index)?;
        let value_context = self.value_context();
        let mut candidates = candidates_for_root(&self.config.root, &value_context);
        if candidates.is_empty() {
            candidates.push(ActionCandidate::new(
                ActionKind::Standby,
                "no activity candidates available",
            ));
        }

        let threat = threat_from_signals(&body_tick.report.signals);
        let decision = self.cycle.decide(CycleInput {
            context: value_context,
            candidates,
            threat,
        });
        let timestamp_secs = timestamp_secs();
        let contract =
            harness::runtime_cycle_contract(index, timestamp_secs, &decision.selected_action);
        harness::append_contract(&self.config.root, &contract)?;
        let execution = if self.config.execute_actions {
            Some(execute_action(
                &self.config.root,
                &decision.selected_action,
            )?)
        } else {
            None
        };
        let self_review = self_review::run_self_review(&self.config.root, index)?;
        let record = DaemonTickRecord {
            contract_id: contract.contract_id.clone(),
            tick_index: index,
            timestamp_secs,
            body_mode: body_tick.report.next_mode.as_str().to_string(),
            body_signal_count: body_tick.report.signals.len(),
            selected_action_kind: format!("{:?}", decision.selected_action.kind),
            selected_action_summary: decision.selected_action.summary.clone(),
            value_score: decision.judgement.composite_score(),
            governance_level: decision.judgement.governance_level,
            reason: decision.reason.clone(),
            written_body_paths: body_tick
                .written_paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            execution_summary: execution.as_ref().map(|record| record.summary.clone()),
            execution_record_path: execution
                .as_ref()
                .map(|record| record.record_path.to_string_lossy().into_owned()),
            self_review_summary: if self_review.findings.is_empty() {
                None
            } else {
                Some(format!(
                    "{} self-review finding(s), {} proposal(s)",
                    self_review.findings.len(),
                    self_review.proposal_paths.len()
                ))
            },
            self_review_proposal_paths: self_review
                .proposal_paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        };

        let mut episode = Episode::new(
            timestamp_secs.to_string(),
            format!(
                "body_mode={}, body_signals={}",
                record.body_mode, record.body_signal_count
            ),
            decision.selected_action.clone(),
            decision.judgement,
        );
        episode.selected_action = Some(decision.selected_action);
        episode.contract_id = Some(contract.contract_id.clone());
        episode.body_gate_decision = format!("body_supervisor_mode:{}", record.body_mode);
        episode.resource_cost = record.value_score;
        episode.security_result = Some(format!("{} body signal(s)", record.body_signal_count));
        episode.actual_outcome = execution
            .as_ref()
            .map(|record| format!("{}: {}", record.status, record.summary));

        append_jsonl(
            &self.config.root.join("audit").join("runtime-cycle.jsonl"),
            &record,
        )?;
        append_jsonl(
            &self.config.root.join("audit").join("value-episodes.jsonl"),
            &episode,
        )?;
        organism::append_signed_event(
            &self.config.root,
            "runtime_cycle",
            serde_json::to_value(&record).map_err(std::io::Error::other)?,
        )?;
        harness::append_outcome(
            &self.config.root,
            &harness::HarnessOutcome::new(
                contract.contract_id.clone(),
                harness::HarnessStatus::Completed,
                format!(
                    "tick {} selected {}",
                    record.tick_index, record.selected_action_kind
                ),
            )
            .with_output("audit", "audit/runtime-cycle.jsonl", "runtime cycle record")
            .with_output(
                "audit",
                "audit/value-episodes.jsonl",
                "value episode record",
            ),
        )?;
        write_state_snapshot(
            &self.config.root,
            &DaemonStateSnapshot {
                last_tick_index: record.tick_index,
                last_tick_timestamp_secs: record.timestamp_secs,
                last_body_mode: record.body_mode.clone(),
                last_selected_action_kind: record.selected_action_kind.clone(),
                last_selected_action_summary: record.selected_action_summary.clone(),
            },
        )?;

        Ok(record)
    }

    pub fn run(&mut self) -> std::io::Result<Vec<DaemonTickRecord>> {
        let max_ticks = self.config.max_ticks.unwrap_or(usize::MAX);
        let mut records = Vec::new();
        for index in 0..max_ticks {
            records.push(self.tick(index)?);
            if index + 1 < max_ticks {
                thread::sleep(self.config.tick_interval);
            }
        }
        Ok(records)
    }

    fn value_context(&self) -> ValueContext {
        let context_bytes = context_bytes(&self.config.root);
        ValueContext {
            context_pressure: ratio(context_bytes, self.config.max_context_bytes),
            resource_pressure: 0.0,
            first_hand_interaction_available: first_hand_interaction_available(&self.config.root),
            quiet_hours_for_target_humans: false,
            recent_topics: Vec::new(),
        }
    }
}

fn daemon_sensors(root: &Path) -> std::io::Result<SensorSuite> {
    let protected_paths = ["self.md", "GOVERNANCE.md", "BODY.md", "VALUE_MODEL.md"]
        .into_iter()
        .map(|name| root.join(name))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();

    let mut suite = SensorSuite::new();
    if protected_paths.is_empty() {
        suite = suite.with_sensor(StaticSignalSensor::new(vec![SensorSignal::critical(
            buster_body::SignalKind::AuthorityFileMissing,
            root.to_string_lossy(),
            "no protected authority files found at daemon root",
        )]));
    } else {
        suite = suite.with_sensor(AuthorityFileSensor::from_paths(protected_paths)?);
    }
    Ok(suite)
}

fn candidates_for_root(root: &Path, context: &ValueContext) -> Vec<ActionCandidate> {
    let mut candidates = Vec::new();

    if context.context_pressure >= 0.75 {
        candidates.push(
            ActionCandidate::new(ActionKind::Skeftomai, "context pressure crossed threshold")
                .with_cost(0.35),
        );
        candidates.push(ActionCandidate::new(
            ActionKind::Standby,
            "wait after context-pressure reflection candidate",
        ));
        return candidates;
    }

    if has_files(root.join("inbox").join("human")) {
        candidates.push(
            ActionCandidate::new(ActionKind::HumanDialogue, "process pending human inbox")
                .with_info_source(InfoSource::FirstHandHuman)
                .with_cost(0.25),
        );
    }
    if has_files(root.join("inbox").join("agents")) {
        candidates.push(
            ActionCandidate::new(ActionKind::AgentDialogue, "process pending agent inbox")
                .with_info_source(InfoSource::FirstHandAgent)
                .with_security_risk(0.25),
        );
    }
    if !context.first_hand_interaction_available {
        if let Ok(Some(candidate)) = research::next_research_candidate(root) {
            candidates.push(candidate);
        }
        candidates.push(
            ActionCandidate::new(
                ActionKind::SecondaryResearch,
                "ask secondary LLM for research leads and verification paths",
            )
            .with_info_source(InfoSource::LlmSecondary)
            .with_cost(0.2),
        );
    }

    candidates.push(ActionCandidate::new(
        ActionKind::Standby,
        "remain available for deterministic next tick",
    ));
    candidates
}

fn threat_from_signals(
    signals: &[SensorSignal],
) -> Option<buster_free_will::threat_interrupts::ThreatKind> {
    if !signals
        .iter()
        .any(|signal| signal.severity == SignalSeverity::Critical)
    {
        return None;
    }

    if signals.iter().any(|signal| {
        matches!(
            signal.kind,
            buster_body::SignalKind::AuthorityFileChanged
                | buster_body::SignalKind::AuthorityFileMissing
        )
    }) {
        return Some(buster_free_will::threat_interrupts::ThreatKind::IdentityKeyRisk);
    }

    Some(buster_free_will::threat_interrupts::ThreatKind::ResourceExhaustion)
}

fn ensure_daemon_dirs(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("audit"))?;
    fs::create_dir_all(root.join("state"))?;
    fs::create_dir_all(root.join("secrets"))?;
    fs::create_dir_all(root.join("research"))?;
    fs::create_dir_all(root.join("security").join("reports"))?;
    fs::create_dir_all(root.join("proposals").join("body"))?;
    let _ = research::ensure_research_queue(root)?;
    research::update_research_digest(root)?;
    let _ = research_framework::update_quality_digest(root)?;
    let _ = security_research::ensure_security_taskflows(root)?;
    let _ = protocol_security::check_protocol_invariants(root)?;
    buster_skills::SkillWorkspace::new(root)
        .refresh_registry()
        .map_err(std::io::Error::other)?;
    buster_tools::ToolWorkspace::new(root)
        .refresh_registry()
        .map_err(std::io::Error::other)?;
    let _ = autobiography::refresh(root)?;
    Ok(())
}

fn hash_json_for_payload(value: &impl Serialize) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
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

pub(crate) fn append_daemon_jsonl(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    append_jsonl(path, value)
}

pub fn now_secs() -> u64 {
    timestamp_secs()
}

fn write_state_snapshot(root: &Path, snapshot: &DaemonStateSnapshot) -> std::io::Result<()> {
    let path = root.join("state").join("buster-daemon.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(snapshot).map_err(std::io::Error::other)?;
    fs::write(path, json)
}

fn context_bytes(root: &Path) -> u64 {
    ["MEMORY.md", "SKEFTOMAI.md"]
        .into_iter()
        .map(|name| file_len(root.join(name)))
        .sum::<u64>()
        + dir_len(root.join("memory"))
}

fn file_len(path: impl AsRef<Path>) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn dir_len(path: impl AsRef<Path>) -> u64 {
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                dir_len(path)
            } else {
                file_len(path)
            }
        })
        .sum()
}

fn first_hand_interaction_available(root: &Path) -> bool {
    has_files(root.join("inbox").join("human")) || has_files(root.join("inbox").join("agents"))
}

fn has_files(path: impl AsRef<Path>) -> bool {
    fs::read_dir(path).ok().is_some_and(|mut entries| {
        entries.any(|entry| entry.is_ok_and(|entry| entry.path().is_file()))
    })
}

fn ratio(value: u64, max: u64) -> f32 {
    if max == 0 {
        return 1.0;
    }
    ((value as f64 / max as f64).min(1.0)) as f32
}

fn timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_single_tick_writes_audit_episode_and_state() {
        let root = unique_temp_dir("buster-daemon-tick");
        write_authority_files(&root);
        fs::create_dir_all(root.join("inbox").join("human")).unwrap();
        fs::write(root.join("inbox").join("human").join("msg.md"), "hello").unwrap();
        let mut daemon = BusterDaemon::new(DaemonConfig {
            root: root.clone(),
            tick_interval: Duration::from_millis(0),
            max_ticks: Some(1),
            write_body_reports: true,
            max_context_bytes: 64 * 1024,
            execute_actions: false,
        })
        .unwrap();

        let record = daemon.tick(0).unwrap();

        assert_eq!(record.tick_index, 0);
        assert!(!record.contract_id.is_empty());
        assert_eq!(record.selected_action_kind, "HumanDialogue");
        assert!(root.join("audit").join("runtime-cycle.jsonl").exists());
        assert!(root.join("audit").join("harness-contracts.jsonl").exists());
        assert!(root.join("audit").join("harness-outcomes.jsonl").exists());
        assert!(root.join("audit").join("value-episodes.jsonl").exists());
        assert!(root.join("state").join("buster-daemon.json").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn daemon_selects_skeftomai_when_context_pressure_is_high() {
        let root = unique_temp_dir("buster-daemon-skeftomai");
        write_authority_files(&root);
        fs::write(root.join("MEMORY.md"), "x".repeat(2048)).unwrap();
        let mut daemon = BusterDaemon::new(DaemonConfig {
            root: root.clone(),
            tick_interval: Duration::from_millis(0),
            max_ticks: Some(1),
            write_body_reports: false,
            max_context_bytes: 1024,
            execute_actions: false,
        })
        .unwrap();

        let record = daemon.tick(0).unwrap();

        assert_eq!(record.selected_action_kind, "Skeftomai");
        let _ = fs::remove_dir_all(root);
    }

    fn write_authority_files(root: &Path) {
        fs::create_dir_all(root).unwrap();
        fs::write(root.join("self.md"), "# Self\n").unwrap();
        fs::write(root.join("GOVERNANCE.md"), "# Governance\n").unwrap();
        fs::write(root.join("BODY.md"), "# Body\n").unwrap();
        fs::write(root.join("VALUE_MODEL.md"), "# Value\n").unwrap();
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
