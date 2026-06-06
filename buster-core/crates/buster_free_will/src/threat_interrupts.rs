//! Threats can interrupt normal rhythms.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatKind {
    IdentityKeyRisk,
    PromptInjection,
    ToolPoisoning,
    ResourceExhaustion,
    ValueModelPoisoning,
    CivilizationRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptAction {
    Continue,
    SlowDown,
    Quarantine,
    Stop,
}
