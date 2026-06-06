//! Buster Body Runtime controller.
//!
//! This follows the controller pattern: observe current signals, compare them
//! with the desired body state, and emit conservative actions that move Buster
//! toward safety without taking ownership of the whole host machine.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::audit::{AuditEvent, AuditLevel};
use crate::host_api::{ChangeLevel, RiskLine};
use crate::policy::{BodyPolicy, PolicyDecision};
use crate::registry::{ManagedUnit, UnitRegistry};
use crate::responder::{BodyAction, BodyResponder};
use crate::sensors::{SensorSignal, SignalSeverity};
use crate::state::{BodyMode, CurrentBodyState, DesiredBodyState};

#[derive(Debug, Clone)]
pub struct BodyController {
    desired: DesiredBodyState,
    current: CurrentBodyState,
    registry: UnitRegistry,
    policy: BodyPolicy,
    responder: BodyResponder,
}

impl BodyController {
    pub fn new(desired: DesiredBodyState) -> Self {
        Self {
            desired,
            current: CurrentBodyState::default(),
            registry: UnitRegistry::default(),
            policy: BodyPolicy::default(),
            responder: BodyResponder,
        }
    }

    pub fn mode(&self) -> BodyMode {
        self.current.mode
    }

    pub fn register_unit(&mut self, unit: ManagedUnit) {
        self.registry.register(unit);
    }

