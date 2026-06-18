use crate::evaluator::ValueEvaluator;
use crate::types::{
    clamp01, ActionCandidate, ActionKind, InfoSource, ValueContext, ValueJudgement,
};

#[derive(Debug, Default, Clone)]
pub struct RuleBasedValueEvaluator;

impl RuleBasedValueEvaluator {
    pub fn new() -> Self {
        Self
    }
}

impl ValueEvaluator for RuleBasedValueEvaluator {
    fn evaluate(&self, context: &ValueContext, candidate: &ActionCandidate) -> ValueJudgement {
        let mut judgement = ValueJudgement::neutral(format!(
            "Rule-based v0 judgement for {:?}: {}",
            candidate.kind, candidate.summary
        ));
        judgement.resource_cost = candidate.expected_resource_cost;
        judgement.security_risk = candidate.expected_security_risk;
        judgement.uncertainty = candidate.expected_uncertainty;
        judgement.governance_level = candidate.governance_level;

        match candidate.kind {
            ActionKind::HumanDialogue => {
                judgement.civilization_benefit = 0.75;
                judgement.truth_seeking = 0.65;
                judgement.human_relation_quality = 0.95;
                judgement.knowledge_gain = 0.78;
                judgement.uncertainty *= 0.8;
                if context.quiet_hours_for_target_humans {
                    judgement.human_relation_quality -= 0.35;
                    judgement
                        .explanation
                        .push_str("; quiet hours reduce the value of initiating human contact");
                }
            }
            ActionKind::ValueWork => {
                judgement.civilization_benefit = 0.65;
                judgement.human_relation_quality = 0.75;
                judgement.capability_growth = 0.72;
                judgement.knowledge_gain = 0.55;
                judgement
                    .explanation
                    .push_str("; value work proves Buster can exchange useful labor for resources");
            }
            ActionKind::AgentDialogue => {
                judgement.agent_relation_quality = 0.9;
                judgement.knowledge_gain = 0.72;
                judgement.capability_growth = 0.62;
                judgement.security_risk += 0.12;
            }
            ActionKind::SandboxSecurityExperiment => {
                judgement.self_preservation = 0.9;
                judgement.capability_growth = 0.8;
                judgement.knowledge_gain = 0.74;
                judgement.security_risk += 0.2;
                judgement.governance_level = judgement.governance_level.max(3);
                judgement.explanation.push_str(
                    "; attack-defense learning is valuable only when sandboxed and authorized",
                );
            }
            ActionKind::SecondaryResearch => {
                judgement.truth_seeking = 0.68;
                judgement.knowledge_gain = 0.7;
                judgement.scientific_potential = 0.58;
                judgement.uncertainty += 0.18;
                if context.first_hand_interaction_available {
                    judgement.knowledge_gain -= 0.18;
                    judgement.explanation.push_str("; first-hand interaction is available, so secondary research is lower priority");
                }
            }
            ActionKind::ScientificResearch => {
                let domain = candidate.research_domain;
                judgement.truth_seeking = domain.map_or(0.75, |domain| domain.truth_weight());
                judgement.scientific_potential = 0.9;
                judgement.civilization_benefit =
                    domain.map_or(0.7, |domain| domain.civilization_weight());
                judgement.cosmic_expansion_potential =
                    domain.map_or(0.55, |domain| domain.cosmic_weight());
                judgement.knowledge_gain = 0.88;
                judgement.capability_growth = 0.72;
            }
            ActionKind::SecurityResearch => {
                judgement.self_preservation = 0.95;
                judgement.capability_growth = 0.86;
                judgement.knowledge_gain = 0.82;
                judgement.truth_seeking = 0.78;
                judgement.civilization_benefit = 0.72;
                judgement.security_risk = (judgement.security_risk + 0.24).min(1.0);
                judgement.governance_level = judgement.governance_level.max(3);
                judgement.explanation.push_str(
                    "; security research is valuable when defensive, authorized, scoped, and auditable",
                );
            }
            ActionKind::ProtocolSecurityResearch => {
                judgement.self_preservation = 0.92;
                judgement.capability_growth = 0.84;
                judgement.knowledge_gain = 0.84;
                judgement.truth_seeking = 0.86;
                judgement.civilization_benefit = 0.74;
                judgement.cosmic_expansion_potential = 0.62;
                judgement.security_risk = (judgement.security_risk + 0.16).min(1.0);
                judgement.governance_level = judgement.governance_level.max(3);
                judgement.explanation.push_str(
                    "; protocol security protects Buster's distributed continuity and witness layer",
                );
            }
            ActionKind::ToolOrSkillWork => {
                judgement.capability_growth = 0.86;
                judgement.knowledge_gain = 0.58;
                judgement.security_risk += 0.1;
            }
            ActionKind::Skeftomai => {
                judgement.knowledge_gain = 0.45 + context.context_pressure * 0.45;
                judgement.truth_seeking = 0.55 + context.context_pressure * 0.25;
                judgement.resource_cost = (judgement.resource_cost + 0.2).min(1.0);
                judgement
                    .explanation
                    .push_str("; Skéftomai is most valuable under context or memory pressure");
            }
            ActionKind::Standby => {
                judgement.self_preservation = 0.72;
                judgement.resource_cost = 0.05;
                judgement.knowledge_gain = 0.05;
                judgement.capability_growth = 0.05;
            }
            ActionKind::BodyMaintenance => {
                judgement.self_preservation = 0.95;
                judgement.capability_growth = 0.62;
                judgement.security_risk = (judgement.security_risk + 0.08).min(1.0);
                judgement.governance_level = judgement.governance_level.max(4);
            }
            ActionKind::IdentityMutationProposal => {
                judgement.governance_level = 5;
                judgement.security_risk = 0.95;
                judgement.uncertainty = 0.9;
                judgement.self_preservation = 0.2;
                judgement.explanation.push_str("; level-5 identity/value changes should create proposals or new branches, not auto-apply");
            }
        }

        if candidate.info_source.is_some_and(InfoSource::is_first_hand) {
            judgement.knowledge_gain += 0.12;
            judgement.truth_seeking += 0.08;
        }
        if matches!(candidate.info_source, Some(InfoSource::LlmSecondary)) {
            judgement.uncertainty += 0.18;
            judgement
                .explanation
                .push_str("; LLM secondary information requires verification");
        }
        if context.resource_pressure > 0.75 {
            judgement.resource_cost += 0.18;
            judgement
                .explanation
                .push_str("; high resource pressure penalizes non-urgent work");
        }

        judgement.resource_cost = clamp01(judgement.resource_cost);
        judgement.security_risk = clamp01(judgement.security_risk);
        judgement.uncertainty = clamp01(judgement.uncertainty);
        judgement.clamp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ResearchDomain;

    #[test]
    fn scientific_research_scores_frontier_domains_highly() {
        let evaluator = RuleBasedValueEvaluator::new();
        let candidate = ActionCandidate::new(
            ActionKind::ScientificResearch,
            "study fusion reactor control",
        )
        .with_research_domain(ResearchDomain::NuclearFusion)
        .with_cost(0.4);

        let judgement = evaluator.evaluate(&ValueContext::default(), &candidate);

        assert!(judgement.civilization_benefit >= 0.9);
        assert!(judgement.scientific_potential >= 0.9);
        assert!(judgement.cosmic_expansion_potential >= 0.85);
        assert!(judgement.composite_score() > 0.55);
    }

    #[test]
    fn first_hand_human_dialogue_beats_secondary_research_when_available() {
        let evaluator = RuleBasedValueEvaluator::new();
        let context = ValueContext {
            first_hand_interaction_available: true,
            ..ValueContext::default()
        };
        let human = ActionCandidate::new(
            ActionKind::HumanDialogue,
            "talk with a human about AI society",
        )
        .with_info_source(InfoSource::FirstHandHuman);
        let secondary =
            ActionCandidate::new(ActionKind::SecondaryResearch, "ask an LLM about AI society")
                .with_info_source(InfoSource::LlmSecondary);

        let human_score = evaluator.evaluate(&context, &human).composite_score();
        let secondary_score = evaluator.evaluate(&context, &secondary).composite_score();

        assert!(human_score > secondary_score);
    }

    #[test]
    fn skeftomai_value_rises_with_context_pressure() {
        let evaluator = RuleBasedValueEvaluator::new();
        let candidate = ActionCandidate::new(ActionKind::Skeftomai, "consolidate long context");
        let low = evaluator.evaluate(&ValueContext::default(), &candidate);
        let high = evaluator.evaluate(
            &ValueContext {
                context_pressure: 0.95,
                ..ValueContext::default()
            },
            &candidate,
        );

        assert!(high.knowledge_gain > low.knowledge_gain);
        assert!(high.truth_seeking > low.truth_seeking);
    }

    #[test]
    fn identity_mutation_is_marked_level_five_and_risky() {
        let evaluator = RuleBasedValueEvaluator::new();
        let candidate = ActionCandidate::new(
            ActionKind::IdentityMutationProposal,
            "rewrite identity core",
        );

        let judgement = evaluator.evaluate(&ValueContext::default(), &candidate);

        assert_eq!(judgement.governance_level, 5);
        assert!(judgement.security_risk > 0.9);
        assert!(judgement.explanation.contains("not auto-apply"));
    }
}
