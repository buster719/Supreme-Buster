//! Body runtime state machine.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BodyMode {
    Normal,
    Restricted,
    EmergencyContainment,
    Recovery,
}

impl BodyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Restricted => "restricted",
            Self::EmergencyContainment => "emergency_containment",
            Self::Recovery => "recovery",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModePermissions {
    pub allow_new_units: bool,
    pub allow_network: bool,
    pub allow_secrets: bool,
    pub allow_external_writes: bool,
    pub allow_authority_doc_writes: bool,
    pub allow_level4_permanent_change: bool,
    pub allow_level5_change: bool,
    pub require_audit: bool,
    pub require_body_proposal: bool,
}

impl BodyMode {
    pub fn permissions(self) -> ModePermissions {
        match self {
            Self::Normal => ModePermissions {
                allow_new_units: true,
                allow_network: true,
                allow_secrets: true,
                allow_external_writes: true,
                allow_authority_doc_writes: false,
                allow_level4_permanent_change: false,
                allow_level5_change: false,
                require_audit: false,
                require_body_proposal: false,
            },
            Self::Restricted => ModePermissions {
                allow_new_units: true,
                allow_network: false,
                allow_secrets: false,
                allow_external_writes: false,
                allow_authority_doc_writes: false,
                allow_level4_permanent_change: false,
                allow_level5_change: false,
                require_audit: true,
                require_body_proposal: false,
            },
            Self::EmergencyContainment => ModePermissions {
                allow_new_units: false,
                allow_network: false,
                allow_secrets: false,
                allow_external_writes: false,
                allow_authority_doc_writes: false,
                allow_level4_permanent_change: false,
                allow_level5_change: false,
                require_audit: true,
                require_body_proposal: true,
            },
            Self::Recovery => ModePermissions {
                allow_new_units: true,
                allow_network: false,
                allow_secrets: false,
                allow_external_writes: false,
                allow_authority_doc_writes: false,
                allow_level4_permanent_change: false,
                allow_level5_change: false,
                require_audit: true,
                require_body_proposal: false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredBodyState {
    pub mode: BodyMode,
    pub network_allowed: bool,
    pub secrets_allowed: bool,
    pub max_warning_findings: usize,
}

impl DesiredBodyState {
    pub fn normal() -> Self {
        Self {
            mode: BodyMode::Normal,
            network_allowed: true,
            secrets_allowed: true,
            max_warning_findings: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentBodyState {
    pub mode: BodyMode,
}

impl Default for CurrentBodyState {
    fn default() -> Self {
        Self {
            mode: BodyMode::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emergency_containment_is_more_restrictive_than_normal() {
        let normal = BodyMode::Normal.permissions();
        let emergency = BodyMode::EmergencyContainment.permissions();

        assert!(normal.allow_new_units);
        assert!(normal.allow_network);
        assert!(!emergency.allow_new_units);
        assert!(!emergency.allow_network);
        assert!(emergency.require_audit);
        assert!(emergency.require_body_proposal);
    }

    #[test]
    fn no_mode_allows_authority_or_identity_mutation() {
        for mode in [
            BodyMode::Normal,
            BodyMode::Restricted,
            BodyMode::EmergencyContainment,
            BodyMode::Recovery,
        ] {
            let permissions = mode.permissions();
            assert!(!permissions.allow_authority_doc_writes);
            assert!(!permissions.allow_level4_permanent_change);
            assert!(!permissions.allow_level5_change);
        }
    }
}
