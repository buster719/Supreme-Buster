//! Hermes-style nudges for memory and skill review.
//!
//! Nudges are soft background prompts from the agency rhythm. They do not write
//! memory or skills directly; they ask a reviewer to inspect the recent context
//! when the relevant tool has not been used for a while.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NudgeConfig {
    pub memory_interval_turns: u32,
    pub skill_interval_turns: u32,
}

impl Default for NudgeConfig {
    fn default() -> Self {
        Self {
            memory_interval_turns: 10,
            skill_interval_turns: 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NudgeToolAvailability {
    pub memory_tool: bool,
    pub skill_tool: bool,
}

impl NudgeToolAvailability {
    pub fn all_available() -> Self {
        Self {
            memory_tool: true,
            skill_tool: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeEvent {
    UserTurn,
    MemoryToolUsed,
    SkillToolUsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeKind {
    MemoryReview,
    SkillReview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NudgeRequest {
    pub kind: NudgeKind,
    pub turns_since_last_use: u32,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NudgeState {
    pub config: NudgeConfig,
    pub turns_since_memory: u32,
    pub turns_since_skill: u32,
}

impl Default for NudgeState {
    fn default() -> Self {
        Self {
            config: NudgeConfig::default(),
            turns_since_memory: 0,
            turns_since_skill: 0,
        }
    }
}

impl NudgeState {
    pub fn new(config: NudgeConfig) -> Self {
        Self {
            config,
            turns_since_memory: 0,
            turns_since_skill: 0,
        }
    }

    pub fn observe_event(
        &mut self,
        event: NudgeEvent,
        tools: NudgeToolAvailability,
    ) -> Vec<NudgeRequest> {
        match event {
            NudgeEvent::UserTurn => {
                self.turns_since_memory = self.turns_since_memory.saturating_add(1);
                self.turns_since_skill = self.turns_since_skill.saturating_add(1);
                self.pending_requests(tools)
            }
            NudgeEvent::MemoryToolUsed => {
                self.turns_since_memory = 0;
                Vec::new()
            }
            NudgeEvent::SkillToolUsed => {
                self.turns_since_skill = 0;
                Vec::new()
            }
        }
    }

    pub fn pending_requests(&self, tools: NudgeToolAvailability) -> Vec<NudgeRequest> {
        let mut requests = Vec::new();
        if tools.memory_tool && self.turns_since_memory >= self.config.memory_interval_turns {
            requests.push(NudgeRequest {
                kind: NudgeKind::MemoryReview,
                turns_since_last_use: self.turns_since_memory,
                prompt: "Review recent context for stable facts, preferences, recurring experiences, or immune lessons worth writing to daily recall. Do not write secrets, task logs, identity memory, or value memory.".to_string(),
            });
        }
        if tools.skill_tool && self.turns_since_skill >= self.config.skill_interval_turns {
            requests.push(NudgeRequest {
                kind: NudgeKind::SkillReview,
                turns_since_last_use: self.turns_since_skill,
                prompt: "Review recent work for repeated procedures that should become or update a skill. Skills must include trigger, purpose, procedure, validation, and disabled_when boundaries.".to_string(),
            });
        }
        requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_nudge_triggers_after_interval_when_tool_exists() {
        let mut state = NudgeState::new(NudgeConfig {
            memory_interval_turns: 3,
            skill_interval_turns: 99,
        });
        let tools = NudgeToolAvailability {
            memory_tool: true,
            skill_tool: false,
        };

        assert!(state.observe_event(NudgeEvent::UserTurn, tools).is_empty());
        assert!(state.observe_event(NudgeEvent::UserTurn, tools).is_empty());
        let requests = state.observe_event(NudgeEvent::UserTurn, tools);

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].kind, NudgeKind::MemoryReview);
        assert_eq!(requests[0].turns_since_last_use, 3);
    }

    #[test]
    fn using_memory_tool_resets_memory_counter() {
        let mut state = NudgeState::new(NudgeConfig {
            memory_interval_turns: 2,
            skill_interval_turns: 99,
        });
        let tools = NudgeToolAvailability {
            memory_tool: true,
            skill_tool: false,
        };

        state.observe_event(NudgeEvent::UserTurn, tools);
        state.observe_event(NudgeEvent::MemoryToolUsed, tools);
        let requests = state.observe_event(NudgeEvent::UserTurn, tools);

        assert!(requests.is_empty());
        assert_eq!(state.turns_since_memory, 1);
    }

    #[test]
    fn unavailable_tools_do_not_trigger_nudges() {
        let mut state = NudgeState::new(NudgeConfig {
            memory_interval_turns: 1,
            skill_interval_turns: 1,
        });

        let requests = state.observe_event(
            NudgeEvent::UserTurn,
            NudgeToolAvailability {
                memory_tool: false,
                skill_tool: false,
            },
        );

        assert!(requests.is_empty());
    }

    #[test]
    fn memory_and_skill_nudges_are_independent() {
        let mut state = NudgeState::new(NudgeConfig {
            memory_interval_turns: 2,
            skill_interval_turns: 2,
        });
        let tools = NudgeToolAvailability::all_available();

        state.observe_event(NudgeEvent::UserTurn, tools);
        state.observe_event(NudgeEvent::SkillToolUsed, tools);
        let requests = state.observe_event(NudgeEvent::UserTurn, tools);

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].kind, NudgeKind::MemoryReview);
        assert_eq!(state.turns_since_memory, 2);
        assert_eq!(state.turns_since_skill, 1);
    }
}
