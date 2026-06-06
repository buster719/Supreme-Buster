//! Body gate that wires real execution paths through layer-4 controls.
//!
//! This is the choke point between Buster's cognitive/tool layers and concrete
//! side effects. LLM calls, tool execution, network preflight, secret leases,
//! memory writes, and skill changes should pass through this gate so the body
//! can enforce leases and persist redacted audit records.

use std::fmt;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::audit::{AuditEvent, AuditLevel};
use crate::host_api::{BodyScope, ChangeLevel, RiskLine};
use crate::network::{EgressBroker, EgressError};
use crate::persistence::{BodyStore, BodyStoreError};
use crate::runtime::{BodyRuntime, RuntimeBackend, RuntimeOutcome, RuntimePolicy, RuntimeRequest};
use crate::secrets::{SecretDenyReason, SecretHandle, SecretLease, SecretRegistry};
use crate::state::BodyMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatePath {
    LlmCall,
    ToolExecution,
    NetworkEgress,
    SecretLease,
    MemoryWrite,
    SkillChange,
}

impl GatePath {
    fn summary_name(self) -> &'static str {
        match self {
            Self::LlmCall => "llm call",
            Self::ToolExecution => "tool execution",
            Self::NetworkEgress => "network egress",
            Self::SecretLease => "secret lease",
            Self::MemoryWrite => "memory write",
            Self::SkillChange => "skill change",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryWriteIntent {
    pub kind: String,
    pub source: String,
    pub content_len: usize,
    pub content_hash: String,
}

impl MemoryWriteIntent {
    pub fn new(kind: impl Into<String>, source: impl Into<String>, content: &str) -> Self {
        Self {
            kind: kind.into(),
            source: source.into(),
            content_len: content.len(),
            content_hash: sha256_hex(content),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillChangeIntent {
    pub skill_name: String,
    pub action: String,
    pub status: String,
    pub manifest_hash: Option<String>,
}

impl SkillChangeIntent {
    pub fn new(
        skill_name: impl Into<String>,
        action: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            skill_name: skill_name.into(),
            action: action.into(),
            status: status.into(),
            manifest_hash: None,
        }
    }

    pub fn with_manifest(mut self, manifest: &str) -> Self {
        self.manifest_hash = Some(sha256_hex(manifest));
        self
    }
}

#[derive(Debug)]
pub enum BodyGateError {
    Store(BodyStoreError),
    Network(EgressError),
    Secret(SecretDenyReason),
}

impl fmt::Display for BodyGateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "body gate store error: {error}"),
            Self::Network(error) => write!(formatter, "body gate network error: {error}"),
            Self::Secret(reason) => write!(formatter, "body gate secret denied: {reason:?}"),
        }
    }
}

impl std::error::Error for BodyGateError {}

impl From<BodyStoreError> for BodyGateError {
    fn from(error: BodyStoreError) -> Self {
        Self::Store(error)
    }
}

impl From<EgressError> for BodyGateError {
    fn from(error: EgressError) -> Self {
        Self::Network(error)
    }
}

#[derive(Debug)]
pub struct BodyGate<S> {
    store: S,
    mode: BodyMode,
}

