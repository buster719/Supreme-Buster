use serde::{Deserialize, Serialize};

use crate::types::{ActionCandidate, ValueJudgement};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub timestamp: String,
    pub state_summary: String,
    pub candidate_action: ActionCandidate,
    pub selected_action: Option<ActionCandidate>,
    pub value_judgement: ValueJudgement,
    pub body_gate_decision: String,
    pub resource_cost: f32,
    pub actual_outcome: Option<String>,
    pub human_feedback: Option<String>,
    pub agent_feedback: Option<String>,
    pub memory_impact: Option<String>,
    pub security_result: Option<String>,
    pub retrospective_notes: Option<String>,
}

impl Episode {
    pub fn new(
        timestamp: impl Into<String>,
        state_summary: impl Into<String>,
        candidate_action: ActionCandidate,
        value_judgement: ValueJudgement,
    ) -> Self {
        Self {
            timestamp: timestamp.into(),
            state_summary: state_summary.into(),
            candidate_action,
            selected_action: None,
            value_judgement,
            body_gate_decision: "not_checked".to_string(),
            resource_cost: 0.0,
            actual_outcome: None,
            human_feedback: None,
            agent_feedback: None,
            memory_impact: None,
            security_result: None,
            retrospective_notes: None,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodeLedger {
    episodes: Vec<Episode>,
}

impl EpisodeLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, episode: Episode) {
        self.episodes.push(episode);
    }

    pub fn episodes(&self) -> &[Episode] {
        &self.episodes
    }

    pub fn len(&self) -> usize {
        self.episodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.episodes.is_empty()
    }
}
