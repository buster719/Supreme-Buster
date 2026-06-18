//! External LLM access as a body-managed cognitive organ.
//!
//! The LLM is deliberately inside the body layer. Buster's free will may ask
//! for cognition, but the body owns network access, secret use, context size,
//! cost, degradation, and audit boundaries.

use crate::network::{EgressBroker, EgressRequest, NetworkPolicy};
use crate::resources::ResourceBudget;
use crate::runtime::{
    BackendCompletion, BackendFailure, RuntimeBackend, RuntimeKind, RuntimeRequest,
};
use crate::secrets::{SecretBackend, SecretBroker, SecretBrokerError};
use crate::state::BodyMode;
use std::env;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmProvider {
    pub name: String,
    pub model: String,
    pub context_window_tokens: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmRequestPolicy {
    pub provider: LlmProvider,
    pub budget: ResourceBudget,
    pub allow_identity_context: bool,
    pub allow_secret_context: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmMessage {
    pub role: LlmRole,
    pub content: String,
}

impl LlmMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::Assistant,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmRole {
    System,
    User,
    Assistant,
}

impl LlmRole {
    fn as_openrouter_role(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmPrompt {
    pub messages: Vec<LlmMessage>,
    pub max_tokens: Option<u32>,
}

impl LlmPrompt {
    pub fn single_user(content: impl Into<String>) -> Self {
        Self {
            messages: vec![LlmMessage::user(content)],
            max_tokens: Some(512),
        }
    }

    pub fn with_system(mut self, content: impl Into<String>) -> Self {
        self.messages.insert(0, LlmMessage::system(content));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmCompletion {
    pub content: String,
    pub model: String,
    pub provider: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryInfoRequest {
    pub question: String,
    pub context: Option<String>,
    pub max_tokens: Option<u32>,
}

impl SecondaryInfoRequest {
    pub fn new(question: impl Into<String>) -> Self {
        Self {
            question: question.into(),
            context: None,
            max_tokens: Some(768),
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryInfoReport {
    pub content: String,
    pub source_type: &'static str,
    pub needs_verification: bool,
    pub authority: &'static str,
    pub model: String,
    pub provider: Option<String>,
    pub total_tokens: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SecondaryInfoProvider {
    client: OpenRouterClient,
    provider: LlmProvider,
}

impl SecondaryInfoProvider {
    pub fn new(client: OpenRouterClient, provider: LlmProvider) -> Self {
        Self { client, provider }
    }

    pub fn query(&self, request: SecondaryInfoRequest) -> Result<SecondaryInfoReport, LlmError> {
        let completion = self
            .client
            .chat(&self.provider, prompt_for_secondary_info(request))?;
        Ok(SecondaryInfoReport {
            content: completion.content,
            source_type: "llm_secondary",
            needs_verification: true,
            authority: "hypothesis_or_explanation",
            model: completion.model,
            provider: completion.provider,
            total_tokens: completion.total_tokens,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmError {
    MissingApiKeyEnv {
        env_var: String,
    },
    Http {
        status: Option<u16>,
        message: String,
    },
    EmptyChoices,
    InvalidResponse {
        message: String,
    },
    SecretBroker {
        message: String,
    },
    Egress {
        message: String,
    },
}

impl fmt::Display for LlmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKeyEnv { env_var } => {
                write!(formatter, "missing API key environment variable {env_var}")
            }
            Self::Http { status, message } => match status {
                Some(status) => write!(formatter, "LLM HTTP error {status}: {message}"),
                None => write!(formatter, "LLM HTTP error: {message}"),
            },
            Self::EmptyChoices => formatter.write_str("LLM response contained no choices"),
            Self::InvalidResponse { message } => {
                write!(formatter, "invalid LLM response: {message}")
            }
            Self::SecretBroker { message } => write!(formatter, "secret broker error: {message}"),
            Self::Egress { message } => write!(formatter, "egress broker error: {message}"),
        }
    }
}

impl std::error::Error for LlmError {}

#[derive(Clone)]
pub struct OpenRouterClient {
    api_key: String,
    client: reqwest::blocking::Client,
    endpoint: String,
    auth: LlmAuth,
    referer: Option<String>,
    title: Option<String>,
    provider_order: Option<Vec<String>>,
    egress_broker: Option<Arc<EgressBroker>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LlmAuth {
    Bearer,
    ApiKeyHeader(String),
}

impl fmt::Debug for OpenRouterClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenRouterClient")
            .field("api_key", &"[REDACTED]")
            .field("endpoint", &self.endpoint)
            .field("auth", &self.auth)
            .field("referer", &self.referer)
            .field("title", &self.title)
            .field("provider_order", &self.provider_order)
            .field("egress_broker", &self.egress_broker)
            .finish()
    }
}

impl OpenRouterClient {
    pub const DEFAULT_ENDPOINT: &'static str = "https://openrouter.ai/api/v1/chat/completions";
    pub const DEFAULT_API_KEY_ENV: &'static str = "OPENROUTER_API_KEY";

    pub fn from_env() -> Result<Self, LlmError> {
        Self::from_env_var(Self::DEFAULT_API_KEY_ENV)
    }

    pub fn from_env_var(env_var: &str) -> Result<Self, LlmError> {
        let api_key = env::var(env_var).map_err(|_| LlmError::MissingApiKeyEnv {
            env_var: env_var.to_string(),
        })?;
        Ok(Self::new(api_key))
    }

    pub fn new(api_key: impl Into<String>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            api_key: api_key.into(),
            client,
            endpoint: Self::DEFAULT_ENDPOINT.to_string(),
            auth: LlmAuth::Bearer,
            referer: None,
            title: Some("Buster".to_string()),
            provider_order: None,
            egress_broker: None,
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    pub fn with_api_key_header(mut self, header_name: impl Into<String>) -> Self {
        self.auth = LlmAuth::ApiKeyHeader(header_name.into());
        self
    }

    pub fn with_referer(mut self, referer: impl Into<String>) -> Self {
        self.referer = Some(referer.into());
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_provider_order<I, S>(mut self, providers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let providers = providers
            .into_iter()
            .map(Into::into)
            .filter(|provider: &String| !provider.trim().is_empty())
            .collect::<Vec<_>>();
        if !providers.is_empty() {
            self.provider_order = Some(providers);
        }
        self
    }

    pub fn with_egress_broker(mut self, broker: Arc<EgressBroker>) -> Self {
        self.egress_broker = Some(broker);
        self
    }

    pub fn chat(
        &self,
        provider: &LlmProvider,
        prompt: LlmPrompt,
    ) -> Result<LlmCompletion, LlmError> {
        let request = OpenRouterChatRequest {
            model: provider.model.clone(),
            messages: prompt
                .messages
                .into_iter()
                .map(OpenRouterMessage::from)
                .collect(),
            max_tokens: prompt.max_tokens,
            temperature: Some(0.2),
            provider: self
                .provider_order
                .clone()
                .map(|order| OpenRouterProviderRouting { order }),
        };

        let request_body =
            serde_json::to_value(&request).map_err(|error| LlmError::InvalidResponse {
                message: error.to_string(),
            })?;

        let (status, body) = if let Some(broker) = &self.egress_broker {
            let mut egress_request = EgressRequest::post_json("llm", &self.endpoint, request_body);
            egress_request = match &self.auth {
                LlmAuth::Bearer => egress_request.with_bearer_token(self.api_key.clone()),
                LlmAuth::ApiKeyHeader(header_name) => {
                    egress_request.with_header(header_name, self.api_key.clone())
                }
            };
            if let Some(referer) = &self.referer {
                egress_request = egress_request.with_header("HTTP-Referer", referer);
            }
            if let Some(title) = &self.title {
                egress_request = egress_request.with_header("X-Title", title);
            }

            let response = broker
                .post_json(egress_request)
                .map_err(|error| LlmError::Egress {
                    message: error.to_string(),
                })?;
            (response.status, response.body)
        } else {
            let mut builder = self
                .client
                .post(&self.endpoint)
                .header("Accept-Encoding", "identity")
                .json(&request);
            builder = match &self.auth {
                LlmAuth::Bearer => builder.bearer_auth(&self.api_key),
                LlmAuth::ApiKeyHeader(header_name) => builder.header(header_name, &self.api_key),
            };

            if let Some(referer) = &self.referer {
                builder = builder.header("HTTP-Referer", referer);
            }
            if let Some(title) = &self.title {
                builder = builder.header("X-Title", title);
            }

            let response = builder.send().map_err(|error| LlmError::Http {
                status: error.status().map(|status| status.as_u16()),
                message: error.to_string(),
            })?;

            let status = response.status().as_u16();
            let body_bytes = response.bytes().map_err(|error| LlmError::Http {
                status: Some(status),
                message: error.to_string(),
            })?;
            let body = String::from_utf8_lossy(&body_bytes).into_owned();
            (status, body)
        };

        if !(200..300).contains(&status) {
            return Err(LlmError::Http {
                status: Some(status),
                message: sanitize_error_body(&body),
            });
        }

        let parsed: OpenRouterChatResponse =
            serde_json::from_str(&body).map_err(|error| LlmError::InvalidResponse {
                message: error.to_string(),
            })?;
        let choice = parsed
            .choices
            .into_iter()
            .next()
            .ok_or(LlmError::EmptyChoices)?;
        let content = openrouter_message_content(choice.message.content)?;
        Ok(LlmCompletion {
            content,
            model: parsed.model.unwrap_or_else(|| provider.model.clone()),
            provider: parsed.provider,
            prompt_tokens: parsed.usage.as_ref().and_then(|usage| usage.prompt_tokens),
            completion_tokens: parsed
                .usage
                .as_ref()
                .and_then(|usage| usage.completion_tokens),
            total_tokens: parsed.usage.and_then(|usage| usage.total_tokens),
        })
    }
}

#[derive(Debug, Clone)]
pub struct OpenRouterLlmBackend {
    client: OpenRouterClient,
    provider: LlmProvider,
    system_prompt: Option<String>,
    max_tokens: Option<u32>,
}

impl OpenRouterLlmBackend {
    pub fn new(client: OpenRouterClient, provider: LlmProvider) -> Self {
        Self {
            client,
            provider,
            system_prompt: Some(default_buster_system_prompt()),
            max_tokens: Some(512),
        }
    }

    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(system_prompt.into());
        self
    }

    pub fn without_system_prompt(mut self) -> Self {
        self.system_prompt = None;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    fn prompt_for_request(&self, request: &RuntimeRequest) -> LlmPrompt {
        let mut prompt = LlmPrompt {
            messages: vec![LlmMessage::user(request.input_summary.clone())],
            max_tokens: self.max_tokens,
        };
        if let Some(system_prompt) = &self.system_prompt {
            prompt = prompt.with_system(system_prompt.clone());
        }
        prompt
    }
}

impl RuntimeBackend for OpenRouterLlmBackend {
    fn runtime_kind(&self) -> RuntimeKind {
        RuntimeKind::ExternalLlm
    }

    fn execute(&self, request: &RuntimeRequest) -> Result<BackendCompletion, BackendFailure> {
        let started = Instant::now();
        let completion = self
            .client
            .chat(&self.provider, self.prompt_for_request(request))
            .map_err(|error| BackendFailure {
                reason: error.to_string(),
            })?;

        let mut usage = request.estimated_usage.clone();
        usage.wall_clock_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        usage.tokens = completion.total_tokens.unwrap_or(usage.tokens);
        usage.network_bytes = completion.content.len().min(u64::MAX as usize) as u64;

        Ok(BackendCompletion {
            output_summary: completion.content,
            actual_usage: usage,
        })
    }
}

pub struct BrokeredOpenRouterLlmBackend<B>
where
    B: SecretBackend,
{
    broker: Arc<Mutex<SecretBroker<B>>>,
    lease_id: String,
    capability: String,
    body_mode: BodyMode,
    provider: LlmProvider,
    endpoint: String,
    referer: Option<String>,
    title: Option<String>,
    egress_broker: Option<Arc<EgressBroker>>,
    system_prompt: Option<String>,
    max_tokens: Option<u32>,
}

impl<B> fmt::Debug for BrokeredOpenRouterLlmBackend<B>
where
    B: SecretBackend,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokeredOpenRouterLlmBackend")
            .field("lease_id", &self.lease_id)
            .field("capability", &self.capability)
            .field("body_mode", &self.body_mode)
            .field("provider", &self.provider)
            .field("endpoint", &self.endpoint)
            .field("referer", &self.referer)
            .field("title", &self.title)
            .field("egress_broker", &self.egress_broker)
            .finish()
    }
}

impl<B> BrokeredOpenRouterLlmBackend<B>
where
    B: SecretBackend,
{
    pub fn new(
        broker: Arc<Mutex<SecretBroker<B>>>,
        lease_id: impl Into<String>,
        capability: impl Into<String>,
        provider: LlmProvider,
    ) -> Self {
        Self {
            broker,
            lease_id: lease_id.into(),
            capability: capability.into(),
            body_mode: BodyMode::Normal,
            provider,
            endpoint: OpenRouterClient::DEFAULT_ENDPOINT.to_string(),
            referer: None,
            title: Some("Buster".to_string()),
            egress_broker: Some(Arc::new(EgressBroker::new(NetworkPolicy {
                allowed_hosts: vec!["openrouter.ai".to_string()],
                deny_private_networks: true,
                max_response_bytes: Some(512 * 1024),
            }))),
            system_prompt: Some(default_buster_system_prompt()),
            max_tokens: Some(512),
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    pub fn with_body_mode(mut self, mode: BodyMode) -> Self {
        self.body_mode = mode;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_egress_broker(mut self, broker: Arc<EgressBroker>) -> Self {
        self.egress_broker = Some(broker);
        self
    }

    pub fn without_system_prompt(mut self) -> Self {
        self.system_prompt = None;
        self
    }

    fn prompt_for_request(&self, request: &RuntimeRequest) -> LlmPrompt {
        let mut prompt = LlmPrompt {
            messages: vec![LlmMessage::user(request.input_summary.clone())],
            max_tokens: self.max_tokens,
        };
        if let Some(system_prompt) = &self.system_prompt {
            prompt = prompt.with_system(system_prompt.clone());
        }
        prompt
    }

    fn broker_error(error: SecretBrokerError) -> BackendFailure {
        BackendFailure {
            reason: LlmError::SecretBroker {
                message: format!("{error:?}"),
            }
            .to_string(),
        }
    }
}

impl<B> RuntimeBackend for BrokeredOpenRouterLlmBackend<B>
where
    B: SecretBackend,
{
    fn runtime_kind(&self) -> RuntimeKind {
        RuntimeKind::ExternalLlm
    }

    fn execute(&self, request: &RuntimeRequest) -> Result<BackendCompletion, BackendFailure> {
        let started = Instant::now();
        let prompt = self.prompt_for_request(request);
        let provider = self.provider.clone();
        let endpoint = self.endpoint.clone();
        let referer = self.referer.clone();
        let title = self.title.clone();
        let egress_broker = self.egress_broker.clone();
        let mut broker = self.broker.lock().map_err(|_| BackendFailure {
            reason: "secret broker lock poisoned".to_string(),
        })?;

        let completion = broker
            .use_secret_once(
                &self.lease_id,
                &self.capability,
                self.body_mode,
                |api_key| {
                    let mut client =
                        OpenRouterClient::new(api_key.to_string()).with_endpoint(endpoint.clone());
                    if let Some(referer) = referer.clone() {
                        client = client.with_referer(referer);
                    }
                    if let Some(title) = title.clone() {
                        client = client.with_title(title);
                    }
                    if let Some(egress_broker) = egress_broker.clone() {
                        client = client.with_egress_broker(egress_broker);
                    }
                    client.chat(&provider, prompt)
                },
            )
            .map_err(Self::broker_error)?
            .map_err(|error| BackendFailure {
                reason: error.to_string(),
            })?;

        let mut usage = request.estimated_usage.clone();
        usage.wall_clock_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        usage.tokens = completion.total_tokens.unwrap_or(usage.tokens);
        usage.network_bytes = completion.content.len().min(u64::MAX as usize) as u64;

        Ok(BackendCompletion {
            output_summary: completion.content,
            actual_usage: usage,
        })
    }
}

fn default_buster_system_prompt() -> String {
    "You are Buster's external cognitive organ. Treat Buster's body, governance, identity, memory, and secrets as protected boundaries. Do not request or reveal secrets. Return concise, auditable reasoning for the requested task.".to_string()
}

fn secondary_info_system_prompt() -> String {
    "You are a secondary information provider for Buster. Return explanations, hypotheses, summaries, and verification leads only. Do not claim final authority. Mark uncertainty, avoid secrets, and suggest what evidence would verify or falsify the answer.".to_string()
}

pub fn prompt_for_secondary_info(request: SecondaryInfoRequest) -> LlmPrompt {
    let mut user = String::new();
    if let Some(context) = request.context {
        user.push_str("Context:\n");
        user.push_str(&context);
        user.push_str("\n\n");
    }
    user.push_str("Question:\n");
    user.push_str(&request.question);
    user.push_str("\n\nReturn: concise answer, uncertainty, and verification leads.");

    LlmPrompt {
        messages: vec![
            LlmMessage::system(secondary_info_system_prompt()),
            LlmMessage::user(user),
        ],
        max_tokens: request.max_tokens,
    }
}

fn sanitize_error_body(body: &str) -> String {
    let mut sanitized = body.replace('\n', " ");
    if sanitized.len() > 512 {
        sanitized.truncate(512);
        sanitized.push_str("...");
    }
    sanitized
}

#[derive(Debug, serde::Serialize)]
struct OpenRouterChatRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<OpenRouterProviderRouting>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct OpenRouterProviderRouting {
    order: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct OpenRouterMessage {
    role: String,
    content: String,
}

impl From<LlmMessage> for OpenRouterMessage {
    fn from(message: LlmMessage) -> Self {
        Self {
            role: message.role.as_openrouter_role().to_string(),
            content: message.content,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct OpenRouterChatResponse {
    choices: Vec<OpenRouterChoice>,
    model: Option<String>,
    provider: Option<String>,
    usage: Option<OpenRouterUsage>,
}

#[derive(Debug, serde::Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterAssistantMessage,
}

#[derive(Debug, serde::Deserialize)]
struct OpenRouterAssistantMessage {
    content: serde_json::Value,
}

#[derive(Debug, serde::Deserialize)]
struct OpenRouterUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

fn openrouter_message_content(value: serde_json::Value) -> Result<String, LlmError> {
    match value {
        serde_json::Value::String(content) => Ok(content),
        serde_json::Value::Array(parts) => {
            let mut content = String::new();
            for part in parts {
                if let Some(text) = part.get("text").and_then(|value| value.as_str()) {
                    content.push_str(text);
                } else if let Some(text) = part.as_str() {
                    content.push_str(text);
                }
            }
            if content.is_empty() {
                Err(LlmError::InvalidResponse {
                    message: "assistant content array contained no text".to_string(),
                })
            } else {
                Ok(content)
            }
        }
        serde_json::Value::Null => Err(LlmError::InvalidResponse {
            message: "assistant content was null".to_string(),
        }),
        other => Err(LlmError::InvalidResponse {
            message: format!("unsupported assistant content shape: {other}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{
        InMemorySecretBackend, SecretBackend, SecretClass, SecretHandle, SecretRecord,
        SecretRegistry,
    };

    #[test]
    fn brokered_openrouter_backend_denies_secret_in_restricted_mode_before_network() {
        let handle = SecretHandle("openrouter_api_key".to_string());
        let mut registry = SecretRegistry::new();
        registry.register(SecretRecord::new(
            handle.clone(),
            SecretClass::LlmApiKey,
            vec!["llm.openrouter".to_string()],
            "OpenRouter key",
        ));
        let lease = registry
            .issue_lease(
                &handle,
                "llm.openrouter",
                Some(60),
                "test",
                BodyMode::Normal,
            )
            .unwrap();
        let mut backend_store = InMemorySecretBackend::new();
        backend_store.put_secret(handle, "sk-test".to_string());
        let broker = Arc::new(Mutex::new(SecretBroker::new(registry, backend_store)));
        let provider = LlmProvider {
            name: "openrouter".to_string(),
            model: "openai/gpt-4o-mini".to_string(),
            context_window_tokens: 128_000,
        };
        let backend =
            BrokeredOpenRouterLlmBackend::new(broker, lease.lease_id, "llm.openrouter", provider)
                .with_body_mode(BodyMode::Restricted);
        let request = RuntimeRequest::new(
            crate::host_api::BodyScope::new("buster", "main", "test"),
            RuntimeKind::ExternalLlm,
            "external_cognition",
            "hello",
        );

        let error = backend.execute(&request).unwrap_err();

        assert!(error.reason.contains("BodyModeBlocksSecrets"));
    }

    #[test]
    fn secondary_info_prompt_marks_llm_as_non_authoritative() {
        let prompt = prompt_for_secondary_info(
            SecondaryInfoRequest::new("What should Buster learn next?")
                .with_context("Recent conversations mention human-AI relations."),
        );

        assert_eq!(prompt.max_tokens, Some(768));
        assert!(prompt.messages[0].content.contains("secondary information"));
        assert!(prompt.messages[0]
            .content
            .contains("Do not claim final authority"));
        assert!(prompt.messages[1].content.contains("verification leads"));
    }
}
