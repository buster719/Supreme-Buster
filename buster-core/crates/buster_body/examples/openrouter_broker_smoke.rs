use std::sync::{Arc, Mutex};

use buster_body::capabilities::CapabilityLease;
use buster_body::host_api::BodyScope;
use buster_body::llm::{BrokeredOpenRouterLlmBackend, LlmProvider};
use buster_body::resources::{ResourceBudget, ResourceUsage};
use buster_body::runtime::{RuntimeKind, RuntimeOutcome, RuntimePolicy, RuntimeRequest};
use buster_body::{
    BodyGate, InMemorySecretBackend, SecretBackend, SecretBroker, SecretClass, SecretHandle,
    SecretRecord, SecretRegistry, SqliteBodyStore,
};

fn main() {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .expect("set OPENROUTER_API_KEY before running this development smoke test");
    let secret_handle = SecretHandle("openrouter_api_key".to_string());
    let capability = "llm.openrouter";

    let mut registry = SecretRegistry::new();
    registry.register(SecretRecord::new(
        secret_handle.clone(),
        SecretClass::LlmApiKey,
        vec![capability.to_string()],
        "OpenRouter API key stored behind Buster body secret broker",
    ));
    let secret_lease = registry
        .issue_lease(
            &secret_handle,
            capability,
            Some(600),
            "OpenRouter broker smoke test",
            buster_body::BodyMode::Normal,
        )
        .expect("secret lease should be issued");

    let mut secret_backend = InMemorySecretBackend::new();
    secret_backend.put_secret(secret_handle, api_key);
    let broker = Arc::new(Mutex::new(SecretBroker::new(registry, secret_backend)));

    let provider = LlmProvider {
        name: "openrouter".to_string(),
        model: std::env::var("BUSTER_OPENROUTER_MODEL")
            .or_else(|_| std::env::var("OPENROUTER_MODEL"))
            .unwrap_or_else(|_| "openai/gpt-4o-mini".to_string()),
        context_window_tokens: 128_000,
    };
    let backend =
        BrokeredOpenRouterLlmBackend::new(broker, secret_lease.lease_id, capability, provider)
            .with_max_tokens(128);
    let store = SqliteBodyStore::open_in_memory().expect("body gate store should open");
    let gate = BodyGate::new(store, buster_body::BodyMode::Normal);

    let scope = BodyScope::new("buster", "main", "openrouter-broker-smoke");
    let runtime_capability = "external_cognition";
    let request = RuntimeRequest::new(
        scope.clone(),
        RuntimeKind::ExternalLlm,
        runtime_capability,
        "用一句中文说明 Buster 的 LLM 调用为什么要经过 SecretBroker。",
    )
    .with_network()
    .with_secret()
    .with_estimated_usage(ResourceUsage {
        tokens: 512,
        wall_clock_ms: 30_000,
        network_bytes: 16 * 1024,
        usd: 0.01,
    });

    let policy = RuntimePolicy::deny_by_default(ResourceBudget {
        max_tokens: Some(2_000),
        max_wall_clock_ms: Some(60_000),
        max_network_bytes: Some(256 * 1024),
        max_usd: Some(0.05),
    })
    .with_network()
    .with_secret()
    .with_lease(CapabilityLease {
        scope,
        capability_name: runtime_capability.to_string(),
        expires_at: None,
        reason: "OpenRouter broker smoke test".to_string(),
    });

    match gate.invoke_runtime(request, &policy, backend) {
        Ok(RuntimeOutcome::Completed(completion)) => {
            println!("{}", completion.output_summary);
            println!("usage: {:?}", completion.actual_usage);
            println!(
                "body gate audit events: {}",
                gate.store()
                    .audit_event_count()
                    .expect("audit count should be readable")
            );
        }
        Ok(RuntimeOutcome::Blocked(block)) => {
            eprintln!("blocked: {:?}", block.reason);
            std::process::exit(2);
        }
        Ok(RuntimeOutcome::Failed(failure)) => {
            eprintln!("failed: {}", failure.reason);
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("body gate error: {error}");
            std::process::exit(1);
        }
    }
}
