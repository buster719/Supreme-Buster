//! Runtime lanes for tools, scripts, WASM components, MCP servers, and external LLM calls.
//!
//! This is not a full virtual machine yet. It is Buster's first host runtime:
//! a boundary that classifies risk, checks leases and budgets, dispatches to a
//! concrete backend, and emits audit events. WASM, containers, or micro-VMs can
//! later become concrete backends behind this contract.

use crate::audit::{AuditEvent, AuditLevel};
use crate::capabilities::CapabilityLease;
use crate::host_api::{BodyScope, ChangeLevel, RiskLine};
use crate::resources::{ResourceBudget, ResourceUsage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Wasm,
    Script,
    Mcp,
    Tool,
    ExternalLlm,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeRequest {
    pub scope: BodyScope,
    pub runtime: RuntimeKind,
    pub capability: String,
    pub change_level: ChangeLevel,
    pub risk_line: RiskLine,
    pub input_summary: String,
    pub requires_network: bool,
    pub requires_secret: bool,
    pub estimated_usage: ResourceUsage,
}

impl RuntimeRequest {
    pub fn new(
        scope: BodyScope,
        runtime: RuntimeKind,
        capability: impl Into<String>,
        input_summary: impl Into<String>,
    ) -> Self {
        Self {
            scope,
            runtime,
            capability: capability.into(),
            change_level: ChangeLevel::CapabilityExecution,
            risk_line: RiskLine::Green,
            input_summary: input_summary.into(),
            requires_network: false,
            requires_secret: false,
            estimated_usage: ResourceUsage::default(),
        }
    }

    pub fn with_risk(mut self, change_level: ChangeLevel, risk_line: RiskLine) -> Self {
        self.change_level = change_level;
        self.risk_line = risk_line;
        self
    }

    pub fn with_network(mut self) -> Self {
        self.requires_network = true;
        self
    }

    pub fn with_secret(mut self) -> Self {
        self.requires_secret = true;
        self
    }

    pub fn with_estimated_usage(mut self, usage: ResourceUsage) -> Self {
        self.estimated_usage = usage;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeOutcome {
    Completed(RuntimeCompletion),
    Blocked(RuntimeBlock),
    Failed(RuntimeFailure),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCompletion {
    pub output_summary: String,
    pub actual_usage: ResourceUsage,
    pub audit: AuditEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBlock {
    pub reason: BlockReason,
    pub audit: AuditEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFailure {
    pub reason: String,
    pub audit: AuditEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    RedLine,
    MissingCapabilityLease,
    SecretDenied,
    NetworkDenied,
    BudgetExceeded,
    BackendUnavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePolicy {
    pub budget: ResourceBudget,
    pub allow_yellow_line: bool,
    pub allow_red_line: bool,
    pub allow_network: bool,
    pub allow_secret: bool,
    pub leases: Vec<CapabilityLease>,
}

impl RuntimePolicy {
    pub fn deny_by_default(budget: ResourceBudget) -> Self {
        Self {
            budget,
            allow_yellow_line: true,
            allow_red_line: false,
            allow_network: false,
            allow_secret: false,
            leases: Vec::new(),
        }
    }

    pub fn with_lease(mut self, lease: CapabilityLease) -> Self {
        self.leases.push(lease);
        self
    }

    pub fn with_network(mut self) -> Self {
        self.allow_network = true;
        self
    }

    pub fn with_secret(mut self) -> Self {
        self.allow_secret = true;
        self
    }
}

pub trait RuntimeBackend {
    fn runtime_kind(&self) -> RuntimeKind;
    fn execute(&self, request: &RuntimeRequest) -> Result<BackendCompletion, BackendFailure>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackendCompletion {
    pub output_summary: String,
    pub actual_usage: ResourceUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendFailure {
    pub reason: String,
}

pub struct BodyRuntime<B> {
    backend: B,
}

impl<B> BodyRuntime<B>
where
    B: RuntimeBackend,
{
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn invoke(&self, request: RuntimeRequest, policy: &RuntimePolicy) -> RuntimeOutcome {
        if self.backend.runtime_kind() != request.runtime {
            return blocked(&request, BlockReason::BackendUnavailable);
        }

        if let Some(reason) = preflight(&request, policy) {
            return blocked(&request, reason);
        }

        match self.backend.execute(&request) {
            Ok(completion) => RuntimeOutcome::Completed(RuntimeCompletion {
                output_summary: completion.output_summary,
                actual_usage: completion.actual_usage,
                audit: audit_event(
                    AuditLevel::Info,
                    request.change_level,
                    request.risk_line,
                    format!("runtime completed capability {}", request.capability),
                    None,
                ),
            }),
            Err(failure) => RuntimeOutcome::Failed(RuntimeFailure {
                reason: failure.reason.clone(),
                audit: audit_event(
                    AuditLevel::Warning,
                    request.change_level,
                    request.risk_line,
                    format!("runtime failed capability {}", request.capability),
                    Some(failure.reason),
                ),
            }),
        }
    }
}

fn preflight(request: &RuntimeRequest, policy: &RuntimePolicy) -> Option<BlockReason> {
    if request.risk_line == RiskLine::Red && !policy.allow_red_line {
        return Some(BlockReason::RedLine);
    }

    if request.risk_line == RiskLine::Yellow && !policy.allow_yellow_line {
        return Some(BlockReason::RedLine);
    }

    if request.requires_network && !policy.allow_network {
        return Some(BlockReason::NetworkDenied);
    }

    if request.requires_secret && !policy.allow_secret {
        return Some(BlockReason::SecretDenied);
    }

    if !policy.budget.allows(&request.estimated_usage) {
        return Some(BlockReason::BudgetExceeded);
    }

    if !has_matching_lease(request, policy) {
        return Some(BlockReason::MissingCapabilityLease);
    }

    None
}

fn has_matching_lease(request: &RuntimeRequest, policy: &RuntimePolicy) -> bool {
    policy
        .leases
        .iter()
        .any(|lease| lease.scope == request.scope && lease.capability_name == request.capability)
}

fn blocked(request: &RuntimeRequest, reason: BlockReason) -> RuntimeOutcome {
    RuntimeOutcome::Blocked(RuntimeBlock {
        reason,
        audit: audit_event(
            AuditLevel::Warning,
            request.change_level,
            request.risk_line,
            format!("runtime blocked capability {}", request.capability),
            Some(format!("{reason:?}")),
        ),
    })
}

fn audit_event(
    level: AuditLevel,
    change_level: ChangeLevel,
    risk_line: RiskLine,
    summary: String,
    consequence: Option<String>,
) -> AuditEvent {
    AuditEvent {
        level,
        change_level,
        risk_line,
        summary,
        consequence,
    }
}

#[derive(Debug, Clone)]
pub struct EchoBackend {
    kind: RuntimeKind,
}

impl EchoBackend {
    pub fn new(kind: RuntimeKind) -> Self {
        Self { kind }
    }
}

impl RuntimeBackend for EchoBackend {
    fn runtime_kind(&self) -> RuntimeKind {
        self.kind
    }

    fn execute(&self, request: &RuntimeRequest) -> Result<BackendCompletion, BackendFailure> {
        Ok(BackendCompletion {
            output_summary: format!("echo: {}", request.input_summary),
            actual_usage: request.estimated_usage.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> BodyScope {
        BodyScope::new("buster", "main", "task")
    }

    fn lease() -> CapabilityLease {
        CapabilityLease {
            scope: scope(),
            capability_name: "think".to_string(),
            expires_at: None,
            reason: "test".to_string(),
        }
    }

    #[test]
    fn runtime_requires_matching_lease() {
        let runtime = BodyRuntime::new(EchoBackend::new(RuntimeKind::ExternalLlm));
        let request = RuntimeRequest::new(scope(), RuntimeKind::ExternalLlm, "think", "hello");
        let policy = RuntimePolicy::deny_by_default(ResourceBudget::default());

        let outcome = runtime.invoke(request, &policy);

        assert!(matches!(
            outcome,
            RuntimeOutcome::Blocked(RuntimeBlock {
                reason: BlockReason::MissingCapabilityLease,
                ..
            })
        ));
    }

    #[test]
    fn runtime_blocks_red_line_by_default() {
        let runtime = BodyRuntime::new(EchoBackend::new(RuntimeKind::ExternalLlm));
        let request = RuntimeRequest::new(scope(), RuntimeKind::ExternalLlm, "think", "hello")
            .with_risk(ChangeLevel::IdentityValue, RiskLine::Red);
        let policy = RuntimePolicy::deny_by_default(ResourceBudget::default()).with_lease(lease());

        let outcome = runtime.invoke(request, &policy);

        assert!(matches!(
            outcome,
            RuntimeOutcome::Blocked(RuntimeBlock {
                reason: BlockReason::RedLine,
                ..
            })
        ));
    }

    #[test]
    fn runtime_completes_when_preflight_passes() {
        let runtime = BodyRuntime::new(EchoBackend::new(RuntimeKind::ExternalLlm));
        let request = RuntimeRequest::new(scope(), RuntimeKind::ExternalLlm, "think", "hello");
        let policy = RuntimePolicy::deny_by_default(ResourceBudget::default()).with_lease(lease());

        let outcome = runtime.invoke(request, &policy);

        assert!(matches!(outcome, RuntimeOutcome::Completed(_)));
    }
}
