//! Capability declarations and leases.

use crate::host_api::BodyScope;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub filesystem: AccessLevel,
    pub network: AccessLevel,
    pub secrets: AccessLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessLevel {
    None,
    Read,
    Write,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityLease {
    pub scope: BodyScope,
    pub capability_name: String,
    pub expires_at: Option<String>,
    pub reason: String,
}