    pub fn reconcile(&mut self, signals: Vec<SensorSignal>) -> ReconcileReport {
        let previous_mode = self.current.mode;
        let decision = self.policy.decide(previous_mode, &signals);
        self.current.mode = decision.next_mode;

        let actions = self.responder.actions_for(
            previous_mode,
            decision.next_mode,
            &signals,
            self.registry.units(),
        );
        let audit_events = audit_events_for(&decision, &signals, &actions);

        ReconcileReport {
            desired_mode: self.desired.mode,
            previous_mode,
            next_mode: decision.next_mode,
            decision,
            signals,
            actions,
            audit_events,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconcileReport {
    pub desired_mode: BodyMode,
    pub previous_mode: BodyMode,
    pub next_mode: BodyMode,
    pub decision: PolicyDecision,
    pub signals: Vec<SensorSignal>,
    pub actions: Vec<BodyAction>,
    pub audit_events: Vec<AuditEvent>,
}

impl ReconcileReport {
    pub fn is_emergency(&self) -> bool {
        self.next_mode == BodyMode::EmergencyContainment
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyRuntimePaths {
    pub audit_dir: PathBuf,
    pub emergency_dir: PathBuf,
    pub body_proposals_dir: PathBuf,
}

impl BodyRuntimePaths {
    pub fn from_project_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            audit_dir: root.join("audit"),
            emergency_dir: root.join("emergency"),
            body_proposals_dir: root.join("proposals").join("body"),
        }
    }
}

pub struct BodyRuntimeWriter {
    paths: BodyRuntimePaths,
}

impl BodyRuntimeWriter {
    pub fn new(paths: BodyRuntimePaths) -> Self {
        Self { paths }
    }

    pub fn write_report(&self, report: &ReconcileReport) -> std::io::Result<Vec<PathBuf>> {
        let mut written = Vec::new();
        let stamp = timestamp_nanos();

        if report
            .actions
            .iter()
            .any(|action| matches!(action, BodyAction::WriteEmergencyRecord))
        {
            fs::create_dir_all(&self.paths.emergency_dir)?;
            let path = self
                .paths
                .emergency_dir
                .join(format!("{stamp}-containment.md"));
            fs::write(&path, render_emergency(report))?;
            written.push(path);
        }

        if report
            .actions
            .iter()
            .any(|action| matches!(action, BodyAction::WriteAuditRecord))
        {
            fs::create_dir_all(&self.paths.audit_dir)?;
            let path = self
                .paths
                .audit_dir
                .join(format!("{stamp}-body-runtime.md"));
            fs::write(&path, render_audit(report))?;
            written.push(path);
        }

        if report
            .actions
            .iter()
            .any(|action| matches!(action, BodyAction::CreateBodyProposalDraft))
        {
            fs::create_dir_all(&self.paths.body_proposals_dir)?;
            let path = self
                .paths
                .body_proposals_dir
                .join(format!("{stamp}-emergency-followup.md"));
            fs::write(&path, render_body_proposal(report))?;
            written.push(path);
        }

        Ok(written)
    }
}

fn audit_events_for(
    decision: &PolicyDecision,
    signals: &[SensorSignal],
    actions: &[BodyAction],
) -> Vec<AuditEvent> {
    let level = if signals
        .iter()
        .any(|signal| signal.severity == SignalSeverity::Critical)
    {
        AuditLevel::Critical
    } else if signals
        .iter()
        .any(|signal| signal.severity == SignalSeverity::Warning)
    {
        AuditLevel::Warning
    } else {
        AuditLevel::Info
    };

    vec![AuditEvent {
        level,
        change_level: ChangeLevel::BoundaryResource,
        risk_line: if level == AuditLevel::Critical {
            RiskLine::Red
        } else {
            RiskLine::Yellow
        },
        summary: format!(
            "body runtime reconciled to {} after {} signal(s)",
            decision.next_mode.as_str(),
            signals.len()
        ),
        consequence: Some(format!(
            "{}; emitted {} action(s)",
            decision.reason,
            actions.len()
        )),
    }]
}

fn render_emergency(report: &ReconcileReport) -> String {
    render_report("Emergency Containment Record", report)
}

fn render_audit(report: &ReconcileReport) -> String {
    render_report("Body Runtime Audit Record", report)
}

fn render_body_proposal(report: &ReconcileReport) -> String {
    format!(
        "# Body Upgrade Proposal: emergency follow-up\n\n\
         ## Metadata\n\n\
         - Governance layer: 4\n\
         - Status: draft\n\
         - Emergency status: emergency_containment\n\n\
         ## Summary\n\n\
         Buster entered emergency containment and generated this draft so the temporary response can be reviewed before any permanent BODY change.\n\n\
         ## Trigger\n\n{}\n\n\
         ## Proposed Behavior\n\n\
         Review the emergency actions, decide whether any body-layer rule should become permanent, and keep level 5 identity-value documents unchanged.\n\n\
         ## Safety Bounds\n\n\
         - Does not modify `self.md`.\n\
         - Does not modify `GOVERNANCE.md`.\n\
         - Does not permanently replace `BODY.md` without approval.\n\
         - Does not redefine Buster identity, civilization priority, or value-model authority.\n\n\
         ## Audit Plan\n\n\
         Preserve the paired `emergency/` and `audit/` records and attach scan/test evidence before approval.\n",
        render_signal_list(&report.signals)
    )
}

fn render_report(title: &str, report: &ReconcileReport) -> String {
    format!(
        "# {title}\n\n\
         ## State\n\n\
         - Desired: {}\n\
         - Previous: {}\n\
         - Next: {}\n\
         - Reason: {}\n\n\
         ## Signals\n\n{}\n\n\
         ## Actions\n\n{}\n",
        report.desired_mode.as_str(),
        report.previous_mode.as_str(),
        report.next_mode.as_str(),
        report.decision.reason,
        render_signal_list(&report.signals),
        render_action_list(&report.actions)
    )
}

fn render_signal_list(signals: &[SensorSignal]) -> String {
    if signals.is_empty() {
        return "- none".to_string();
    }

    signals
        .iter()
        .map(|signal| {
            format!(
                "- {:?} {:?} from `{}`: {}",
                signal.severity, signal.kind, signal.source, signal.summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_action_list(actions: &[BodyAction]) -> String {
    if actions.is_empty() {
        return "- none".to_string();
    }

    actions
        .iter()
        .map(|action| format!("- {action:?}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

pub fn ensure_runtime_dirs(paths: &BodyRuntimePaths) -> std::io::Result<()> {
    ensure_dir(&paths.audit_dir)?;
    ensure_dir(&paths.emergency_dir)?;
    ensure_dir(&paths.body_proposals_dir)?;
    Ok(())
}

fn ensure_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{ManagedUnit, UnitKind, UnitStatus};
    use crate::sensors::{SensorSignal, SignalKind};

    #[test]
    fn warning_signal_enters_restricted_mode() {
        let mut controller = BodyController::new(DesiredBodyState::normal());
        let report = controller.reconcile(vec![SensorSignal::warning(
            SignalKind::ResourcePressure,
            "budget",
            "token budget is near limit",
        )]);

        assert_eq!(report.next_mode, BodyMode::Restricted);
        assert!(report
            .actions
            .iter()
            .any(|action| matches!(action, BodyAction::WriteAuditRecord)));
    }

    #[test]
    fn critical_signal_enters_emergency_and_freezes_suspicious_owned_unit() {
        let mut controller = BodyController::new(DesiredBodyState::normal());
        let mut unit =
            ManagedUnit::new("tool-a", UnitKind::Tool, "buster").with_status(UnitStatus::Running);
        unit.notes.push("suspicious network anomaly".to_string());
        controller.register_unit(unit);

        let report = controller.reconcile(vec![SensorSignal::critical(
            SignalKind::AuthorityFileChanged,
            "BODY.md",
            "protected body file changed",
        )]);

        assert_eq!(report.next_mode, BodyMode::EmergencyContainment);
        assert!(report.is_emergency());
        assert!(report
            .actions
            .iter()
            .any(|action| matches!(action, BodyAction::FreezeOwnedUnit { unit_id, .. } if unit_id == "tool-a")));
        assert!(report
            .actions
            .iter()
            .any(|action| matches!(action, BodyAction::CreateBodyProposalDraft)));
    }

    #[test]
    fn writer_creates_emergency_audit_and_proposal_records() {
        let root = unique_temp_dir("buster-body-runtime-writer");
        let paths = BodyRuntimePaths::from_project_root(&root);
        let writer = BodyRuntimeWriter::new(paths);
        let mut controller = BodyController::new(DesiredBodyState::normal());
        let report = controller.reconcile(vec![SensorSignal::critical(
            SignalKind::SecretExposureRisk,
            "secrets",
            "possible identity secret exposure",
        )]);

        let written = writer.write_report(&report).unwrap();

        assert_eq!(written.len(), 3);
        assert!(written
            .iter()
            .any(|path| path.starts_with(root.join("emergency"))));
        assert!(written
            .iter()
            .any(|path| path.starts_with(root.join("audit"))));
        assert!(written
            .iter()
            .any(|path| path.starts_with(root.join("proposals").join("body"))));

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
