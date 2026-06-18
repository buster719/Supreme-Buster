use std::path::Path;

use buster_body::{
    BodyGate, BodyGateError, BodyMode, BodyScope, EgressBroker, MemoryWriteIntent, NetworkPolicy,
    SqliteBodyStore,
};

pub(crate) const BODY_GATE_DB: &str = "body-gate.sqlite3";

pub(crate) fn scope(task_id: impl Into<String>) -> BodyScope {
    BodyScope::new("buster", "main", task_id)
}

pub(crate) fn open_gate(root: &Path) -> Result<BodyGate<SqliteBodyStore>, BodyGateError> {
    let store = SqliteBodyStore::open(root.join("state").join(BODY_GATE_DB))
        .map_err(BodyGateError::Store)?;
    Ok(BodyGate::new(store, BodyMode::Normal))
}

pub fn preflight_network(
    root: &Path,
    url: &str,
    allowed_hosts: &[&str],
) -> Result<String, BodyGateError> {
    let broker = EgressBroker::new(NetworkPolicy {
        allowed_hosts: allowed_hosts
            .iter()
            .map(|host| (*host).to_string())
            .collect(),
        deny_private_networks: true,
        max_response_bytes: Some(2 * 1024 * 1024),
    });
    open_gate(root)?.preflight_network(&broker, url)
}

pub(crate) fn record_memory_write(
    root: &Path,
    kind: &str,
    source: &str,
    content: &str,
) -> Result<(), BodyGateError> {
    let gate = open_gate(root)?;
    gate.record_memory_write(
        &scope(format!("memory.{kind}")),
        MemoryWriteIntent::new(kind, source, content),
    )
}
