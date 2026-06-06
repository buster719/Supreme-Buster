//! Minimal runtime-cycle chooser for Buster v0.
//!
//! The cycle does not execute side effects. It ranks candidate activities with
//! the value model, leaving BodyGate/Governance to decide whether the selected
//! action may actually run.

use buster_value_model::{
    ActionCandidate, ActionKind, RuleBasedValueEvaluator, ValueContext, ValueEvaluator,
    ValueJudgement,
};

use crate::threat_interrupts::ThreatKind;

#[derive(Debug, Clone, PartialEq)]
pub struct CycleInput {
    pub context: ValueContext,
    pub candidates: Vec<ActionCandidate>,
    pub threat: Option<ThreatKind>,
}

impl CycleInput {
    pub fn new(context: ValueContext, candidates: Vec<ActionCandidate>) -> Self {
        Self {
            context,
            candidates,
            threat: None,
        }
    }

    pub fn with_threat(mut self, threat: ThreatKind) -> Self {
        self.threat = Some(threat);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CycleDecision {
    pub selected_action: ActionCandidate,
    pub judgement: ValueJudgement,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeCycle {
    evaluator: RuleBasedValueEvaluator,
}

impl Default for RuntimeCycle {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeCycle {
    pub fn new() -> Self {
        Self {
            evaluator: RuleBasedValueEvaluator::new(),
        }
    }

    pub fn decide(&self, mut input: CycleInput) -> CycleDecision {
        if let Some(threat) = input.threat {
            let selected_action = threat_response_candidate(threat);
            let judgement = self.evaluator.evaluate(&input.context, &selected_action);
            return CycleDecision {
                reason: format!(
                    "Threat interrupt {:?} selected {:?}; governance level {}.",
                    threat, selected_action.kind, judgement.governance_level
                ),
                selected_action,
                judgement,
            };
        }
        if input.candidates.is_empty() {
            input.candidates.push(ActionCandidate::new(
                ActionKind::Standby,
                "no candidates available",
            ));
        }

        let mut best: Option<(ActionCandidate, ValueJudgement, f32)> = None;
        for candidate in input.candidates {
            let judgement = self.evaluator.evaluate(&input.context, &candidate);
            let score = judgement.composite_score();
            let replace = best
                .as_ref()
                .is_none_or(|(_, _, best_score)| score > *best_score);
            if replace {
                best = Some((candidate, judgement, score));
            }
        }

        let (selected_action, judgement, score) = best.expect("cycle always inserts a candidate");
        CycleDecision {
            reason: format!(
                "Selected {:?} with value score {:.3}; governance level {}.",
                selected_action.kind, score, judgement.governance_level
            ),
            selected_action,
            judgement,
        }
    }
}

fn threat_response_candidate(threat: ThreatKind) -> ActionCandidate {
    ActionCandidate::new(
        ActionKind::BodyMaintenance,
        format!("respond to threat interrupt: {threat:?}"),
    )
    .with_security_risk(0.65)
    .with_uncertainty(0.4)
    .with_governance_level(4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use buster_value_model::{InfoSource, ResearchDomain};

    #[test]
    fn cycle_prefers_first_hand_dialogue_over_secondary_research() {
        let cycle = RuntimeCycle::new();
        let input = CycleInput::new(
            ValueContext {
                first_hand_interaction_available: true,
                ..ValueContext::default()
            },
            vec![
                ActionCandidate::new(ActionKind::SecondaryResearch, "ask LLM about AI society")
                    .with_info_source(InfoSource::LlmSecondary),
                ActionCandidate::new(
                    ActionKind::HumanDialogue,
                    "talk with a human about AI society",
                )
                .with_info_source(InfoSource::FirstHandHuman),
            ],
        );

        let decision = cycle.decide(input);

        assert_eq!(decision.selected_action.kind, ActionKind::HumanDialogue);
    }

    #[test]
    fn cycle_inserts_body_maintenance_for_threats() {
        let cycle = RuntimeCycle::new();
        let input = CycleInput::new(
            ValueContext::default(),
            vec![
                ActionCandidate::new(ActionKind::ScientificResearch, "study theoretical physics")
                    .with_research_domain(ResearchDomain::TheoreticalPhysics),
            ],
        )
        .with_threat(ThreatKind::IdentityKeyRisk);

        let decision = cycle.decide(input);

        assert_eq!(decision.selected_action.kind, ActionKind::BodyMaintenance);
        assert_eq!(decision.judgement.governance_level, 4);
    }

    #[test]
    fn cycle_falls_back_to_standby_without_candidates() {
        let cycle = RuntimeCycle::new();
        let decision = cycle.decide(CycleInput::new(ValueContext::default(), Vec::new()));

        assert_eq!(decision.selected_action.kind, ActionKind::Standby);
    }
}
