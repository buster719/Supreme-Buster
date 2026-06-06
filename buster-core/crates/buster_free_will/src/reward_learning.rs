//! Learning from consequences without reducing Buster to reward maximization.

#[derive(Debug, Clone, PartialEq)]
pub struct RewardSignal {
    pub score: f32,
    pub explanation: String,
    pub uncertainty: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsequenceRecord {
    pub action_id: String,
    pub observed_effect: String,
    pub accepted_responsibility: bool,
}
