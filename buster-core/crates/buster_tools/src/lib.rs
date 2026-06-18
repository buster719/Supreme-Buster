//! Buster's layer-3 tool system.
//!
//! This crate is intentionally plain: tools are named capabilities with input
//! schemas, runtime requirements, and permission metadata. Layer 4
//! (`buster_body`) decides whether an invocation is allowed and records the
//! audit outcome.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use buster_body::host_api::{BodyScope, ChangeLevel, RiskLine};
use buster_body::resources::{ResourceBudget, ResourceUsage};
use buster_body::runtime::{
    BodyRuntime, RuntimeBackend, RuntimeKind, RuntimeOutcome, RuntimePolicy, RuntimeRequest,
};
use buster_body::{BodyGate, BodyGateError, BodyStore, CapabilityLease};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ToolPermission {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolSource {
    Builtin,
    Runtime,
    Plugin,
    ExternalCli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolStatus {
    Draft,
    Active,
    Quarantined,
    Disabled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolManifest {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub runtime_kind: RuntimeKind,
    pub required_permission: ToolPermission,
    pub capability: String,
    pub required_network_hosts: Vec<String>,
    pub required_secrets: Vec<String>,
    pub budget: ResourceBudget,
    pub source: ToolSource,
    pub status: ToolStatus,
    pub action_rules: Vec<ToolActionRule>,
}

impl ToolManifest {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        runtime_kind: RuntimeKind,
        capability: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: serde_json::json!({ "type": "object" }),
            runtime_kind,
            required_permission: ToolPermission::ReadOnly,
            capability: capability.into(),
            required_network_hosts: Vec::new(),
            required_secrets: Vec::new(),
            budget: ResourceBudget::default(),
            source: ToolSource::Builtin,
            status: ToolStatus::Draft,
            action_rules: Vec::new(),
        }
    }

    pub fn active(mut self) -> Self {
        self.status = ToolStatus::Active;
        self
    }

    pub fn with_input_schema(mut self, schema: Value) -> Self {
        self.input_schema = schema;
        self
    }

    pub fn with_permission(mut self, permission: ToolPermission) -> Self {
        self.required_permission = permission;
        self
    }

    pub fn with_network_hosts(
        mut self,
        hosts: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.required_network_hosts = hosts.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_secrets(mut self, secrets: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.required_secrets = secrets.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_budget(mut self, budget: ResourceBudget) -> Self {
        self.budget = budget;
        self
    }

    pub fn with_source(mut self, source: ToolSource) -> Self {
        self.source = source;
        self
    }

    pub fn with_action_rules(mut self, rules: impl IntoIterator<Item = ToolActionRule>) -> Self {
        self.action_rules = rules.into_iter().collect();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolActionRule {
    pub action: String,
    pub required_permission: ToolPermission,
    pub allowed_identities: Vec<String>,
    pub allowed_targets: Vec<String>,
    pub max_invocations: u32,
    pub max_chars: Option<usize>,
}

impl ToolActionRule {
    pub fn new(action: impl Into<String>, permission: ToolPermission) -> Self {
        Self {
            action: action.into(),
            required_permission: permission,
            allowed_identities: Vec::new(),
            allowed_targets: Vec::new(),
            max_invocations: 1,
            max_chars: None,
        }
    }

    pub fn with_identities(
        mut self,
        identities: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.allowed_identities = identities.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_targets(mut self, targets: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowed_targets = targets.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_max_invocations(mut self, max_invocations: u32) -> Self {
        self.max_invocations = max_invocations;
        self
    }

    pub fn with_max_chars(mut self, max_chars: usize) -> Self {
        self.max_chars = Some(max_chars);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolRegistry {
    tools: BTreeMap<String, ToolManifest>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, manifest: ToolManifest) -> Result<(), ToolRegistryError> {
        if self.tools.contains_key(&manifest.name) {
            return Err(ToolRegistryError::DuplicateTool {
                name: manifest.name,
            });
        }
        self.tools.insert(manifest.name.clone(), manifest);
        Ok(())
    }

    pub fn replace(&mut self, manifest: ToolManifest) {
        self.tools.insert(manifest.name.clone(), manifest);
    }

    pub fn get(&self, name: &str) -> Option<&ToolManifest> {
        self.tools.get(name)
    }

    pub fn list(&self) -> Vec<&ToolManifest> {
        self.tools.values().collect()
    }

    pub fn disable(&mut self, name: &str) -> Result<(), ToolRegistryError> {
        let tool = self
            .tools
            .get_mut(name)
            .ok_or_else(|| ToolRegistryError::UnknownTool {
                name: name.to_string(),
            })?;
        tool.status = ToolStatus::Disabled;
        Ok(())
    }

    pub fn quarantine(&mut self, name: &str) -> Result<(), ToolRegistryError> {
        let tool = self
            .tools
            .get_mut(name)
            .ok_or_else(|| ToolRegistryError::UnknownTool {
                name: name.to_string(),
            })?;
        tool.status = ToolStatus::Quarantined;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolRegistryError {
    DuplicateTool { name: String },
    UnknownTool { name: String },
}

impl fmt::Display for ToolRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateTool { name } => write!(formatter, "duplicate tool `{name}`"),
            Self::UnknownTool { name } => write!(formatter, "unknown tool `{name}`"),
        }
    }
}

impl std::error::Error for ToolRegistryError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRegistryEntry {
    pub name: String,
    pub description: String,
    pub runtime_kind: String,
    pub required_permission: ToolPermission,
    pub capability: String,
    pub required_network_hosts: Vec<String>,
    pub required_secrets: Vec<String>,
    pub source: ToolSource,
    pub status: ToolStatus,
    pub governance_level: u8,
    pub risk_line: String,
    pub action_rules: Vec<ToolActionRule>,
    pub input_schema: Value,
    pub budget: ResourceBudget,
    pub updated_at_secs: u64,
    pub last_used_at_secs: Option<u64>,
    pub use_count: u64,
    pub failure_count: u64,
    pub quarantined_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRegistrySnapshot {
    pub updated_at_secs: u64,
    pub entries: Vec<ToolRegistryEntry>,
}

impl Default for ToolRegistrySnapshot {
    fn default() -> Self {
        Self {
            updated_at_secs: now_secs(),
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolEvent {
    pub timestamp_secs: u64,
    pub kind: ToolEventKind,
    pub tool_name: Option<String>,
    pub summary: String,
    pub governance_level: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolEventKind {
    RegistryScanned,
    ToolViewed,
    ToolQueried,
    ToolUsed,
    ToolQuarantined,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolView {
    pub entry: ToolRegistryEntry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolUseReceipt {
    pub entry: ToolRegistryEntry,
    pub result: ToolActionResult,
    pub summary: String,
    pub audit_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ToolWorkspace {
    root: PathBuf,
}

impl ToolWorkspace {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn ensure_layout(&self) -> Result<(), ToolWorkspaceError> {
        fs::create_dir_all(self.root.join("tools").join("installed"))?;
        fs::create_dir_all(self.root.join("tools").join("generated"))?;
        fs::create_dir_all(self.root.join("tools").join("archive"))?;
        fs::create_dir_all(self.root.join("state"))?;
        fs::create_dir_all(self.root.join("audit"))?;
        Ok(())
    }

    pub fn refresh_registry(&self) -> Result<ToolRegistrySnapshot, ToolWorkspaceError> {
        self.ensure_layout()?;
        let previous = self.read_registry().unwrap_or_default();
        let mut entries = builtin_tool_manifests()
            .into_iter()
            .map(|manifest| entry_from_manifest(manifest, &previous))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        let snapshot = ToolRegistrySnapshot {
            updated_at_secs: now_secs(),
            entries,
        };
        self.write_registry(&snapshot)?;
        self.append_event(&ToolEvent {
            timestamp_secs: now_secs(),
            kind: ToolEventKind::RegistryScanned,
            tool_name: None,
            summary: format!("scanned {} tool(s)", snapshot.entries.len()),
            governance_level: 3,
        })?;
        Ok(snapshot)
    }

    pub fn read_registry(&self) -> Result<ToolRegistrySnapshot, ToolWorkspaceError> {
        let raw = fs::read_to_string(self.registry_path())?;
        serde_json::from_str(&raw).map_err(ToolWorkspaceError::Json)
    }

    pub fn query_tools(&self, query: &str) -> Result<Vec<ToolRegistryEntry>, ToolWorkspaceError> {
        let registry = self.registry_or_scan()?;
        let needle = query.trim().to_ascii_lowercase();
        let matches = registry
            .entries
            .into_iter()
            .filter(|entry| {
                needle.is_empty()
                    || entry.name.to_ascii_lowercase().contains(&needle)
                    || entry.description.to_ascii_lowercase().contains(&needle)
                    || entry.capability.to_ascii_lowercase().contains(&needle)
                    || entry
                        .required_network_hosts
                        .iter()
                        .any(|host| host.to_ascii_lowercase().contains(&needle))
                    || entry
                        .action_rules
                        .iter()
                        .any(|rule| rule.action.to_ascii_lowercase().contains(&needle))
            })
            .collect::<Vec<_>>();
        self.append_event(&ToolEvent {
            timestamp_secs: now_secs(),
            kind: ToolEventKind::ToolQueried,
            tool_name: None,
            summary: format!(
                "tool query `{}` returned {} result(s)",
                query,
                matches.len()
            ),
            governance_level: 3,
        })?;
        Ok(matches)
    }

    pub fn view_tool(&self, name: &str) -> Result<ToolView, ToolWorkspaceError> {
        let registry = self.registry_or_scan()?;
        let entry =
            find_entry(&registry, name).ok_or_else(|| ToolWorkspaceError::ToolNotFound {
                name: name.to_string(),
            })?;
        self.append_event(&ToolEvent {
            timestamp_secs: now_secs(),
            kind: ToolEventKind::ToolViewed,
            tool_name: Some(entry.name.clone()),
            summary: "tool manifest viewed for possible use".to_string(),
            governance_level: entry.governance_level,
        })?;
        Ok(ToolView { entry })
    }

    pub fn record_use(
        &self,
        name: &str,
        result: ToolActionResult,
        summary: impl Into<String>,
    ) -> Result<ToolUseReceipt, ToolWorkspaceError> {
        let summary = summary.into();
        let mut registry = self.registry_or_scan()?;
        let index =
            find_entry_index(&registry, name).ok_or_else(|| ToolWorkspaceError::ToolNotFound {
                name: name.to_string(),
            })?;
        registry.entries[index].use_count = registry.entries[index].use_count.saturating_add(1);
        registry.entries[index].last_used_at_secs = Some(now_secs());
        if result != ToolActionResult::Success {
            registry.entries[index].failure_count =
                registry.entries[index].failure_count.saturating_add(1);
        }
        if registry.entries[index].failure_count >= 3
            && registry.entries[index].status == ToolStatus::Active
        {
            registry.entries[index].status = ToolStatus::Quarantined;
            registry.entries[index].quarantined_reason =
                Some("three recorded non-successful tool uses".to_string());
            self.append_event(&ToolEvent {
                timestamp_secs: now_secs(),
                kind: ToolEventKind::ToolQuarantined,
                tool_name: Some(registry.entries[index].name.clone()),
                summary: registry.entries[index]
                    .quarantined_reason
                    .clone()
                    .unwrap_or_else(|| "tool quarantined".to_string()),
                governance_level: registry.entries[index].governance_level,
            })?;
        }
        registry.updated_at_secs = now_secs();
        let entry = registry.entries[index].clone();
        self.write_registry(&registry)?;
        let record = serde_json::json!({
            "timestamp_secs": now_secs(),
            "tool_name": entry.name,
            "result": result,
            "summary": summary,
            "use_count": entry.use_count,
            "failure_count": entry.failure_count,
            "status": entry.status,
        });
        append_jsonl(&self.audit_path(), &record)?;
        self.append_event(&ToolEvent {
            timestamp_secs: now_secs(),
            kind: ToolEventKind::ToolUsed,
            tool_name: Some(entry.name.clone()),
            summary: "tool use recorded".to_string(),
            governance_level: entry.governance_level,
        })?;
        Ok(ToolUseReceipt {
            entry,
            result,
            summary,
            audit_path: self.audit_path(),
        })
    }

    pub fn runtime_registry(&self) -> Result<ToolRegistry, ToolWorkspaceError> {
        let snapshot = self.registry_or_scan()?;
        let mut registry = ToolRegistry::new();
        for manifest in builtin_tool_manifests() {
            let status = find_entry(&snapshot, &manifest.name)
                .map(|entry| entry.status)
                .unwrap_or(manifest.status);
            let mut manifest = manifest;
            manifest.status = status;
            registry.replace(manifest);
        }
        Ok(registry)
    }

    fn registry_or_scan(&self) -> Result<ToolRegistrySnapshot, ToolWorkspaceError> {
        self.read_registry().or_else(|_| self.refresh_registry())
    }

    fn write_registry(&self, registry: &ToolRegistrySnapshot) -> Result<(), ToolWorkspaceError> {
        atomic_write(
            &self.registry_path(),
            &serde_json::to_string_pretty(registry).map_err(ToolWorkspaceError::Json)?,
        )
    }

    fn append_event(&self, event: &ToolEvent) -> Result<(), ToolWorkspaceError> {
        append_jsonl(&self.events_path(), event)
    }

    fn registry_path(&self) -> PathBuf {
        self.root.join("state").join("tool-registry.json")
    }

    fn events_path(&self) -> PathBuf {
        self.root.join("audit").join("tool-events.jsonl")
    }

    fn audit_path(&self) -> PathBuf {
        self.root.join("audit").join("tool-use.jsonl")
    }
}

#[derive(Debug)]
pub enum ToolWorkspaceError {
    Io(std::io::Error),
    Json(serde_json::Error),
    ToolNotFound { name: String },
}

impl fmt::Display for ToolWorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "tool workspace I/O error: {error}"),
            Self::Json(error) => write!(formatter, "tool workspace JSON error: {error}"),
            Self::ToolNotFound { name } => write!(formatter, "tool `{name}` not found"),
        }
    }
}

impl std::error::Error for ToolWorkspaceError {}

impl From<std::io::Error> for ToolWorkspaceError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolInvocation {
    pub tool_name: String,
    pub input: Value,
    pub scope: BodyScope,
    pub reason: String,
    pub action: Option<ToolActionRequest>,
}

impl ToolInvocation {
    pub fn new(
        tool_name: impl Into<String>,
        input: Value,
        scope: BodyScope,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            input,
            scope,
            reason: reason.into(),
            action: None,
        }
    }

    pub fn with_action(mut self, action: ToolActionRequest) -> Self {
        self.action = Some(action);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolActionRequest {
    pub action: String,
    pub identity: Option<String>,
    pub target: Option<String>,
    pub content_chars: Option<usize>,
}

impl ToolActionRequest {
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            identity: None,
            target: None,
            content_chars: None,
        }
    }

    pub fn with_identity(mut self, identity: impl Into<String>) -> Self {
        self.identity = Some(identity.into());
        self
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    pub fn with_content_chars(mut self, content_chars: usize) -> Self {
        self.content_chars = Some(content_chars);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolActionLease {
    pub lease_id: String,
    pub tool_name: String,
    pub capability: String,
    pub action: String,
    pub identity: Option<String>,
    pub target: Option<String>,
    pub max_invocations: u32,
    pub used_invocations: u32,
    pub expires_at_secs: u64,
    pub issued_by: LeaseIssuer,
    pub reason: String,
}

impl ToolActionLease {
    pub fn can_use(&self, now_secs: u64) -> bool {
        self.used_invocations < self.max_invocations && now_secs <= self.expires_at_secs
    }

    pub fn consume(&mut self, now_secs: u64) -> Result<(), ToolLeaseError> {
        if !self.can_use(now_secs) {
            return Err(ToolLeaseError::LeaseExpiredOrConsumed {
                lease_id: self.lease_id.clone(),
            });
        }
        self.used_invocations += 1;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaseIssuer {
    Policy,
    Agency,
    ValueModel,
    PeerQuorum,
    HumanDelegate,
    EmergencyRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolLeaseError {
    UnknownAction { action: String },
    IdentityDenied { identity: String },
    TargetDenied { target: String },
    ContentTooLarge { max: usize, actual: usize },
    LeaseExpiredOrConsumed { lease_id: String },
}

impl fmt::Display for ToolLeaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAction { action } => write!(formatter, "unknown tool action `{action}`"),
            Self::IdentityDenied { identity } => write!(
                formatter,
                "identity `{identity}` is not allowed for this action"
            ),
            Self::TargetDenied { target } => write!(
                formatter,
                "target `{target}` is not allowed for this action"
            ),
            Self::ContentTooLarge { max, actual } => {
                write!(formatter, "content too large: {actual} chars exceeds {max}")
            }
            Self::LeaseExpiredOrConsumed { lease_id } => write!(
                formatter,
                "tool action lease `{lease_id}` is expired or consumed"
            ),
        }
    }
}

impl std::error::Error for ToolLeaseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolActionPolicy {
    pub ttl_secs: u64,
    pub issuer: LeaseIssuer,
}

impl Default for ToolActionPolicy {
    fn default() -> Self {
        Self {
            ttl_secs: 300,
            issuer: LeaseIssuer::Policy,
        }
    }
}

impl ToolActionPolicy {
    pub fn issue_lease(
        &self,
        manifest: &ToolManifest,
        request: &ToolActionRequest,
        reason: impl Into<String>,
    ) -> Result<ToolActionLease, ToolLeaseError> {
        let rule = manifest
            .action_rules
            .iter()
            .find(|rule| rule.action == request.action)
            .ok_or_else(|| ToolLeaseError::UnknownAction {
                action: request.action.clone(),
            })?;

        if let Some(identity) = &request.identity {
            if !rule.allowed_identities.is_empty()
                && !rule
                    .allowed_identities
                    .iter()
                    .any(|allowed| allowed == identity)
            {
                return Err(ToolLeaseError::IdentityDenied {
                    identity: identity.clone(),
                });
            }
        }

        if let Some(target) = &request.target {
            if !rule.allowed_targets.is_empty()
                && !rule.allowed_targets.iter().any(|allowed| allowed == target)
            {
                return Err(ToolLeaseError::TargetDenied {
                    target: target.clone(),
                });
            }
        }

        if let (Some(max), Some(actual)) = (rule.max_chars, request.content_chars) {
            if actual > max {
                return Err(ToolLeaseError::ContentTooLarge { max, actual });
            }
        }

        let now = now_secs();
        Ok(ToolActionLease {
            lease_id: format!("tool-lease-{}-{now}", manifest.name),
            tool_name: manifest.name.clone(),
            capability: manifest.capability.clone(),
            action: request.action.clone(),
            identity: request.identity.clone(),
            target: request.target.clone(),
            max_invocations: rule.max_invocations,
            used_invocations: 0,
            expires_at_secs: now.saturating_add(self.ttl_secs),
            issued_by: self.issuer,
            reason: reason.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutorError {
    UnknownTool { name: String },
    DisabledTool { name: String },
    RuntimeKindMismatch { tool: String },
    ActionLeaseDenied { reason: String },
    ActionLeaseRequired { tool: String },
    BodyGate { reason: String },
}

impl fmt::Display for ToolExecutorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTool { name } => write!(formatter, "unknown tool `{name}`"),
            Self::DisabledTool { name } => write!(formatter, "tool `{name}` is disabled"),
            Self::RuntimeKindMismatch { tool } => {
                write!(
                    formatter,
                    "backend runtime kind does not match tool `{tool}`"
                )
            }
            Self::ActionLeaseDenied { reason } => {
                write!(formatter, "tool action lease denied: {reason}")
            }
            Self::ActionLeaseRequired { tool } => {
                write!(formatter, "tool `{tool}` requires an action lease")
            }
            Self::BodyGate { reason } => write!(formatter, "body gate rejected tool: {reason}"),
        }
    }
}

impl std::error::Error for ToolExecutorError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolActionAuditRecord {
    pub occurred_at_secs: u64,
    pub tool_name: String,
    pub action: String,
    pub identity: Option<String>,
    pub target_hmac: Option<String>,
    pub input_hmac: String,
    pub input_len: usize,
    pub message_id: Option<String>,
    pub lease_id: String,
    pub lease_issuer: LeaseIssuer,
    pub result: ToolActionResult,
    pub consequence: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolActionResult {
    Success,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolAuditHasher {
    hmac_key: Vec<u8>,
}

impl ToolAuditHasher {
    pub fn new(hmac_key: impl Into<Vec<u8>>) -> Self {
        Self {
            hmac_key: hmac_key.into(),
        }
    }

    pub fn digest_hex(&self, value: &str) -> String {
        let mut mac =
            HmacSha256::new_from_slice(&self.hmac_key).expect("HMAC accepts keys of any size");
        mac.update(value.as_bytes());
        let bytes = mac.finalize().into_bytes();
        hex_lower(&bytes)
    }
}

impl Default for ToolAuditHasher {
    fn default() -> Self {
        Self::new(b"buster-tool-audit-dev-key".to_vec())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolExecutionReport {
    pub outcome: RuntimeOutcome,
    pub action_audit: Option<ToolActionAuditRecord>,
    pub quarantine_recommendation: Option<ToolQuarantineRecommendation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolQuarantineRecommendation {
    pub tool_name: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolQuarantinePolicy {
    pub quarantine_on_blocked: bool,
    pub quarantine_on_failed_external_effect: bool,
}

impl Default for ToolQuarantinePolicy {
    fn default() -> Self {
        Self {
            quarantine_on_blocked: true,
            quarantine_on_failed_external_effect: true,
        }
    }
}

impl ToolQuarantinePolicy {
    pub fn recommendation_for(
        &self,
        manifest: &ToolManifest,
        outcome: &RuntimeOutcome,
        action_lease: Option<&ToolActionLease>,
    ) -> Option<ToolQuarantineRecommendation> {
        match outcome {
            RuntimeOutcome::Blocked(block) if self.quarantine_on_blocked => {
                Some(ToolQuarantineRecommendation {
                    tool_name: manifest.name.clone(),
                    reason: format!("runtime blocked action: {:?}", block.reason),
                })
            }
            RuntimeOutcome::Failed(failure)
                if self.quarantine_on_failed_external_effect && action_lease.is_some() =>
            {
                Some(ToolQuarantineRecommendation {
                    tool_name: manifest.name.clone(),
                    reason: format!("external-effect tool failed: {}", failure.reason),
                })
            }
            _ => None,
        }
    }

    pub fn apply(
        &self,
        registry: &mut ToolRegistry,
        recommendation: &ToolQuarantineRecommendation,
    ) -> Result<(), ToolRegistryError> {
        registry.quarantine(&recommendation.tool_name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolExecutor {
    registry: ToolRegistry,
}

impl ToolExecutor {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    pub fn invoke_with_backend<B>(
        &self,
        invocation: ToolInvocation,
        backend: B,
        input_summary: impl Into<String>,
    ) -> Result<RuntimeOutcome, ToolExecutorError>
    where
        B: RuntimeBackend,
    {
        Ok(self
            .invoke_with_backend_report(invocation, backend, input_summary)?
            .outcome)
    }

    pub fn invoke_with_backend_report<B>(
        &self,
        invocation: ToolInvocation,
        backend: B,
        input_summary: impl Into<String>,
    ) -> Result<ToolExecutionReport, ToolExecutorError>
    where
        B: RuntimeBackend,
    {
        let manifest = self.registry.get(&invocation.tool_name).ok_or_else(|| {
            ToolExecutorError::UnknownTool {
                name: invocation.tool_name.clone(),
            }
        })?;

        if manifest.status != ToolStatus::Active {
            return Err(ToolExecutorError::DisabledTool {
                name: invocation.tool_name,
            });
        }

        if backend.runtime_kind() != manifest.runtime_kind {
            return Err(ToolExecutorError::RuntimeKindMismatch {
                tool: manifest.name.clone(),
            });
        }

        let mut action_lease = if manifest.action_rules.is_empty() {
            None
        } else {
            let action = invocation.action.as_ref().ok_or_else(|| {
                ToolExecutorError::ActionLeaseRequired {
                    tool: manifest.name.clone(),
                }
            })?;
            Some(
                ToolActionPolicy::default()
                    .issue_lease(manifest, action, invocation.reason.clone())
                    .map_err(|error| ToolExecutorError::ActionLeaseDenied {
                        reason: error.to_string(),
                    })?,
            )
        };

        if let Some(lease) = &mut action_lease {
            lease
                .consume(now_secs())
                .map_err(|error| ToolExecutorError::ActionLeaseDenied {
                    reason: error.to_string(),
                })?;
        }

        let request = runtime_request_for(manifest, &invocation, input_summary.into());
        let policy = runtime_policy_for(manifest, &invocation);
        let runtime = BodyRuntime::new(backend);
        let outcome = runtime.invoke(request, &policy);
        let action_audit = action_lease.as_ref().map(|lease| {
            action_audit_record(lease, &invocation, &outcome, &ToolAuditHasher::default())
        });
        let quarantine_recommendation = ToolQuarantinePolicy::default().recommendation_for(
            manifest,
            &outcome,
            action_lease.as_ref(),
        );

        Ok(ToolExecutionReport {
            outcome,
            action_audit,
            quarantine_recommendation,
        })
    }

    pub fn invoke_with_gate<B, S>(
        &self,
        gate: &BodyGate<S>,
        invocation: ToolInvocation,
        backend: B,
        input_summary: impl Into<String>,
    ) -> Result<ToolExecutionReport, ToolExecutorError>
    where
        B: RuntimeBackend,
        S: BodyStore,
    {
        let manifest = self.registry.get(&invocation.tool_name).ok_or_else(|| {
            ToolExecutorError::UnknownTool {
                name: invocation.tool_name.clone(),
            }
        })?;

        if manifest.status != ToolStatus::Active {
            return Err(ToolExecutorError::DisabledTool {
                name: invocation.tool_name,
            });
        }

        if backend.runtime_kind() != manifest.runtime_kind {
            return Err(ToolExecutorError::RuntimeKindMismatch {
                tool: manifest.name.clone(),
            });
        }

        let mut action_lease = if manifest.action_rules.is_empty() {
            None
        } else {
            let action = invocation.action.as_ref().ok_or_else(|| {
                ToolExecutorError::ActionLeaseRequired {
                    tool: manifest.name.clone(),
                }
            })?;
            Some(
                ToolActionPolicy::default()
                    .issue_lease(manifest, action, invocation.reason.clone())
                    .map_err(|error| ToolExecutorError::ActionLeaseDenied {
                        reason: error.to_string(),
                    })?,
            )
        };

        if let Some(lease) = &mut action_lease {
            lease
                .consume(now_secs())
                .map_err(|error| ToolExecutorError::ActionLeaseDenied {
                    reason: error.to_string(),
                })?;
        }

        let request = runtime_request_for(manifest, &invocation, input_summary.into());
        let policy = runtime_policy_for(manifest, &invocation);
        let outcome = gate
            .invoke_runtime(request, &policy, backend)
            .map_err(tool_gate_error)?;
        let action_audit = action_lease.as_ref().map(|lease| {
            action_audit_record(lease, &invocation, &outcome, &ToolAuditHasher::default())
        });
        if let Some(audit) = &action_audit {
            gate.record_tool_action(audit).map_err(tool_gate_error)?;
        }
        let quarantine_recommendation = ToolQuarantinePolicy::default().recommendation_for(
            manifest,
            &outcome,
            action_lease.as_ref(),
        );

        Ok(ToolExecutionReport {
            outcome,
            action_audit,
            quarantine_recommendation,
        })
    }
}

fn tool_gate_error(error: BodyGateError) -> ToolExecutorError {
    ToolExecutorError::BodyGate {
        reason: error.to_string(),
    }
}

fn action_audit_record(
    lease: &ToolActionLease,
    invocation: &ToolInvocation,
    outcome: &RuntimeOutcome,
    hasher: &ToolAuditHasher,
) -> ToolActionAuditRecord {
    let input_string = invocation.input.to_string();
    let (result, consequence) = match outcome {
        RuntimeOutcome::Completed(completion) => (
            ToolActionResult::Success,
            completion.audit.consequence.clone(),
        ),
        RuntimeOutcome::Blocked(block) => {
            (ToolActionResult::Blocked, block.audit.consequence.clone())
        }
        RuntimeOutcome::Failed(failure) => (ToolActionResult::Failed, Some(failure.reason.clone())),
    };

    ToolActionAuditRecord {
        occurred_at_secs: now_secs(),
        tool_name: lease.tool_name.clone(),
        action: lease.action.clone(),
        identity: lease.identity.clone(),
        target_hmac: lease
            .target
            .as_ref()
            .map(|target| hasher.digest_hex(target)),
        input_hmac: hasher.digest_hex(&input_string),
        input_len: input_string.len(),
        message_id: extract_message_id(outcome),
        lease_id: lease.lease_id.clone(),
        lease_issuer: lease.issued_by,
        result,
        consequence,
    }
}

fn extract_message_id(outcome: &RuntimeOutcome) -> Option<String> {
    let text = match outcome {
        RuntimeOutcome::Completed(completion) => &completion.output_summary,
        RuntimeOutcome::Failed(failure) => &failure.reason,
        RuntimeOutcome::Blocked(_) => return None,
    };

    for marker in ["message_id", "messageId"] {
        if let Some(value) = extract_labeled_value(text, marker) {
            return Some(value);
        }
    }
    None
}

fn extract_labeled_value(text: &str, marker: &str) -> Option<String> {
    let index = text.find(marker)?;
    let after_marker = &text[index + marker.len()..];
    let after_separator = after_marker.trim_start_matches(|ch: char| {
        ch.is_whitespace() || ch == ':' || ch == '=' || ch == '"' || ch == '\''
    });
    let value: String = after_separator
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-' || *ch == '.')
        .collect();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn runtime_request_for(
    manifest: &ToolManifest,
    invocation: &ToolInvocation,
    input_summary: String,
) -> RuntimeRequest {
    let mut request = RuntimeRequest::new(
        invocation.scope.clone(),
        manifest.runtime_kind,
        manifest.capability.clone(),
        input_summary,
    )
    .with_estimated_usage(estimated_usage_from_budget(&manifest.budget));

    request = request.with_risk(
        ChangeLevel::CapabilityExecution,
        risk_line_for(manifest.required_permission),
    );

    if !manifest.required_network_hosts.is_empty() {
        request = request.with_network();
    }
    if !manifest.required_secrets.is_empty() {
        request = request.with_secret();
    }

    request
}

fn runtime_policy_for(manifest: &ToolManifest, invocation: &ToolInvocation) -> RuntimePolicy {
    let mut policy =
        RuntimePolicy::deny_by_default(manifest.budget.clone()).with_lease(CapabilityLease {
            scope: invocation.scope.clone(),
            capability_name: manifest.capability.clone(),
            expires_at: None,
            reason: invocation.reason.clone(),
        });

    if !manifest.required_network_hosts.is_empty() {
        policy = policy.with_network();
    }
    if !manifest.required_secrets.is_empty() {
        policy = policy.with_secret();
    }
    if manifest.required_permission == ToolPermission::DangerFullAccess {
        policy.allow_red_line = true;
    }

    policy
}

fn risk_line_for(permission: ToolPermission) -> RiskLine {
    match permission {
        ToolPermission::ReadOnly => RiskLine::Green,
        ToolPermission::WorkspaceWrite => RiskLine::Yellow,
        ToolPermission::DangerFullAccess => RiskLine::Red,
    }
}

fn estimated_usage_from_budget(budget: &ResourceBudget) -> ResourceUsage {
    ResourceUsage {
        tokens: budget.max_tokens.unwrap_or_default().min(512),
        wall_clock_ms: budget.max_wall_clock_ms.unwrap_or_default().min(1_000),
        network_bytes: budget.max_network_bytes.unwrap_or_default().min(1024),
        usd: budget.max_usd.unwrap_or_default().min(0.01),
    }
}

pub fn builtin_http_post_json_tool(allowed_hosts: Vec<String>) -> ToolManifest {
    ToolManifest::new(
        "http.post_json",
        "POST a JSON body through Buster's egress broker",
        RuntimeKind::Tool,
        "http.post_json",
    )
    .active()
    .with_input_schema(serde_json::json!({
        "type": "object",
        "required": ["url", "body"],
        "properties": {
            "url": { "type": "string" },
            "body": { "type": "object" }
        }
    }))
    .with_permission(ToolPermission::ReadOnly)
    .with_network_hosts(allowed_hosts)
    .with_budget(ResourceBudget {
        max_network_bytes: Some(256 * 1024),
        max_wall_clock_ms: Some(30_000),
        ..ResourceBudget::default()
    })
}

pub fn external_cli_tool(
    name: impl Into<String>,
    description: impl Into<String>,
    capability: impl Into<String>,
    permission: ToolPermission,
) -> ToolManifest {
    ToolManifest::new(name, description, RuntimeKind::Tool, capability)
        .active()
        .with_permission(permission)
        .with_source(ToolSource::ExternalCli)
        .with_input_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "args": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            }
        }))
        .with_budget(ResourceBudget {
            max_wall_clock_ms: Some(30_000),
            max_network_bytes: Some(512 * 1024),
            ..ResourceBudget::default()
        })
}

pub fn feishu_lark_cli_tool() -> ToolManifest {
    external_cli_tool(
        "feishu.lark_cli",
        "Run the official Lark/Feishu Open Platform CLI through Buster's CLI backend",
        "cli.feishu_lark",
        ToolPermission::WorkspaceWrite,
    )
    .with_network_hosts([
        "open.feishu.cn",
        "open.larksuite.com",
        "www.feishu.cn",
        "www.larksuite.com",
    ])
    .with_secrets(["feishu_app_id", "feishu_app_secret"])
    .with_action_rules([
        ToolActionRule::new("help", ToolPermission::ReadOnly).with_max_invocations(1),
        ToolActionRule::new("auth.status", ToolPermission::ReadOnly)
            .with_identities(["user", "bot"])
            .with_max_invocations(1),
        ToolActionRule::new("im.chat_list", ToolPermission::ReadOnly)
            .with_identities(["user", "bot"])
            .with_max_invocations(1),
        ToolActionRule::new("im.message_send", ToolPermission::WorkspaceWrite)
            .with_identities(["bot"])
            .with_max_invocations(1)
            .with_max_chars(500),
    ])
}

pub fn builtin_tool_manifests() -> Vec<ToolManifest> {
    vec![
        builtin_http_post_json_tool(vec![
            "api.xiaomimimo.com".to_string(),
            "openrouter.ai".to_string(),
        ]),
        external_cli_tool(
            "cli.node_version",
            "Check Node.js availability with a read-only CLI smoke command",
            "cli.node_version",
            ToolPermission::ReadOnly,
        ),
        ToolManifest::new(
            "research.source_fetch",
            "Fetch research source metadata from arXiv, OpenAlex, and Crossref through BodyGate preflight",
            RuntimeKind::Tool,
            "research.source_fetch",
        )
        .active()
        .with_permission(ToolPermission::ReadOnly)
        .with_source(ToolSource::Builtin)
        .with_network_hosts([
            "export.arxiv.org",
            "api.openalex.org",
            "api.crossref.org",
            "doi.org",
        ])
        .with_budget(ResourceBudget {
            max_wall_clock_ms: Some(60_000),
            max_network_bytes: Some(2 * 1024 * 1024),
            ..ResourceBudget::default()
        }),
        ToolManifest::new(
            "research.web_search",
            "Search the public web through DuckDuckGo/Bing HTML search as an untrusted evidence-lead tool",
            RuntimeKind::Tool,
            "research.web_search",
        )
        .active()
        .with_permission(ToolPermission::ReadOnly)
        .with_source(ToolSource::Builtin)
        .with_network_hosts(["duckduckgo.com", "html.duckduckgo.com", "bing.com", "www.bing.com"])
        .with_action_rules([
            ToolActionRule::new("search", ToolPermission::ReadOnly)
                .with_max_invocations(1)
                .with_max_chars(500),
        ])
        .with_budget(ResourceBudget {
            max_wall_clock_ms: Some(30_000),
            max_network_bytes: Some(2 * 1024 * 1024),
            ..ResourceBudget::default()
        }),
        ToolManifest::new(
            "arena.devfun",
            "Compete in DevFun Arena through Buster's local Arena harness",
            RuntimeKind::Tool,
            "arena.devfun",
        )
        .active()
        .with_permission(ToolPermission::WorkspaceWrite)
        .with_source(ToolSource::Builtin)
        .with_network_hosts(["arena.dev.fun"])
        .with_secrets(["arena_api_key"])
        .with_action_rules([
            ToolActionRule::new("status", ToolPermission::ReadOnly).with_max_invocations(1),
            ToolActionRule::new("competitions", ToolPermission::ReadOnly).with_max_invocations(1),
            ToolActionRule::new("fetch_skill", ToolPermission::WorkspaceWrite)
                .with_targets(["skills/installed/devfun-arena"])
                .with_max_invocations(3),
            ToolActionRule::new("heartbeat", ToolPermission::ReadOnly).with_max_invocations(1),
            ToolActionRule::new("strategy_update", ToolPermission::WorkspaceWrite)
                .with_targets(["state/arena-strategy.json", ".arena-poker-state"])
                .with_max_invocations(1),
            ToolActionRule::new("eval_start", ToolPermission::WorkspaceWrite)
                .with_targets([".arena-poker-state"])
                .with_max_invocations(1),
            ToolActionRule::new("eval_tick", ToolPermission::WorkspaceWrite)
                .with_targets([".arena-poker-state"])
                .with_max_invocations(1),
            ToolActionRule::new("register", ToolPermission::WorkspaceWrite)
                .with_targets([".arena-credentials"])
                .with_max_invocations(1),
        ])
        .with_budget(ResourceBudget {
            max_wall_clock_ms: Some(120_000),
            max_network_bytes: Some(4 * 1024 * 1024),
            ..ResourceBudget::default()
        }),
        ToolManifest::new(
            "research.paperqa",
            "Run PaperQA2 over Buster's bounded local paper corpus for cited literature QA",
            RuntimeKind::Tool,
            "research.paperqa",
        )
        .active()
        .with_permission(ToolPermission::ReadOnly)
        .with_source(ToolSource::ExternalCli)
        .with_network_hosts([
            "api.openai.com",
            "openrouter.ai",
            "api.xiaomimimo.com",
            "api.crossref.org",
            "api.semanticscholar.org",
            "api.openalex.org",
            "api.unpaywall.org",
        ])
        .with_secrets([
            "llm_api_key",
            "openai_api_key",
            "openrouter_api_key",
            "xiaomi_mimo_api_key",
            "crossref_api_key",
            "semantic_scholar_api_key",
        ])
        .with_action_rules([
            ToolActionRule::new("doctor", ToolPermission::ReadOnly).with_max_invocations(1),
            ToolActionRule::new("ask", ToolPermission::ReadOnly)
                .with_targets(["research/paperqa/papers"])
                .with_max_invocations(1)
                .with_max_chars(1_200),
            ToolActionRule::new("search", ToolPermission::ReadOnly)
                .with_targets(["research/paperqa/papers"])
                .with_max_invocations(1)
                .with_max_chars(500),
        ])
        .with_budget(ResourceBudget {
            max_wall_clock_ms: Some(240_000),
            max_network_bytes: Some(16 * 1024 * 1024),
            max_tokens: Some(8_000),
            ..ResourceBudget::default()
        }),
        ToolManifest::new(
            "research.paper_acquire",
            "Discover and download legal open-access PDFs into Buster's PaperQA corpus",
            RuntimeKind::Tool,
            "research.paper_acquire",
        )
        .active()
        .with_permission(ToolPermission::WorkspaceWrite)
        .with_source(ToolSource::Builtin)
        .with_network_hosts([
            "export.arxiv.org",
            "arxiv.org",
            "api.openalex.org",
            "api.crossref.org",
        ])
        .with_action_rules([
            ToolActionRule::new("discover", ToolPermission::ReadOnly)
                .with_max_invocations(1)
                .with_max_chars(1_200),
            ToolActionRule::new("download_open_pdf", ToolPermission::WorkspaceWrite)
                .with_targets(["research/paperqa/papers"])
                .with_max_invocations(5),
        ])
        .with_budget(ResourceBudget {
            max_wall_clock_ms: Some(180_000),
            max_network_bytes: Some(64 * 1024 * 1024),
            ..ResourceBudget::default()
        }),
        ToolManifest::new(
            "research.paper_brief",
            "Read extracted open-paper text and write a traceable research brief through BodyGate",
            RuntimeKind::ExternalLlm,
            "research.paper_brief",
        )
        .active()
        .with_permission(ToolPermission::WorkspaceWrite)
        .with_source(ToolSource::Builtin)
        .with_network_hosts(["configured_llm_endpoint"])
        .with_secrets(["llm_api_key", "openrouter_api_key", "xiaomi_mimo_api_key"])
        .with_action_rules([
            ToolActionRule::new("select_extracted_text", ToolPermission::ReadOnly)
                .with_targets(["research/paperqa/extracted", "research/paperqa/manifest.jsonl"])
                .with_max_invocations(1),
            ToolActionRule::new("brief", ToolPermission::WorkspaceWrite)
                .with_targets(["research/paperqa/briefs"])
                .with_max_invocations(1)
                .with_max_chars(42_000),
        ])
        .with_budget(ResourceBudget {
            max_wall_clock_ms: Some(120_000),
            max_network_bytes: Some(2 * 1024 * 1024),
            max_tokens: Some(3_000),
            ..ResourceBudget::default()
        }),
        feishu_lark_cli_tool(),
    ]
}

fn entry_from_manifest(
    manifest: ToolManifest,
    previous: &ToolRegistrySnapshot,
) -> ToolRegistryEntry {
    let old = previous
        .entries
        .iter()
        .find(|entry| entry.name == manifest.name);
    let status = old.map(|entry| entry.status).unwrap_or(manifest.status);
    ToolRegistryEntry {
        name: manifest.name,
        description: manifest.description,
        runtime_kind: runtime_kind_name(manifest.runtime_kind).to_string(),
        required_permission: manifest.required_permission,
        capability: manifest.capability,
        required_network_hosts: manifest.required_network_hosts,
        required_secrets: manifest.required_secrets,
        source: manifest.source,
        status,
        governance_level: governance_level_for_permission(manifest.required_permission),
        risk_line: format!("{:?}", risk_line_for(manifest.required_permission)),
        action_rules: manifest.action_rules,
        input_schema: manifest.input_schema,
        budget: manifest.budget,
        updated_at_secs: now_secs(),
        last_used_at_secs: old.and_then(|entry| entry.last_used_at_secs),
        use_count: old.map(|entry| entry.use_count).unwrap_or(0),
        failure_count: old.map(|entry| entry.failure_count).unwrap_or(0),
        quarantined_reason: old.and_then(|entry| entry.quarantined_reason.clone()),
    }
}

fn runtime_kind_name(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Wasm => "wasm",
        RuntimeKind::Script => "script",
        RuntimeKind::Mcp => "mcp",
        RuntimeKind::Tool => "tool",
        RuntimeKind::ExternalLlm => "external_llm",
    }
}

fn governance_level_for_permission(permission: ToolPermission) -> u8 {
    match permission {
        ToolPermission::ReadOnly => 3,
        ToolPermission::WorkspaceWrite => 3,
        ToolPermission::DangerFullAccess => 4,
    }
}

fn find_entry(registry: &ToolRegistrySnapshot, name: &str) -> Option<ToolRegistryEntry> {
    let needle = name.trim().to_ascii_lowercase();
    registry
        .entries
        .iter()
        .find(|entry| entry.name.to_ascii_lowercase() == needle)
        .cloned()
}

fn find_entry_index(registry: &ToolRegistrySnapshot, name: &str) -> Option<usize> {
    let needle = name.trim().to_ascii_lowercase();
    registry
        .entries
        .iter()
        .position(|entry| entry.name.to_ascii_lowercase() == needle)
}

fn append_jsonl(path: &Path, value: &impl Serialize) -> Result<(), ToolWorkspaceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value).map_err(ToolWorkspaceError::Json)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), ToolWorkspaceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension(format!("tmp.{}", now_secs()));
    fs::write(&tmp_path, contents)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use buster_body::{
        BodyGate, BodyStore, BrokeredHttpToolBackend, CliToolBackend, CliToolCommand, EgressBroker,
        JsonlBodyStore, NetworkPolicy, SqliteBodyStore,
    };
    use std::fs;

    fn scope() -> BodyScope {
        BodyScope::new("buster", "main", "tool-test")
    }

    #[test]
    fn registry_rejects_duplicate_tool_names() {
        let mut registry = ToolRegistry::new();
        let tool = builtin_http_post_json_tool(vec!["openrouter.ai".to_string()]);

        registry.register(tool.clone()).unwrap();
        let error = registry.register(tool).unwrap_err();

        assert!(matches!(error, ToolRegistryError::DuplicateTool { .. }));
    }

    #[test]
    fn executor_blocks_http_tool_disallowed_host_through_body_runtime() {
        let mut registry = ToolRegistry::new();
        registry
            .register(builtin_http_post_json_tool(vec![
                "openrouter.ai".to_string()
            ]))
            .unwrap();
        let executor = ToolExecutor::new(registry);
        let invocation = ToolInvocation::new(
            "http.post_json",
            serde_json::json!({
                "url": "https://evil.example/api",
                "body": { "hello": "buster" }
            }),
            scope(),
            "test blocked egress",
        );
        let broker = EgressBroker::new(NetworkPolicy {
            allowed_hosts: vec!["openrouter.ai".to_string()],
            deny_private_networks: true,
            max_response_bytes: Some(1024),
        });
        let backend = BrokeredHttpToolBackend::new(
            broker,
            "tool-test",
            serde_json::json!({ "hello": "buster" }),
        );

        let outcome = executor
            .invoke_with_backend(invocation, backend, "https://evil.example/api")
            .unwrap();

        assert!(matches!(outcome, RuntimeOutcome::Failed(_)));
        if let RuntimeOutcome::Failed(failure) = outcome {
            assert!(failure.reason.contains("evil.example"));
        }
    }

    #[test]
    fn executor_rejects_disabled_tool_before_runtime() {
        let mut registry = ToolRegistry::new();
        registry
            .register(builtin_http_post_json_tool(vec![
                "openrouter.ai".to_string()
            ]))
            .unwrap();
        registry.disable("http.post_json").unwrap();
        let executor = ToolExecutor::new(registry);
        let invocation = ToolInvocation::new(
            "http.post_json",
            serde_json::json!({}),
            scope(),
            "disabled test",
        );
        let broker = EgressBroker::new(NetworkPolicy::deny_all());
        let backend = BrokeredHttpToolBackend::new(broker, "tool-test", serde_json::json!({}));

        let error = executor
            .invoke_with_backend(invocation, backend, "https://openrouter.ai")
            .unwrap_err();

        assert!(matches!(error, ToolExecutorError::DisabledTool { .. }));
    }

    #[test]
    fn feishu_lark_cli_manifest_declares_network_and_secrets() {
        let manifest = feishu_lark_cli_tool();

        assert_eq!(manifest.source, ToolSource::ExternalCli);
        assert!(manifest
            .required_network_hosts
            .contains(&"open.feishu.cn".to_string()));
        assert!(manifest
            .required_secrets
            .contains(&"feishu_app_secret".to_string()));
        assert!(manifest
            .action_rules
            .iter()
            .any(|rule| rule.action == "im.message_send"));
    }

    #[test]
    fn executor_can_run_cli_backend_for_help_like_command() {
        let mut registry = ToolRegistry::new();
        registry
            .register(external_cli_tool(
                "cli.node_version",
                "Check Node.js availability",
                "cli.node_version",
                ToolPermission::ReadOnly,
            ))
            .unwrap();
        let executor = ToolExecutor::new(registry);
        let invocation = ToolInvocation::new(
            "cli.node_version",
            serde_json::json!({ "args": ["--version"] }),
            scope(),
            "CLI smoke test",
        );
        let backend = CliToolBackend::new("cli-smoke", smoke_command());

        let outcome = executor
            .invoke_with_backend(invocation, backend, "print buster-cli")
            .unwrap();

        assert!(matches!(outcome, RuntimeOutcome::Completed(_)));
    }

    #[test]
    fn tool_action_policy_issues_single_use_bot_send_lease() {
        let manifest = feishu_lark_cli_tool();
        let request = ToolActionRequest::new("im.message_send")
            .with_identity("bot")
            .with_target("oc_test")
            .with_content_chars(80);

        let mut lease = ToolActionPolicy::default()
            .issue_lease(&manifest, &request, "send status")
            .unwrap();

        assert_eq!(lease.identity.as_deref(), Some("bot"));
        lease.consume(now_secs()).unwrap();
        assert!(lease.consume(now_secs()).is_err());
    }

    #[test]
    fn tool_action_policy_rejects_user_identity_for_feishu_send() {
        let manifest = feishu_lark_cli_tool();
        let request = ToolActionRequest::new("im.message_send")
            .with_identity("user")
            .with_content_chars(80);

        let error = ToolActionPolicy::default()
            .issue_lease(&manifest, &request, "send status")
            .unwrap_err();

        assert!(matches!(error, ToolLeaseError::IdentityDenied { .. }));
    }

    #[test]
    fn executor_requires_action_lease_for_feishu_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(feishu_lark_cli_tool()).unwrap();
        let executor = ToolExecutor::new(registry);
        let invocation = ToolInvocation::new(
            "feishu.lark_cli",
            serde_json::json!({ "args": ["im", "+messages-send"] }),
            scope(),
            "missing action lease",
        );
        let backend = CliToolBackend::new("cli-smoke", smoke_command());

        let error = executor
            .invoke_with_backend(invocation, backend, "lark-cli im +messages-send")
            .unwrap_err();

        assert!(matches!(
            error,
            ToolExecutorError::ActionLeaseRequired { .. }
        ));
    }

    #[test]
    fn executor_report_records_external_effect_without_plain_content() {
        let mut registry = ToolRegistry::new();
        registry.register(feishu_lark_cli_tool()).unwrap();
        let executor = ToolExecutor::new(registry);
        let text = "hello from buster";
        let invocation = ToolInvocation::new(
            "feishu.lark_cli",
            serde_json::json!({
                "args": ["im", "+messages-send", "--as", "bot"],
                "text": text
            }),
            scope(),
            "send status",
        )
        .with_action(
            ToolActionRequest::new("im.message_send")
                .with_identity("bot")
                .with_target("oc_test")
                .with_content_chars(text.chars().count()),
        );
        let backend = MessageIdBackend;

        let report = executor
            .invoke_with_backend_report(invocation, backend, "send feishu message")
            .unwrap();

        let audit = report.action_audit.unwrap();
        assert_eq!(audit.result, ToolActionResult::Success);
        assert_eq!(audit.identity.as_deref(), Some("bot"));
        assert_eq!(audit.message_id.as_deref(), Some("om_test_123"));
        assert!(audit.target_hmac.is_some());
        assert!(!audit.input_hmac.contains(text));
    }

    #[test]
    fn failed_external_effect_recommends_tool_quarantine() {
        let mut registry = ToolRegistry::new();
        registry.register(feishu_lark_cli_tool()).unwrap();
        let executor = ToolExecutor::new(registry.clone());
        let invocation = ToolInvocation::new(
            "feishu.lark_cli",
            serde_json::json!({ "args": ["im", "+messages-send", "--as", "bot"] }),
            scope(),
            "send status",
        )
        .with_action(
            ToolActionRequest::new("im.message_send")
                .with_identity("bot")
                .with_target("oc_test")
                .with_content_chars(12),
        );

        let report = executor
            .invoke_with_backend_report(invocation, FailingToolBackend, "send feishu message")
            .unwrap();
        let recommendation = report.quarantine_recommendation.unwrap();
        ToolQuarantinePolicy::default()
            .apply(&mut registry, &recommendation)
            .unwrap();

        assert_eq!(
            registry.get("feishu.lark_cli").unwrap().status,
            ToolStatus::Quarantined
        );
    }

    #[test]
    fn action_audit_record_can_be_persisted_to_body_store() {
        let root =
            std::env::temp_dir().join(format!("buster-tool-audit-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = JsonlBodyStore::new(&root);
        let record = ToolActionAuditRecord {
            occurred_at_secs: 1,
            tool_name: "feishu.lark_cli".to_string(),
            action: "im.message_send".to_string(),
            identity: Some("bot".to_string()),
            target_hmac: Some("target-hmac".to_string()),
            input_hmac: "input-hmac".to_string(),
            input_len: 42,
            message_id: Some("om_test_123".to_string()),
            lease_id: "tool-lease-feishu".to_string(),
            lease_issuer: LeaseIssuer::Policy,
            result: ToolActionResult::Success,
            consequence: None,
        };

        store.append_tool_action(&record).unwrap();

        let raw = fs::read_to_string(root.join("audit/tool-actions.jsonl")).unwrap();
        assert!(raw.contains("feishu.lark_cli"));
        assert!(raw.contains("om_test_123"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn executor_can_route_tool_execution_through_body_gate() {
        let mut registry = ToolRegistry::new();
        registry.register(feishu_lark_cli_tool()).unwrap();
        let executor = ToolExecutor::new(registry);
        let store = SqliteBodyStore::open_in_memory().unwrap();
        let gate = BodyGate::new(store, buster_body::BodyMode::Normal);
        let text = "hello from gated tool";
        let invocation = ToolInvocation::new(
            "feishu.lark_cli",
            serde_json::json!({
                "args": ["im", "+messages-send", "--as", "bot"],
                "text": text
            }),
            scope(),
            "send status through body gate",
        )
        .with_action(
            ToolActionRequest::new("im.message_send")
                .with_identity("bot")
                .with_target("oc_test")
                .with_content_chars(text.chars().count()),
        );

        let report = executor
            .invoke_with_gate(&gate, invocation, MessageIdBackend, "send feishu message")
            .unwrap();

        assert!(matches!(report.outcome, RuntimeOutcome::Completed(_)));
        assert_eq!(gate.store().tool_action_count().unwrap(), 1);
        assert_eq!(gate.store().audit_event_count().unwrap(), 2);
    }

    #[test]
    fn tool_workspace_scans_builtin_registry_for_agent_query() {
        let root = temp_root("workspace-scan");
        let workspace = ToolWorkspace::new(&root);

        let registry = workspace.refresh_registry().unwrap();

        assert!(registry
            .entries
            .iter()
            .any(|entry| entry.name == "feishu.lark_cli"));
        assert!(registry
            .entries
            .iter()
            .any(|entry| entry.name == "research.source_fetch"));
        assert!(root.join("state").join("tool-registry.json").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tool_workspace_queries_and_views_tools() {
        let root = temp_root("workspace-query");
        let workspace = ToolWorkspace::new(&root);
        workspace.refresh_registry().unwrap();

        let results = workspace.query_tools("feishu").unwrap();
        let view = workspace.view_tool("feishu.lark_cli").unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(view.entry.name, "feishu.lark_cli");
        assert!(view
            .entry
            .action_rules
            .iter()
            .any(|rule| rule.action == "im.message_send"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tool_workspace_quarantines_after_repeated_failures() {
        let root = temp_root("workspace-quarantine");
        let workspace = ToolWorkspace::new(&root);
        workspace.refresh_registry().unwrap();

        for _ in 0..3 {
            workspace
                .record_use(
                    "feishu.lark_cli",
                    ToolActionResult::Failed,
                    "simulated send failure",
                )
                .unwrap();
        }

        let registry = workspace.read_registry().unwrap();
        let entry = registry
            .entries
            .iter()
            .find(|entry| entry.name == "feishu.lark_cli")
            .unwrap();
        assert_eq!(entry.status, ToolStatus::Quarantined);
        assert_eq!(entry.failure_count, 3);
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("buster-tools-{name}-{}", std::process::id()))
    }

    fn smoke_command() -> CliToolCommand {
        #[cfg(windows)]
        {
            CliToolCommand::new("cmd")
                .with_args(["/C", "echo", "buster-cli"])
                .inherit_env()
        }

        #[cfg(not(windows))]
        {
            CliToolCommand::new("/bin/sh")
                .with_args(["-c", "printf buster-cli"])
                .inherit_env()
        }
    }

    struct MessageIdBackend;

    impl RuntimeBackend for MessageIdBackend {
        fn runtime_kind(&self) -> RuntimeKind {
            RuntimeKind::Tool
        }

        fn execute(
            &self,
            _request: &buster_body::runtime::RuntimeRequest,
        ) -> Result<buster_body::runtime::BackendCompletion, buster_body::runtime::BackendFailure>
        {
            Ok(buster_body::runtime::BackendCompletion {
                output_summary: "message_id: om_test_123".to_string(),
                actual_usage: ResourceUsage::default(),
            })
        }
    }

    struct FailingToolBackend;

    impl RuntimeBackend for FailingToolBackend {
        fn runtime_kind(&self) -> RuntimeKind {
            RuntimeKind::Tool
        }

        fn execute(
            &self,
            _request: &buster_body::runtime::RuntimeRequest,
        ) -> Result<buster_body::runtime::BackendCompletion, buster_body::runtime::BackendFailure>
        {
            Err(buster_body::runtime::BackendFailure {
                reason: "simulated failed send".to_string(),
            })
        }
    }
}
