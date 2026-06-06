//! Buster's body layer: boundaries, immune system, resources, secrets, runtime, and LLM access.

pub mod audit;
pub mod capabilities;
pub mod cli_tool;
pub mod controller;
pub mod gate;
pub mod host_api;
pub mod ironclaw_secret_backend;
pub mod llm;
pub mod local_secret_store;
pub mod master_key;
pub mod network;
pub mod persistence;
pub mod policy;
pub mod process_manager;
pub mod registry;
pub mod repair;
pub mod resources;
pub mod responder;
pub mod runtime;
pub mod safety;
pub mod script_sandbox;
pub mod secrets;
pub mod sensors;
pub mod state;
pub mod supervisor;

pub use audit::{AuditEvent, AuditLevel};
pub use capabilities::{Capability, CapabilityLease};
pub use cli_tool::{CliToolBackend, CliToolCommand, CliToolError};
pub use controller::{BodyController, BodyRuntimePaths, BodyRuntimeWriter, ReconcileReport};
pub use gate::{BodyGate, BodyGateError, GatePath, MemoryWriteIntent, SkillChangeIntent};
pub use host_api::{BodyScope, ChangeLevel, RiskLine};
pub use ironclaw_secret_backend::{
    IronClawSecretBackend, IronClawSecretBackendError, IronClawSecretResolver,
};
pub use local_secret_store::{LocalEncryptedSecretStore, LocalSecretStoreError};
pub use master_key::{
    default_master_key, EnvMasterKeyProvider, MasterKeyError, MasterKeyProvider,
    WindowsCredentialManagerBridge,
};
pub use network::{
    BrokeredHttpToolBackend, EgressBroker, EgressError, EgressRequest, EgressResponse,
    NetworkPolicy,
};
pub use persistence::{BodyStore, BodyStoreError, JsonlBodyStore, SqliteBodyStore};
pub use process_manager::{
    ProcessManager, ProcessManagerError, ProcessSnapshot, ProcessSpawnRequest,
};
pub use registry::{ManagedUnit, UnitKind, UnitRegistry, UnitStatus};
pub use repair::{AuthorityFileRepairer, RepairOutcome};
pub use resources::{ResourceBudget, ResourceUsage};
pub use script_sandbox::{
    ScriptSandbox, ScriptSandboxConfig, ScriptSandboxError, ScriptSandboxRun,
};
pub use secrets::{
    InMemorySecretBackend, SecretBackend, SecretBroker, SecretBrokerError, SecretClass,
    SecretDecision, SecretDenyReason, SecretFingerprint, SecretHandle, SecretLease, SecretRecord,
    SecretRedactor, SecretRegistry,
};
pub use sensors::{
    AuthorityFileSensor, BodySensor, ExternalScanFinding, ExternalScanSensor, ExternalScannerKind,
    NetworkEgressSensor, NetworkObservation, OutputObservation, OwnedUnitSensor,
    SecretOutputSensor, SensorSignal, SensorSuite, SignalKind, SignalSeverity, StaticSignalSensor,
};
pub use state::{BodyMode, CurrentBodyState, DesiredBodyState};
pub use supervisor::{BodySupervisor, SupervisorConfig, SupervisorTick};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyStatus {
    pub immune_state: ImmuneState,
    pub active_leases: usize,
    pub open_audit_events: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImmuneState {
    Normal,
    Watchful,
    Quarantined,
    DegradedIdentity,
}
