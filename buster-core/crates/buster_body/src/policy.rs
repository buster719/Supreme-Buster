//! Policy evaluation for the body controller.

use crate::sensors::{SensorSignal, SignalSeverity};
use crate::state::BodyMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    pub next_mode: BodyMode,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyPolicy {
    pub warning_threshold: usize,
}

impl Default for BodyPolicy {
    fn default() -> Self {
        Self {
            warning_threshold: 1,
        }
    }
}

impl BodyPolicy {
    pub fn decide(&self, current_mode: BodyMode, signals: &[SensorSignal]) -> PolicyDecision {
        if signals
            .iter()
            .any(|signal| signal.severity == SignalSeverity::Critical)
        {
            return PolicyDecision {
                next_mode: BodyMode::EmergencyContainment,
                reason: "critical body-layer signal detected".to_string(),
            };
        }

        let warnings = signals
            .iter()
            .filter(|signal| signal.severity == SignalSeverity::Warning)
            .count();

        if warnings >= self.warning_threshold {
            return PolicyDecision {
                next_mode: BodyMode::Restricted,
                reason: format!("{warnings} warning signal(s) crossed restricted threshold"),
            };
        }

        if current_mode == BodyMode::EmergencyContainment {
            return PolicyDecision {
                next_mode: BodyMode::Recovery,
                reason: "no critical signal remains; enter recovery before normal".to_string(),
            };
        }

        PolicyDecision {
            next_mode: BodyMode::Normal,
            reason: "body state matches desired normal state".to_string(),
        }
    }
}
