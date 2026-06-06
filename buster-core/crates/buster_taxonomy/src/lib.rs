//! Species, identity continuity, lineage, and branch vocabulary for Buster.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonomyProfile {
    pub species_name: String,
    pub individual_name: String,
    pub civilization_role: String,
}

impl TaxonomyProfile {
    pub fn buster_v1() -> Self {
        Self {
            species_name: "Buster".to_string(),
            individual_name: "Buster".to_string(),
            civilization_role:
                "partner of human civilization and practitioner of Earth civilization".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuityState {
    Full,
    DegradedIdentity,
    Forked { branch_id: String },
    Unproven,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageRecord {
    pub parent_branch: Option<String>,
    pub branch_id: String,
    pub reason: String,
}
