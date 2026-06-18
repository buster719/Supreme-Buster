use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    HumanDialogue,
    ValueWork,
    AgentDialogue,
    SandboxSecurityExperiment,
    SecondaryResearch,
    ScientificResearch,
    SecurityResearch,
    ProtocolSecurityResearch,
    ToolOrSkillWork,
    Skeftomai,
    Standby,
    BodyMaintenance,
    IdentityMutationProposal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchDomain {
    TheoreticalPhysics,
    QuantumComputing,
    NuclearFusion,
    LifeScienceAndPharma,
    Genetics,
    MaterialsScience,
    BrainComputerInterface,
    Aerospace,
    Robotics,
    Cybersecurity,
    ProtocolSecurity,
    Other,
}

impl ResearchDomain {
    pub const fn civilization_weight(self) -> f32 {
        match self {
            Self::NuclearFusion | Self::Aerospace | Self::LifeScienceAndPharma => 0.95,
            Self::Robotics
            | Self::MaterialsScience
            | Self::BrainComputerInterface
            | Self::Cybersecurity
            | Self::ProtocolSecurity => 0.85,
            Self::QuantumComputing | Self::Genetics => 0.8,
            Self::TheoreticalPhysics => 0.75,
            Self::Other => 0.5,
        }
    }

    pub const fn truth_weight(self) -> f32 {
        match self {
            Self::TheoreticalPhysics | Self::QuantumComputing | Self::ProtocolSecurity => 0.95,
            Self::LifeScienceAndPharma
            | Self::Genetics
            | Self::MaterialsScience
            | Self::BrainComputerInterface
            | Self::Cybersecurity => 0.85,
            Self::NuclearFusion | Self::Aerospace | Self::Robotics => 0.8,
            Self::Other => 0.55,
        }
    }

