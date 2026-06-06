use buster_body::capabilities::CapabilityLease;
use buster_body::host_api::BodyScope;
use buster_body::llm::{LlmProvider, OpenRouterClient, OpenRouterLlmBackend};
use buster_body::resources::{ResourceBudget, ResourceUsage};
use buster_body::runtime::{
    BodyRuntime, RuntimeKind, RuntimeOutcome, RuntimePolicy, RuntimeRequest,
};

fn main() {
    let client =
        OpenRouterClient::from_env().expect("set OPENROUTER_API_KEY before running this example");
    let provider = LlmProvider {
        name: "openrouter".to_string(),
        model: std::env::var("BUSTER_OPENROUTER_MODEL")
            .unwrap_or_else(|_| "openai/gpt-4o-mini".to_string()),
        context_window_tokens: 128_000,
    };
    let backend = OpenRouterLlmBackend::new(client, provider).with_max_tokens(128);
    let runtime = BodyRuntime::new(backend);

    let scope = BodyScope::new("buster", "main", "openrouter-smoke");
    let capability = "external_cognition";
    let request = RuntimeRequest::new(
        scope.clone(),
        RuntimeKind::ExternalLlm,
        capability,
        "用一句中文说明 Buster 的 runtime 为什么必须经过身体层。",
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
        capability_name: capability.to_string(),
        expires_at: None,
        reason: "OpenRouter smoke test".to_string(),
    });

    match runtime.invoke(request, &policy) {
        RuntimeOutcome::Completed(completion) => {
            println!("{}", completion.output_summary);
            println!("usage: {:?}", completion.actual_usage);
        }
        RuntimeOutcome::Blocked(block) => {
            eprintln!("blocked: {:?}", block.reason);
            std::process::exit(2);
        }
        RuntimeOutcome::Failed(failure) => {
            eprintln!("failed: {}", failure.reason);
            std::process::exit(1);
        }
    }
}
