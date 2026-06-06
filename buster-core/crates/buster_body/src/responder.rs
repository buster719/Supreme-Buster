//! Conservative responses emitted by the body controller.

use crate::registry::{ManagedUnit, UnitStatus};
use crate::sensors::{SensorSignal, SignalSeverity};
use crate::state::BodyMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyAction {
    EnterMode(BodyMode),
    FreezeOwnedUnit { unit_id: String, reason: String },
    RevokeLease { unit_id: String, reason: String },
    TightenNetwork { reason: String },
    TightenSecrets { reason: String },
    WriteEmergencyRecord,
    WriteAuditRecord,
    CreateBodyProposalDraft,
    ObserveOnly,
}

#[derive(Debug, Clone, Default)]
pub struct BodyResponder;

impl BodyResponder {
    pub fn actions_for(
        &self,
        previous_mode: BodyMode,
        next_mode: BodyMode,
        signals: &[SensorSignal],
        units: &[ManagedUnit],
    ) -> Vec<BodyAction> {
        let mut actions = Vec::new();
        let permissions = next_mode.permissions();

        if previous_mode != next_mode {
            actions.push(BodyAction::EnterMode(next_mode));
        }

        if !permissions.allow_network {
            actions.push(BodyAction::TightenNetwork {
                reason: format!(
                    "{} mode blocks nonessential network egress",
                    next_mode.as_str()
                ),
            });
        }

        if !permissions.allow_secrets {
            actions.push(BodyAction::TightenSecrets {
                reason: format!(
                    "{} mode blocks nonessential secret access",
                    next_mode.as_str()
                ),
            });
        }

        if next_mode == BodyMode::EmergencyContainment {
            for unit in units.iter().filter(|unit| {
                matches!(unit.status, UnitStatus::Running)
                    && unit
                        .notes
                        .iter()
                        .any(|note| note.contains("suspicious") || note.contains("anomaly"))
            }) {
                actions.push(BodyAction::FreezeOwnedUnit {
                    unit_id: unit.id.clone(),
                    reason: "running Buster-owned unit has suspicious notes".to_string(),
                });
                if unit.lease.is_some() {
                    actions.push(BodyAction::RevokeLease {
                        unit_id: unit.id.clone(),
                        reason: "lease revoked during emergency containment".to_string(),
                    });
                }
            }
        }

        if permissions.require_audit
            || signals
                .iter()
                .any(|signal| signal.severity >= SignalSeverity::Warning)
        {
            actions.push(BodyAction::WriteAuditRecord);
        }

        if next_mode == BodyMode::EmergencyContainment {
            actions.push(BodyAction::WriteEmergencyRecord);
        }

        if permissions.require_body_proposal {
            actions.push(BodyAction::CreateBodyProposalDraft);
        }

        if actions.is_empty() {
            actions.push(BodyAction::ObserveOnly);
        }

        actions
    }
}
