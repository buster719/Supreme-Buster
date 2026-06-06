//! Buster's value-system v0.
//!
//! This crate is deliberately not a neural network yet. It provides the
//! auditable data structures and a rule-based baseline evaluator that the
//! runtime cycle can call before stronger online learning exists.

pub mod episode;
pub mod evaluator;
pub mod preferences;
pub mod rules;
pub mod types;

pub use episode::{Episode, EpisodeLedger};
pub use evaluator::ValueEvaluator;
pub use preferences::{PreferenceDomain, PreferenceSignal};
pub use rules::RuleBasedValueEvaluator;
pub use types::{
    ActionCandidate, ActionKind, InfoSource, ResearchDomain, ValueContext, ValueJudgement,
};
