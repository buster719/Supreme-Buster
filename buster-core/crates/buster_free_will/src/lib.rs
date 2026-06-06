//! Buster's first agency loop.
//!
//! Agency v1 = circadian clock + homeostasis + instincts/runtime reflexes
//! + reward learning + threat interrupts + environmental adaptation.

pub mod adaptation;
pub mod circadian_clock;
pub mod cycle;
pub mod homeostasis;
pub mod instincts;
pub mod nudge;
pub mod reward_learning;
pub mod threat_interrupts;

pub use cycle::{CycleDecision, CycleInput, RuntimeCycle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgencyLoop {
    pub clock: circadian_clock::CircadianClock,
    pub instincts: Vec<instincts::Instinct>,
    pub nudges: nudge::NudgeState,
}

impl AgencyLoop {
    pub fn v1() -> Self {
        Self {
            clock: circadian_clock::CircadianClock::default(),
            instincts: instincts::default_instincts(),
            nudges: nudge::NudgeState::default(),
        }
    }
}
