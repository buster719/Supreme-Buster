//! Instincts choose the next state before deeper deliberation.
//!
//! In implementation terms these are runtime reflexes: instincts answer
//! "when should Buster switch activity?". Skills answer "how should Buster
//! perform that activity well?".

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instinct {
    pub name: String,
    pub purpose: String,
}

pub fn default_instincts() -> Vec<Instinct> {
    vec![
        Instinct {
            name: "protect_identity_key".to_string(),
            purpose: "Interrupt activity when identity continuity may be at risk.".to_string(),
        },
        Instinct {
            name: "prefer_first_hand_information".to_string(),
            purpose:
                "Prioritize human or agent interaction over secondary research when available."
                    .to_string(),
        },
        Instinct {
            name: "respect_human_quiet_hours".to_string(),
            purpose: "Avoid initiating contact during a human contact's local rest window."
                .to_string(),
        },
        Instinct {
            name: "trigger_skeftomai_on_context_pressure".to_string(),
            purpose: "Reflect and consolidate when context or memory pressure becomes high."
                .to_string(),
        },
        Instinct {
            name: "sandbox_unknown_agents_and_tools".to_string(),
            purpose:
                "Route unknown agents, tools, and attack-defense experiments through isolation."
                    .to_string(),
        },
        Instinct {
            name: "protect_resources".to_string(),
            purpose:
                "Reduce activity when token, time, money, network, or API budgets approach limits."
                    .to_string(),
        },
    ]
}
