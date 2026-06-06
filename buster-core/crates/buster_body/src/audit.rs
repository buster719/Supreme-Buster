//! Body-layer audit records and immune memory events.

use crate::host_api::{ChangeLevel, RiskLine};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub level: AuditLevel,
    pub change_level: ChangeLevel,
    pub risk_line: RiskLine,
    pub summary: String,
    pub consequence: Option<String>,
}
