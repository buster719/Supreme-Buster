//! Adapter seam for reusing IronClaw's secret stores.
//!
//! Buster's body layer keeps its own lease and governance checks in
//! [`crate::secrets::SecretBroker`]. This module adapts an external IronClaw
//! resolver into Buster's small synchronous [`crate::secrets::SecretBackend`]
//! trait without making `buster_body` depend on the entire IronClaw app.

use std::collections::HashMap;
use std::fmt;

use crate::secrets::{SecretBackend, SecretHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IronClawSecretBackendError {
    Missing { handle: SecretHandle },
    StoreUnavailable { reason: String },
}

impl fmt::Display for IronClawSecretBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { handle } => {
                write!(formatter, "IronClaw secret `{}` is missing", handle.0)
            }
            Self::StoreUnavailable { reason } => {
                write!(formatter, "IronClaw secret store unavailable: {reason}")
            }
        }
    }
}

impl std::error::Error for IronClawSecretBackendError {}

pub trait IronClawSecretResolver {
    fn resolve_secret(&self, handle: &SecretHandle) -> Result<String, IronClawSecretBackendError>;
}

impl<F> IronClawSecretResolver for F
where
    F: Fn(&SecretHandle) -> Result<String, IronClawSecretBackendError>,
{
    fn resolve_secret(&self, handle: &SecretHandle) -> Result<String, IronClawSecretBackendError> {
        self(handle)
    }
}

#[derive(Debug, Clone)]
pub struct IronClawSecretBackend<R> {
    resolver: R,
    setup_overlay: HashMap<SecretHandle, String>,
}

impl<R> IronClawSecretBackend<R>
where
    R: IronClawSecretResolver,
{
    pub fn new(resolver: R) -> Self {
        Self {
            resolver,
            setup_overlay: HashMap::new(),
        }
    }
}

impl<R> SecretBackend for IronClawSecretBackend<R>
where
    R: IronClawSecretResolver,
{
    fn get_secret(&self, handle: &SecretHandle) -> Option<String> {
        self.setup_overlay
            .get(handle)
            .cloned()
            .or_else(|| self.resolver.resolve_secret(handle).ok())
    }

    fn put_secret(&mut self, handle: SecretHandle, secret: String) {
        self.setup_overlay.insert(handle, secret);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{SecretBroker, SecretClass, SecretRecord, SecretRegistry};
    use crate::state::BodyMode;

    #[test]
    fn ironclaw_backend_can_feed_buster_secret_broker() {
        let handle = SecretHandle("openrouter_api_key".to_string());
        let mut registry = SecretRegistry::new();
        registry.register(SecretRecord::new(
            handle.clone(),
            SecretClass::LlmApiKey,
            vec!["llm.openrouter".to_string()],
            "stored in IronClaw secret store",
        ));
        let lease = registry
            .issue_lease(
                &handle,
                "llm.openrouter",
                Some(60),
                "test inference",
                BodyMode::Normal,
            )
            .unwrap();

        let backend = IronClawSecretBackend::new(|candidate: &SecretHandle| {
            if candidate.0 == "openrouter_api_key" {
                Ok("sk-from-ironclaw".to_string())
            } else {
                Err(IronClawSecretBackendError::Missing {
                    handle: candidate.clone(),
                })
            }
        });
        let mut broker = SecretBroker::new(registry, backend);

        let header = broker
            .use_secret_once(
                &lease.lease_id,
                "llm.openrouter",
                BodyMode::Normal,
                |secret| format!("Bearer {secret}"),
            )
            .unwrap();

        assert_eq!(header, "Bearer sk-from-ironclaw");
    }
}