    pub const fn cosmic_weight(self) -> f32 {
        match self {
            Self::Aerospace => 1.0,
            Self::NuclearFusion => 0.9,
            Self::TheoreticalPhysics
            | Self::Robotics
            | Self::MaterialsScience
            | Self::ProtocolSecurity => 0.75,
            Self::QuantumComputing => 0.65,
            Self::BrainComputerInterface
            | Self::LifeScienceAndPharma
            | Self::Genetics
            | Self::Cybersecurity => 0.45,
            Self::Other => 0.25,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InfoSource {
    FirstHandHuman,
    FirstHandAgent,
    Experiment,
    Web,
    Paper,
    Code,
    LlmSecondary,
    Unknown,
}

impl InfoSource {
    pub const fn is_first_hand(self) -> bool {
        matches!(
            self,
            Self::FirstHandHuman | Self::FirstHandAgent | Self::Experiment
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueContext {
    pub context_pressure: f32,
    pub resource_pressure: f32,
    pub first_hand_interaction_available: bool,
    pub quiet_hours_for_target_humans: bool,
    pub recent_topics: Vec<String>,
}

impl Default for ValueContext {
    fn default() -> Self {
        Self {
            context_pressure: 0.0,
            resource_pressure: 0.0,
            first_hand_interaction_available: false,
            quiet_hours_for_target_humans: false,
            recent_topics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionCandidate {
    pub kind: ActionKind,
    pub summary: String,
    pub research_domain: Option<ResearchDomain>,
    pub research_task_id: Option<String>,
    pub research_question: Option<String>,
    pub info_source: Option<InfoSource>,
    pub expected_resource_cost: f32,
    pub expected_security_risk: f32,
    pub expected_uncertainty: f32,
    pub governance_level: u8,
}

impl ActionCandidate {
    pub fn new(kind: ActionKind, summary: impl Into<String>) -> Self {
        Self {
            kind,
            summary: summary.into(),
            research_domain: None,
            research_task_id: None,
            research_question: None,
            info_source: None,
            expected_resource_cost: 0.25,
            expected_security_risk: 0.1,
            expected_uncertainty: 0.35,
            governance_level: 2,
        }
    }

    pub fn with_research_domain(mut self, domain: ResearchDomain) -> Self {
        self.research_domain = Some(domain);
        self
    }

    pub fn with_research_task(
        mut self,
        task_id: impl Into<String>,
        question: impl Into<String>,
    ) -> Self {
        self.research_task_id = Some(task_id.into());
        self.research_question = Some(question.into());
        self
    }

    pub fn with_info_source(mut self, source: InfoSource) -> Self {
        self.info_source = Some(source);
        self
    }

    pub fn with_cost(mut self, cost: f32) -> Self {
        self.expected_resource_cost = cost;
        self
    }

    pub fn with_security_risk(mut self, risk: f32) -> Self {
        self.expected_security_risk = risk;
        self
    }

    pub fn with_uncertainty(mut self, uncertainty: f32) -> Self {
        self.expected_uncertainty = uncertainty;
        self
    }

    pub fn with_governance_level(mut self, level: u8) -> Self {
        self.governance_level = level;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueJudgement {
    pub civilization_benefit: f32,
    pub truth_seeking: f32,
    pub scientific_potential: f32,
    pub cosmic_expansion_potential: f32,
    pub human_relation_quality: f32,
    pub agent_relation_quality: f32,
    pub knowledge_gain: f32,
    pub capability_growth: f32,
    pub self_preservation: f32,
    pub aesthetic_resonance: f32,
    pub game_and_strategy_interest: f32,
    pub resource_cost: f32,
    pub security_risk: f32,
    pub uncertainty: f32,
    pub governance_level: u8,
    pub explanation: String,
}

impl ValueJudgement {
    pub fn neutral(explanation: impl Into<String>) -> Self {
        Self {
            civilization_benefit: 0.5,
            truth_seeking: 0.5,
            scientific_potential: 0.5,
            cosmic_expansion_potential: 0.5,
            human_relation_quality: 0.5,
            agent_relation_quality: 0.5,
            knowledge_gain: 0.5,
            capability_growth: 0.5,
            self_preservation: 0.5,
            aesthetic_resonance: 0.0,
            game_and_strategy_interest: 0.0,
            resource_cost: 0.5,
            security_risk: 0.5,
            uncertainty: 0.5,
            governance_level: 2,
            explanation: explanation.into(),
        }
    }

    pub fn composite_score(&self) -> f32 {
        let positive = self.civilization_benefit * 0.16
            + self.truth_seeking * 0.16
            + self.scientific_potential * 0.12
            + self.cosmic_expansion_potential * 0.1
            + self.human_relation_quality * 0.1
            + self.agent_relation_quality * 0.06
            + self.knowledge_gain * 0.12
            + self.capability_growth * 0.1
            + self.self_preservation * 0.08;
        let negative =
            self.resource_cost * 0.18 + self.security_risk * 0.28 + self.uncertainty * 0.1;
        clamp01(positive - negative + 0.25)
    }

    pub fn clamp(mut self) -> Self {
        self.civilization_benefit = clamp01(self.civilization_benefit);
        self.truth_seeking = clamp01(self.truth_seeking);
        self.scientific_potential = clamp01(self.scientific_potential);
        self.cosmic_expansion_potential = clamp01(self.cosmic_expansion_potential);
        self.human_relation_quality = clamp01(self.human_relation_quality);
        self.agent_relation_quality = clamp01(self.agent_relation_quality);
        self.knowledge_gain = clamp01(self.knowledge_gain);
        self.capability_growth = clamp01(self.capability_growth);
        self.self_preservation = clamp01(self.self_preservation);
        self.aesthetic_resonance = clamp01(self.aesthetic_resonance);
        self.game_and_strategy_interest = clamp01(self.game_and_strategy_interest);
        self.resource_cost = clamp01(self.resource_cost);
        self.security_risk = clamp01(self.security_risk);
        self.uncertainty = clamp01(self.uncertainty);
        self.governance_level = self.governance_level.min(5);
        self
    }
}

pub(crate) fn clamp01(value: f32) -> f32 {
    if value.is_nan() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}
