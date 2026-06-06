//! Resource budgets and usage accounting.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub max_tokens: Option<u64>,
    pub max_wall_clock_ms: Option<u64>,
    pub max_network_bytes: Option<u64>,
    pub max_usd: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub tokens: u64,
    pub wall_clock_ms: u64,
    pub network_bytes: u64,
    pub usd: f64,
}

impl ResourceBudget {
    pub fn allows(&self, usage: &ResourceUsage) -> bool {
        self.max_tokens.map_or(true, |max| usage.tokens <= max)
            && self
                .max_wall_clock_ms
                .map_or(true, |max| usage.wall_clock_ms <= max)
            && self
                .max_network_bytes
                .map_or(true, |max| usage.network_bytes <= max)
            && self.max_usd.map_or(true, |max| usage.usd <= max)
    }
}
