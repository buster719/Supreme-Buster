//! Authority vocabulary for the body layer.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ChangeLevel {
    Observation = 0,
    FactMemory = 1,
    BehaviorExperience = 2,
    CapabilityExecution = 3,
    BoundaryResource = 4,
    IdentityValue = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLine {
    Green,
    Yellow,
    Red,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyScope {
    pub agent_id: String,
    pub branch_id: String,
    pub task_id: String,
}

impl BodyScope {
    pub fn new(
        agent_id: impl Into<String>,
        branch_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            branch_id: branch_id.into(),
            task_id: task_id.into(),
        }
    }
}
