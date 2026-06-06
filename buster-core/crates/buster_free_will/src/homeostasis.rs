//! Steady-state needs for Buster's body, memory, tools, and value model.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeostaticNeed {
    Energy,
    MemoryHealth,
    SecurityHealth,
    ToolHealth,
    ValueModelHealth,
    SocialCalibration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeedPressure {
    Low,
    Medium,
    High,
    Critical,
}
