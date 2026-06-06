//! Secret handles, registry metadata, and one-shot leases.
//!
//! This module deliberately does not store plaintext secrets. It tracks handles,
//! allowed capabilities, and short-lived leases so the body runtime can decide
//! whether a capability may receive a secret from a real backend such as
//! Windows Credential Manager, Secret Service, Bitwarden, or IronClaw's secrets
//! store.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::state::BodyMode;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretHandle(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretLease {
    pub handle: SecretHandle,
    pub lease_id: String,
    pub consumed: bool,
    pub capability: String,
    pub reason: String,
    pub expires_at_secs: Option<u64>,
}

impl SecretLease {
    pub fn new(handle: SecretHandle, lease_id: impl Into<String>) -> Self {
        Self {
            handle,
            lease_id: lease_id.into(),
            consumed: false,
            capability: String::new(),
            reason: String::new(),
            expires_at_secs: None,
        }
    }

    pub fn consume(&mut self) {
        self.consumed = true;
    }

    pub fn active_for(&self, capability: &str, now_secs: u64) -> bool {
        !self.consumed
            && self.capability == capability
            && self
                .expires_at_secs
                .map_or(true, |expires_at| now_secs <= expires_at)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretClass {
    Ordinary,
    AccountCredential,
    LlmApiKey,
    IdentityKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRecord {
    pub handle: SecretHandle,
    pub class: SecretClass,
    pub allowed_capabilities: Vec<String>,
    pub description: String,
    pub fingerprint: Option<SecretFingerprint>,
}

impl SecretRecord {
    pub fn new(
        handle: SecretHandle,
        class: SecretClass,
        allowed_capabilities: Vec<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            handle,
            class,
            allowed_capabilities,
            description: description.into(),
            fingerprint: None,
        }
    }

    pub fn with_fingerprint(mut self, fingerprint: SecretFingerprint) -> Self {
        self.fingerprint = Some(fingerprint);
        self
    }

    pub fn allows_capability(&self, capability: &str) -> bool {
        self.allowed_capabilities
            .iter()
            .any(|allowed| allowed == capability)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFingerprint {
    pub stable_hash: String,
    pub prefix: String,
    pub suffix: String,
    pub len: usize,
}

impl SecretFingerprint {
    pub fn from_secret(secret: &str, hmac_key: &[u8]) -> Self {
        Self {
            stable_hash: hmac_sha256_hex(secret, hmac_key),
            prefix: String::new(),
            suffix: String::new(),
            len: secret.chars().count(),
        }
    }

    pub fn matches_text(&self, text: &str, hmac_key: &[u8]) -> bool {
        if self.len == 0 {
            return false;
        }

        secret_candidate_ranges(text, self.len)
            .into_iter()
            .any(|(start, end)| hmac_sha256_hex(&text[start..end], hmac_key) == self.stable_hash)
    }

    pub fn redact(&self, text: &str, hmac_key: &[u8]) -> String {
        if self.len == 0 {
            return text.to_string();
        }

        let ranges: Vec<(usize, usize)> = secret_candidate_ranges(text, self.len)
            .into_iter()
            .filter(|(start, end)| {
                hmac_sha256_hex(&text[*start..*end], hmac_key) == self.stable_hash
            })
            .collect();

        if ranges.is_empty() {
            return text.to_string();
        }

        let mut redacted = String::new();
        let mut cursor = 0;
        for (start, end) in ranges {
            if start < cursor {
                continue;
            }
            redacted.push_str(&text[cursor..start]);
            redacted.push_str("[SECRET_REDACTED]");
            cursor = end;
        }
        redacted.push_str(&text[cursor..]);
        redacted
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretDecision {
    Allowed,
    Denied(SecretDenyReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretDenyReason {
    UnknownHandle,
    IdentityKeyNotLeasable,
    CapabilityNotAllowed,
    BodyModeBlocksSecrets,
    LeaseNotFound,
    LeaseExpired,
    LeaseConsumed,
    LeaseCapabilityMismatch,
}

#[derive(Debug, Clone, Default)]
pub struct SecretRegistry {
    records: HashMap<SecretHandle, SecretRecord>,
    leases: HashMap<String, SecretLease>,
    next_lease_index: u64,
}

impl SecretRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, record: SecretRecord) {
        self.records.insert(record.handle.clone(), record);
    }

    pub fn record(&self, handle: &SecretHandle) -> Option<&SecretRecord> {
        self.records.get(handle)
    }

    pub fn fingerprints(&self) -> Vec<SecretFingerprint> {
        self.records
            .values()
            .filter_map(|record| record.fingerprint.clone())
            .collect()
    }

    pub fn issue_lease(
        &mut self,
        handle: &SecretHandle,
        capability: impl Into<String>,
        ttl_secs: Option<u64>,
        reason: impl Into<String>,
        mode: BodyMode,
    ) -> Result<SecretLease, SecretDenyReason> {
        let capability = capability.into();
        let record = self
            .records
            .get(handle)
            .ok_or(SecretDenyReason::UnknownHandle)?;

        if record.class == SecretClass::IdentityKey {
            return Err(SecretDenyReason::IdentityKeyNotLeasable);
        }

        if !record.allows_capability(&capability) {
            return Err(SecretDenyReason::CapabilityNotAllowed);
        }

        if !mode.permissions().allow_secrets {
            return Err(SecretDenyReason::BodyModeBlocksSecrets);
        }

        self.next_lease_index += 1;
        let now = now_secs();
        let lease = SecretLease {
            handle: handle.clone(),
            lease_id: format!("secret-lease-{}-{now}", self.next_lease_index),
            consumed: false,
            capability,
            reason: reason.into(),
            expires_at_secs: ttl_secs.map(|ttl| now.saturating_add(ttl)),
        };
        self.leases.insert(lease.lease_id.clone(), lease.clone());
        Ok(lease)
    }

    pub fn can_use_lease(
        &self,
        lease_id: &str,
        capability: &str,
        mode: BodyMode,
        now_secs: u64,
    ) -> SecretDecision {
        if !mode.permissions().allow_secrets {
            return SecretDecision::Denied(SecretDenyReason::BodyModeBlocksSecrets);
        }

        let Some(lease) = self.leases.get(lease_id) else {
            return SecretDecision::Denied(SecretDenyReason::LeaseNotFound);
        };

        if lease.consumed {
            return SecretDecision::Denied(SecretDenyReason::LeaseConsumed);
        }

        if lease.capability != capability {
            return SecretDecision::Denied(SecretDenyReason::LeaseCapabilityMismatch);
        }

        if lease
            .expires_at_secs
            .is_some_and(|expires_at| now_secs > expires_at)
        {
            return SecretDecision::Denied(SecretDenyReason::LeaseExpired);
        }

        SecretDecision::Allowed
    }

    pub fn consume_lease(
        &mut self,
        lease_id: &str,
        capability: &str,
        mode: BodyMode,
    ) -> SecretDecision {
        let decision = self.can_use_lease(lease_id, capability, mode, now_secs());
        if decision != SecretDecision::Allowed {
            return decision;
        }

        if let Some(lease) = self.leases.get_mut(lease_id) {
            lease.consume();
        }
        SecretDecision::Allowed
    }

    pub fn revoke_lease(&mut self, lease_id: &str) -> bool {
        self.leases.remove(lease_id).is_some()
    }
}

pub trait SecretBackend {
    fn get_secret(&self, handle: &SecretHandle) -> Option<String>;
    fn put_secret(&mut self, handle: SecretHandle, secret: String);
}

#[derive(Debug, Clone, Default)]
pub struct InMemorySecretBackend {
    secrets: HashMap<SecretHandle, String>,
}

impl InMemorySecretBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretBackend for InMemorySecretBackend {
    fn get_secret(&self, handle: &SecretHandle) -> Option<String> {
        self.secrets.get(handle).cloned()
    }

    fn put_secret(&mut self, handle: SecretHandle, secret: String) {
        self.secrets.insert(handle, secret);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretBrokerError {
    LeaseDenied(SecretDenyReason),
    BackendMissingSecret,
}

pub struct SecretBroker<B> {
    pub registry: SecretRegistry,
    pub backend: B,
}

impl<B> SecretBroker<B>
where
    B: SecretBackend,
{
    pub fn new(registry: SecretRegistry, backend: B) -> Self {
        Self { registry, backend }
    }

    pub fn use_secret_once<T>(
        &mut self,
        lease_id: &str,
        capability: &str,
        mode: BodyMode,
        use_fn: impl FnOnce(&str) -> T,
    ) -> Result<T, SecretBrokerError> {
        match self
            .registry
            .can_use_lease(lease_id, capability, mode, now_secs())
        {
            SecretDecision::Allowed => {}
            SecretDecision::Denied(reason) => return Err(SecretBrokerError::LeaseDenied(reason)),
        }

        let lease =
            self.registry
                .leases
                .get(lease_id)
                .cloned()
                .ok_or(SecretBrokerError::LeaseDenied(
                    SecretDenyReason::LeaseNotFound,
                ))?;
        let secret = self
            .backend
            .get_secret(&lease.handle)
            .ok_or(SecretBrokerError::BackendMissingSecret)?;

        let result = use_fn(&secret);
        let _ = self.registry.consume_lease(lease_id, capability, mode);
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRedactor {
    fingerprints: Vec<SecretFingerprint>,
    hmac_key: Vec<u8>,
}

impl SecretRedactor {
    pub fn new(fingerprints: Vec<SecretFingerprint>, hmac_key: impl Into<Vec<u8>>) -> Self {
        Self {
            fingerprints,
            hmac_key: hmac_key.into(),
        }
    }

    pub fn detects_leak(&self, text: &str) -> bool {
        self.fingerprints
            .iter()
            .any(|fingerprint| fingerprint.matches_text(text, &self.hmac_key))
    }

    pub fn redact(&self, text: &str) -> String {
        self.fingerprints
            .iter()
            .fold(text.to_string(), |current, fingerprint| {
                fingerprint.redact(&current, &self.hmac_key)
            })
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn hmac_sha256_hex(secret: &str, hmac_key: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(hmac_key).expect("HMAC accepts any key length");
    mac.update(secret.as_bytes());
    let bytes = mac.finalize().into_bytes();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn secret_candidate_ranges(text: &str, len_chars: usize) -> Vec<(usize, usize)> {
    let mut positions: Vec<usize> = text.char_indices().map(|(index, _)| index).collect();
    positions.push(text.len());

    if len_chars == 0 || positions.len() <= len_chars {
        return Vec::new();
    }

    (0..positions.len() - len_chars)
        .map(|start| (positions[start], positions[start + len_chars]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn openrouter_record() -> SecretRecord {
        SecretRecord::new(
            SecretHandle("openrouter_api_key".to_string()),
            SecretClass::LlmApiKey,
            vec!["llm.openrouter".to_string()],
            "OpenRouter API key handle",
        )
    }

    #[test]
    fn registry_issues_and_consumes_one_shot_lease() {
        let mut registry = SecretRegistry::new();
        let handle = openrouter_record().handle.clone();
        registry.register(openrouter_record());

        let lease = registry
            .issue_lease(
                &handle,
                "llm.openrouter",
                Some(60),
                "test inference",
                BodyMode::Normal,
            )
            .unwrap();

        assert_eq!(
            registry.can_use_lease(
                &lease.lease_id,
                "llm.openrouter",
                BodyMode::Normal,
                now_secs()
            ),
            SecretDecision::Allowed
        );
        assert_eq!(
            registry.consume_lease(&lease.lease_id, "llm.openrouter", BodyMode::Normal),
            SecretDecision::Allowed
        );
        assert_eq!(
            registry.can_use_lease(
                &lease.lease_id,
                "llm.openrouter",
                BodyMode::Normal,
                now_secs()
            ),
            SecretDecision::Denied(SecretDenyReason::LeaseConsumed)
        );
    }

    #[test]
    fn restricted_mode_blocks_secret_lease_issue() {
        let mut registry = SecretRegistry::new();
        let handle = openrouter_record().handle.clone();
        registry.register(openrouter_record());

        let error = registry
            .issue_lease(
                &handle,
                "llm.openrouter",
                Some(60),
                "test inference",
                BodyMode::Restricted,
            )
            .unwrap_err();

        assert_eq!(error, SecretDenyReason::BodyModeBlocksSecrets);
    }

    #[test]
    fn identity_key_is_not_leasable() {
        let mut registry = SecretRegistry::new();
        let handle = SecretHandle("buster_identity_key".to_string());
        registry.register(SecretRecord::new(
            handle.clone(),
            SecretClass::IdentityKey,
            vec!["identity.prove".to_string()],
            "identity continuity key",
        ));

        let error = registry
            .issue_lease(
                &handle,
                "identity.prove",
                Some(60),
                "prove continuity",
                BodyMode::Normal,
            )
            .unwrap_err();

        assert_eq!(error, SecretDenyReason::IdentityKeyNotLeasable);
    }

    #[test]
    fn capability_must_match_registered_scope() {
        let mut registry = SecretRegistry::new();
        let handle = openrouter_record().handle.clone();
        registry.register(openrouter_record());

        let error = registry
            .issue_lease(
                &handle,
                "github.write",
                Some(60),
                "wrong use",
                BodyMode::Normal,
            )
            .unwrap_err();

        assert_eq!(error, SecretDenyReason::CapabilityNotAllowed);
    }

    #[test]
    fn broker_uses_secret_once_without_returning_plaintext_to_registry() {
        let mut registry = SecretRegistry::new();
        let handle = openrouter_record().handle.clone();
        registry.register(
            openrouter_record().with_fingerprint(SecretFingerprint::from_secret(
                "sk-live-demo-secret",
                b"test-fingerprint-key",
            )),
        );
        let lease = registry
            .issue_lease(
                &handle,
                "llm.openrouter",
                Some(60),
                "test inference",
                BodyMode::Normal,
            )
            .unwrap();
        let mut backend = InMemorySecretBackend::new();
        backend.put_secret(handle, "sk-live-demo-secret".to_string());
        let mut broker = SecretBroker::new(registry, backend);

        let auth_header = broker
            .use_secret_once(
                &lease.lease_id,
                "llm.openrouter",
                BodyMode::Normal,
                |secret| format!("Bearer {secret}"),
            )
            .unwrap();

        assert_eq!(auth_header, "Bearer sk-live-demo-secret");
        assert_eq!(
            broker.registry.can_use_lease(
                &lease.lease_id,
                "llm.openrouter",
                BodyMode::Normal,
                now_secs()
            ),
            SecretDecision::Denied(SecretDenyReason::LeaseConsumed)
        );
    }

    #[test]
    fn redactor_detects_and_redacts_registered_secret_edges() {
        let fingerprint =
            SecretFingerprint::from_secret("sk-live-demo-secret", b"test-fingerprint-key");
        let redactor = SecretRedactor::new(vec![fingerprint], b"test-fingerprint-key".to_vec());
        let text = "leaked sk-live-demo-secret in output";

        assert!(redactor.detects_leak(text));
        let redacted = redactor.redact(text);

        assert!(!redacted.contains("sk-live-demo-secret"));
        assert!(redacted.contains("[SECRET_REDACTED]"));
    }
}
