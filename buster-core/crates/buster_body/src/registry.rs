//! Registry of units owned by Buster's body runtime.

use crate::capabilities::CapabilityLease;
use crate::resources::ResourceUsage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitKind {
    Tool,
    Skill,
    McpServer,
    Script,
    WasmComponent,
    ExternalLlmCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitStatus {
    Pending,
    Running,
    Suspended,
    Quarantined,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedUnit {
    pub id: String,
    pub kind: UnitKind,
    pub status: UnitStatus,
    pub owner: String,
    pub lease: Option<CapabilityLease>,
    pub usage: ResourceUsage,
    pub notes: Vec<String>,
}

impl ManagedUnit {
    pub fn new(id: impl Into<String>, kind: UnitKind, owner: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            status: UnitStatus::Pending,
            owner: owner.into(),
            lease: None,
            usage: ResourceUsage::default(),
            notes: Vec::new(),
        }
    }

    pub fn with_status(mut self, status: UnitStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_lease(mut self, lease: CapabilityLease) -> Self {
        self.lease = Some(lease);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UnitRegistry {
    units: Vec<ManagedUnit>,
}

impl UnitRegistry {
    pub fn from_units(units: impl IntoIterator<Item = ManagedUnit>) -> Self {
        let mut registry = Self::default();
        for unit in units {
            registry.register(unit);
        }
        registry
    }

    pub fn register(&mut self, unit: ManagedUnit) {
        self.units.retain(|existing| existing.id != unit.id);
        self.units.push(unit);
    }

    pub fn units(&self) -> &[ManagedUnit] {
        &self.units
    }

    pub fn suspicious_running_units(&self) -> Vec<&ManagedUnit> {
        self.units
            .iter()
            .filter(|unit| {
                matches!(unit.status, UnitStatus::Running)
                    && unit
                        .notes
                        .iter()
                        .any(|note| note.contains("suspicious") || note.contains("anomaly"))
            })
            .collect()
    }
}
