//! Changes to rhythm, priority, and strategy based on experience.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptationProposal {
    pub target: AdaptationTarget,
    pub reason: String,
    pub expected_effect: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdaptationTarget {
    CircadianClock,
    InstinctPriority,
    SkillSelection,
    ToolPolicy,
    MemoryPolicy,
}