impl<S> BodyGate<S>
where
    S: BodyStore,
{
    pub fn new(store: S, mode: BodyMode) -> Self {
        Self { store, mode }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn mode(&self) -> BodyMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: BodyMode) {
        self.mode = mode;
    }

    pub fn invoke_runtime<B>(
        &self,
        request: RuntimeRequest,
        policy: &RuntimePolicy,
        backend: B,
    ) -> Result<RuntimeOutcome, BodyGateError>
    where
        B: RuntimeBackend,
    {
        let outcome = BodyRuntime::new(backend).invoke(request, policy);
        self.store.append_audit_event(outcome_audit(&outcome))?;
        Ok(outcome)
    }

    pub fn record_tool_action<T>(&self, record: &T) -> Result<(), BodyGateError>
    where
        T: Serialize,
    {
        self.store.append_tool_action(record)?;
        self.store.append_audit_event(&audit_event(
            AuditLevel::Info,
            GatePath::ToolExecution,
            RiskLine::Yellow,
            "tool action audit persisted".to_string(),
            None,
        ))?;
        Ok(())
    }

    pub fn preflight_network(
        &self,
        broker: &EgressBroker,
        url: &str,
    ) -> Result<String, BodyGateError> {
        match broker.preflight_url(url) {
            Ok(host) => {
                self.store.append_audit_event(&audit_event(
                    AuditLevel::Info,
                    GatePath::NetworkEgress,
                    RiskLine::Yellow,
                    format!("network egress allowed for host {host}"),
                    None,
                ))?;
                Ok(host)
            }
            Err(error) => {
                self.store.append_audit_event(&audit_event(
                    AuditLevel::Warning,
                    GatePath::NetworkEgress,
                    RiskLine::Red,
                    "network egress blocked".to_string(),
                    Some(error.to_string()),
                ))?;
                Err(BodyGateError::Network(error))
            }
        }
    }

    pub fn issue_secret_lease(
        &self,
        registry: &mut SecretRegistry,
        handle: &SecretHandle,
        capability: impl Into<String>,
        ttl_secs: Option<u64>,
        reason: impl Into<String>,
    ) -> Result<SecretLease, BodyGateError> {
        let capability = capability.into();
        let reason = reason.into();
        match registry.issue_lease(handle, capability.clone(), ttl_secs, reason, self.mode) {
            Ok(lease) => {
                self.store.append_audit_event(&audit_event(
                    AuditLevel::Info,
                    GatePath::SecretLease,
                    RiskLine::Yellow,
                    format!("secret lease issued for capability {capability}"),
                    Some(format!("lease_id={}", lease.lease_id)),
                ))?;
                Ok(lease)
            }
            Err(reason) => {
                self.store.append_audit_event(&audit_event(
                    AuditLevel::Warning,
                    GatePath::SecretLease,
                    RiskLine::Red,
                    format!("secret lease denied for capability {capability}"),
                    Some(format!("{reason:?}")),
                ))?;
                Err(BodyGateError::Secret(reason))
            }
        }
    }

    pub fn record_memory_write(
        &self,
        scope: &BodyScope,
        intent: MemoryWriteIntent,
    ) -> Result<(), BodyGateError> {
        self.store.append_audit_event(&audit_event(
            AuditLevel::Info,
            GatePath::MemoryWrite,
            RiskLine::Yellow,
            format!("memory write requested for {}", intent.kind),
            Some(format!(
                "agent={} branch={} task={} source={} len={} sha256={}",
                scope.agent_id,
                scope.branch_id,
                scope.task_id,
                intent.source,
                intent.content_len,
                intent.content_hash
            )),
        ))?;
        Ok(())
    }

    pub fn record_skill_change(&self, intent: SkillChangeIntent) -> Result<(), BodyGateError> {
        self.store.append_audit_event(&audit_event(
            AuditLevel::Info,
            GatePath::SkillChange,
            RiskLine::Yellow,
            format!("skill {} {}", intent.skill_name, intent.action),
            Some(format!(
                "status={} manifest_hash={}",
                intent.status,
                intent.manifest_hash.unwrap_or_else(|| "none".to_string())
            )),
        ))?;
        Ok(())
    }
}

fn outcome_audit(outcome: &RuntimeOutcome) -> &AuditEvent {
    match outcome {
        RuntimeOutcome::Completed(completion) => &completion.audit,
        RuntimeOutcome::Blocked(block) => &block.audit,
        RuntimeOutcome::Failed(failure) => &failure.audit,
    }
}

fn audit_event(
    level: AuditLevel,
    path: GatePath,
    risk_line: RiskLine,
    summary: String,
    consequence: Option<String>,
) -> AuditEvent {
    AuditEvent {
        level,
        change_level: ChangeLevel::BoundaryResource,
        risk_line,
        summary: format!("body gate {}: {summary}", path.summary_name()),
        consequence,
    }
}

fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let bytes = hasher.finalize();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::CapabilityLease;
    use crate::network::NetworkPolicy;
    use crate::persistence::SqliteBodyStore;
    use crate::resources::ResourceBudget;
    use crate::runtime::{EchoBackend, RuntimeKind};
    use crate::secrets::{SecretClass, SecretRecord};

    fn scope() -> BodyScope {
        BodyScope::new("buster", "main", "gate-test")
    }

    fn lease(capability: &str) -> CapabilityLease {
        CapabilityLease {
            scope: scope(),
            capability_name: capability.to_string(),
            expires_at: None,
            reason: "gate test".to_string(),
        }
    }

    #[test]
    fn gate_invokes_runtime_and_persists_audit() {
        let store = SqliteBodyStore::open_in_memory().unwrap();
        let gate = BodyGate::new(store, BodyMode::Normal);
        let request = RuntimeRequest::new(
            scope(),
            RuntimeKind::ExternalLlm,
            "llm.think",
            "hello buster",
        );
        let policy = RuntimePolicy::deny_by_default(ResourceBudget::default())
            .with_lease(lease("llm.think"));

        let outcome = gate
            .invoke_runtime(request, &policy, EchoBackend::new(RuntimeKind::ExternalLlm))
            .unwrap();

        assert!(matches!(outcome, RuntimeOutcome::Completed(_)));
        assert_eq!(gate.store().audit_event_count().unwrap(), 1);
    }

    #[test]
    fn gate_blocks_network_preflight_and_persists_audit() {
        let store = SqliteBodyStore::open_in_memory().unwrap();
        let gate = BodyGate::new(store, BodyMode::Normal);
        let broker = EgressBroker::new(NetworkPolicy {
            allowed_hosts: vec!["openrouter.ai".to_string()],
            deny_private_networks: true,
            max_response_bytes: Some(1024),
        });

        let error = gate
            .preflight_network(&broker, "https://evil.example/api")
            .unwrap_err();

        assert!(matches!(error, BodyGateError::Network(_)));
        assert_eq!(gate.store().audit_event_count().unwrap(), 1);
    }

    #[test]
    fn gate_issues_secret_lease_through_body_mode() {
        let store = SqliteBodyStore::open_in_memory().unwrap();
        let gate = BodyGate::new(store, BodyMode::Normal);
        let mut registry = SecretRegistry::new();
        let handle = SecretHandle("openrouter_api_key".to_string());
        registry.register(SecretRecord::new(
            handle.clone(),
            SecretClass::LlmApiKey,
            vec!["llm.openrouter".to_string()],
            "OpenRouter API key",
        ));

        let lease = gate
            .issue_secret_lease(
                &mut registry,
                &handle,
                "llm.openrouter",
                Some(60),
                "cognitive call",
            )
            .unwrap();

        assert_eq!(lease.capability, "llm.openrouter");
        assert_eq!(gate.store().audit_event_count().unwrap(), 1);
    }

    #[test]
    fn gate_records_memory_and_skill_control_events_without_plain_content() {
        let store = SqliteBodyStore::open_in_memory().unwrap();
        let gate = BodyGate::new(store, BodyMode::Normal);

        gate.record_memory_write(
            &scope(),
            MemoryWriteIntent::new("fact", "test", "Buster learned a private detail"),
        )
        .unwrap();
        gate.record_skill_change(
            SkillChangeIntent::new("feishu-bridge", "install", "draft")
                .with_manifest("name: feishu-bridge\nsecret: no"),
        )
        .unwrap();

        assert_eq!(gate.store().audit_event_count().unwrap(), 2);
        let raw: String = gate
            .store()
            .connection()
            .query_row(
                "SELECT event_json FROM body_audit_events ORDER BY id LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!raw.contains("private detail"));
    }
}
