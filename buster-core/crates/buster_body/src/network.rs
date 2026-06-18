//! Network boundary, egress policy, and HTTP broker.
//!
//! The broker is the body-layer choke point for outbound HTTP. Callers should
//! ask it to perform network requests instead of creating raw clients.

use std::env;
use std::fmt;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::process::{Command, Stdio};

use crate::resources::ResourceUsage;
use crate::runtime::{
    BackendCompletion, BackendFailure, RuntimeBackend, RuntimeKind, RuntimeRequest,
};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkPolicy {
    pub allowed_hosts: Vec<String>,
    pub deny_private_networks: bool,
    pub max_response_bytes: Option<u64>,
}

impl NetworkPolicy {
    pub fn deny_all() -> Self {
        Self {
            allowed_hosts: Vec::new(),
            deny_private_networks: true,
            max_response_bytes: Some(1024 * 1024),
        }
    }

    pub fn allows_host(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        self.allowed_hosts
            .iter()
            .any(|allowed| host_matches(&host, &allowed.to_ascii_lowercase()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressRequest {
    pub unit_id: String,
    pub url: String,
    pub bearer_token: Option<String>,
    pub headers: Vec<(String, String)>,
    pub json_body: serde_json::Value,
}

impl EgressRequest {
    pub fn post_json(
        unit_id: impl Into<String>,
        url: impl Into<String>,
        json_body: serde_json::Value,
    ) -> Self {
        Self {
            unit_id: unit_id.into(),
            url: url.into(),
            bearer_token: None,
            headers: Vec::new(),
            json_body,
        }
    }

    pub fn with_bearer_token(mut self, bearer_token: impl Into<String>) -> Self {
        self.bearer_token = Some(bearer_token.into());
        self
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressResponse {
    pub status: u16,
    pub body: String,
    pub bytes: u64,
    pub host: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgressError {
    InvalidUrl {
        url: String,
        reason: String,
    },
    MissingHost {
        url: String,
    },
    HostNotAllowed {
        host: String,
    },
    PrivateNetworkBlocked {
        host: String,
    },
    ResponseTooLarge {
        limit: u64,
        actual: u64,
    },
    Http {
        status: Option<u16>,
        message: String,
    },
}

impl fmt::Display for EgressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl { url, reason } => write!(formatter, "invalid URL `{url}`: {reason}"),
            Self::MissingHost { url } => write!(formatter, "URL `{url}` has no host"),
            Self::HostNotAllowed { host } => write!(formatter, "host `{host}` is not allowed"),
            Self::PrivateNetworkBlocked { host } => {
                write!(
                    formatter,
                    "private or local network host `{host}` is blocked"
                )
            }
            Self::ResponseTooLarge { limit, actual } => {
                write!(
                    formatter,
                    "response too large: {actual} bytes exceeds {limit}"
                )
            }
            Self::Http { status, message } => match status {
                Some(status) => write!(formatter, "HTTP error {status}: {message}"),
                None => write!(formatter, "HTTP error: {message}"),
            },
        }
    }
}

impl std::error::Error for EgressError {}

#[derive(Clone)]
pub struct EgressBroker {
    policy: NetworkPolicy,
    client: reqwest::blocking::Client,
}

impl fmt::Debug for EgressBroker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EgressBroker")
            .field("policy", &self.policy)
            .finish()
    }
}

impl EgressBroker {
    pub fn new(policy: NetworkPolicy) -> Self {
        let mut builder = reqwest::blocking::Client::builder();
        if let Some(proxy_url) = proxy_url_from_env() {
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                builder = builder.proxy(proxy);
            }
        }
        Self {
            policy,
            client: builder
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
        }
    }

    pub fn policy(&self) -> &NetworkPolicy {
        &self.policy
    }

    pub fn preflight_url(&self, url: &str) -> Result<String, EgressError> {
        let parsed = reqwest::Url::parse(url).map_err(|error| EgressError::InvalidUrl {
            url: url.to_string(),
            reason: error.to_string(),
        })?;
        let host = parsed
            .host_str()
            .ok_or_else(|| EgressError::MissingHost {
                url: url.to_string(),
            })?
            .to_ascii_lowercase();

        if self.policy.deny_private_networks && is_private_or_local_host(&host) {
            return Err(EgressError::PrivateNetworkBlocked { host });
        }

        if !self.policy.allows_host(&host) {
            return Err(EgressError::HostNotAllowed { host });
        }

        Ok(host)
    }

    pub fn post_json(&self, request: EgressRequest) -> Result<EgressResponse, EgressError> {
        let host = self.preflight_url(&request.url)?;
        let headers = request_headers(&request);
        let mut builder = self.client.post(&request.url).json(&request.json_body);

        for (name, value) in &headers {
            builder = builder.header(name, value);
        }

        let response = match builder.send() {
            Ok(response) => response,
            Err(error) => {
                if python_fallback_enabled() {
                    return self.post_json_with_python_fallback(
                        &request.url,
                        &headers,
                        &request.json_body,
                        &host,
                        format_error_chain(&error),
                    );
                }
                return Err(EgressError::Http {
                    status: error.status().map(|status| status.as_u16()),
                    message: format_error_chain(&error),
                });
            }
        };
        let status = response.status().as_u16();

        if let Some(limit) = self.policy.max_response_bytes {
            if let Some(length) = response.content_length() {
                if length > limit {
                    return Err(EgressError::ResponseTooLarge {
                        limit,
                        actual: length,
                    });
                }
            }
        }

        let bytes = response.bytes().map_err(|error| EgressError::Http {
            status: Some(status),
            message: format_error_chain(&error),
        })?;
        let actual = bytes.len().min(u64::MAX as usize) as u64;

        if let Some(limit) = self.policy.max_response_bytes {
            if actual > limit {
                return Err(EgressError::ResponseTooLarge { limit, actual });
            }
        }

        let body = String::from_utf8_lossy(&bytes).to_string();

        Ok(EgressResponse {
            status,
            body,
            bytes: actual,
            host,
        })
    }

    fn post_json_with_python_fallback(
        &self,
        url: &str,
        headers: &[(String, String)],
        json_body: &serde_json::Value,
        host: &str,
        primary_error: String,
    ) -> Result<EgressResponse, EgressError> {
        let python = python_program().ok_or_else(|| EgressError::Http {
            status: None,
            message: format!("{primary_error}; python fallback unavailable"),
        })?;
        let request = serde_json::json!({
            "url": url,
            "headers": headers,
            "json_body": json_body,
            "timeout_secs": python_fallback_timeout_secs(),
        });
        let script = r#"
import json, ssl, sys, urllib.error, urllib.request
payload = json.load(sys.stdin)
headers = {str(k): str(v) for k, v in payload.get("headers", [])}
headers.setdefault("Content-Type", "application/json")
body = json.dumps(payload.get("json_body"), ensure_ascii=False).encode("utf-8")
request = urllib.request.Request(
    payload["url"],
    data=body,
    headers=headers,
    method="POST",
)
try:
    with urllib.request.urlopen(
        request,
        timeout=float(payload.get("timeout_secs") or 120),
        context=ssl.create_default_context(),
    ) as response:
        raw = response.read()
        print(json.dumps({
            "status": response.status,
            "body": raw.decode("utf-8", errors="replace"),
        }))
except urllib.error.HTTPError as error:
    raw = error.read()
    print(json.dumps({
        "status": error.code,
        "body": raw.decode("utf-8", errors="replace"),
    }))
except Exception as error:
    print(json.dumps({
        "error": f"{type(error).__name__}: {error}",
    }))
"#;
        let mut child = Command::new(&python)
            .arg("-c")
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| EgressError::Http {
                status: None,
                message: format!("{primary_error}; python fallback spawn failed: {error}"),
            })?;
        {
            let mut stdin = child.stdin.take().ok_or_else(|| EgressError::Http {
                status: None,
                message: format!("{primary_error}; python fallback stdin unavailable"),
            })?;
            stdin
                .write_all(request.to_string().as_bytes())
                .map_err(|error| EgressError::Http {
                    status: None,
                    message: format!("{primary_error}; python fallback input failed: {error}"),
                })?;
        }
        let output = child
            .wait_with_output()
            .map_err(|error| EgressError::Http {
                status: None,
                message: format!("{primary_error}; python fallback wait failed: {error}"),
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(EgressError::Http {
                status: None,
                message: format!(
                    "{primary_error}; python fallback exited with {}: {}",
                    output.status,
                    truncate_for_error(&stderr)
                ),
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let fallback: PythonHttpResponse =
            serde_json::from_str(stdout.trim()).map_err(|error| EgressError::Http {
                status: None,
                message: format!("{primary_error}; python fallback returned invalid JSON: {error}"),
            })?;
        if let Some(error) = fallback.error {
            return Err(EgressError::Http {
                status: None,
                message: format!("{primary_error}; python fallback failed: {error}"),
            });
        }
        let body = fallback.body.unwrap_or_default();
        let actual = body.len().min(u64::MAX as usize) as u64;
        if let Some(limit) = self.policy.max_response_bytes {
            if actual > limit {
                return Err(EgressError::ResponseTooLarge { limit, actual });
            }
        }
        Ok(EgressResponse {
            status: fallback.status.unwrap_or(0),
            body,
            bytes: actual,
            host: host.to_string(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct PythonHttpResponse {
    status: Option<u16>,
    body: Option<String>,
    error: Option<String>,
}

fn request_headers(request: &EgressRequest) -> Vec<(String, String)> {
    let mut headers = request.headers.clone();
    if let Some(token) = &request.bearer_token {
        headers.push(("Authorization".to_string(), format!("Bearer {token}")));
    }
    headers
}

fn python_fallback_enabled() -> bool {
    !matches!(
        env::var("BUSTER_EGRESS_PYTHON_FALLBACK")
            .unwrap_or_else(|_| "1".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "0" | "false" | "off" | "no"
    )
}

fn python_fallback_timeout_secs() -> u64 {
    env::var("BUSTER_EGRESS_PYTHON_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(120)
}

fn python_program() -> Option<String> {
    ["python3", "python"].into_iter().find_map(|program| {
        Command::new(program)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok()
            .filter(|status| status.success())
            .map(|_| program.to_string())
    })
}

fn proxy_url_from_env() -> Option<String> {
    [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ]
    .into_iter()
    .filter_map(|key| env::var(key).ok())
    .map(|value| value.trim().to_string())
    .find(|value| !value.is_empty())
}

fn format_error_chain(error: &dyn std::error::Error) -> String {
    let mut parts = vec![error.to_string()];
    let mut source = error.source();
    while let Some(error) = source {
        parts.push(error.to_string());
        source = error.source();
    }
    parts.join(": ")
}

fn truncate_for_error(text: &str) -> String {
    let mut out = text.chars().take(500).collect::<String>();
    if text.chars().count() > 500 {
        out.push_str("...[truncated]");
    }
    out
}

#[derive(Debug, Clone)]
pub struct BrokeredHttpToolBackend {
    broker: EgressBroker,
    unit_id: String,
    json_body: serde_json::Value,
}

impl BrokeredHttpToolBackend {
    pub fn new(
        broker: EgressBroker,
        unit_id: impl Into<String>,
        json_body: serde_json::Value,
    ) -> Self {
        Self {
            broker,
            unit_id: unit_id.into(),
            json_body,
        }
    }
}

impl RuntimeBackend for BrokeredHttpToolBackend {
    fn runtime_kind(&self) -> RuntimeKind {
        RuntimeKind::Tool
    }

    fn execute(&self, request: &RuntimeRequest) -> Result<BackendCompletion, BackendFailure> {
        let response = self
            .broker
            .post_json(EgressRequest::post_json(
                &self.unit_id,
                request.input_summary.clone(),
                self.json_body.clone(),
            ))
            .map_err(|error| BackendFailure {
                reason: error.to_string(),
            })?;

        Ok(BackendCompletion {
            output_summary: format!(
                "HTTP {} from {} ({} bytes)",
                response.status, response.host, response.bytes
            ),
            actual_usage: ResourceUsage {
                network_bytes: response.bytes,
                ..request.estimated_usage.clone()
            },
        })
    }
}

fn host_matches(host: &str, allowed: &str) -> bool {
    if allowed == "*" {
        return true;
    }
    if let Some(suffix) = allowed.strip_prefix("*.") {
        return host == suffix || host.ends_with(&format!(".{suffix}"));
    }
    host == allowed
}

fn is_private_or_local_host(host: &str) -> bool {
    if matches!(host, "localhost" | "127.0.0.1" | "::1") {
        return true;
    }

    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(addr)) => is_private_or_local_ipv4(addr),
        Ok(IpAddr::V6(addr)) => is_private_or_local_ipv6(addr),
        Err(_) => false,
    }
}

fn is_private_or_local_ipv4(addr: Ipv4Addr) -> bool {
    addr.is_private()
        || addr.is_loopback()
        || addr.is_link_local()
        || addr.is_unspecified()
        || addr.octets()[0] == 100 && (64..=127).contains(&addr.octets()[1])
}

fn is_private_or_local_ipv6(addr: Ipv6Addr) -> bool {
    addr.is_loopback()
        || addr.is_unspecified()
        || addr.is_unique_local()
        || addr.is_unicast_link_local()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_allows_exact_and_wildcard_hosts() {
        let policy = NetworkPolicy {
            allowed_hosts: vec!["openrouter.ai".to_string(), "*.example.com".to_string()],
            deny_private_networks: true,
            max_response_bytes: Some(1024),
        };

        assert!(policy.allows_host("openrouter.ai"));
        assert!(policy.allows_host("api.example.com"));
        assert!(!policy.allows_host("evil.example"));
    }

    #[test]
    fn broker_blocks_unapproved_host_before_network() {
        let broker = EgressBroker::new(NetworkPolicy {
            allowed_hosts: vec!["openrouter.ai".to_string()],
            deny_private_networks: true,
            max_response_bytes: Some(1024),
        });

        let error = broker
            .preflight_url("https://evil.example/api")
            .unwrap_err();

        assert!(matches!(error, EgressError::HostNotAllowed { host } if host == "evil.example"));
    }

    #[test]
    fn broker_blocks_private_network_before_allowlist() {
        let broker = EgressBroker::new(NetworkPolicy {
            allowed_hosts: vec!["127.0.0.1".to_string()],
            deny_private_networks: true,
            max_response_bytes: Some(1024),
        });

        let error = broker
            .preflight_url("http://127.0.0.1:8080/private")
            .unwrap_err();

        assert!(matches!(error, EgressError::PrivateNetworkBlocked { .. }));
    }

    #[test]
    fn brokered_http_tool_blocks_disallowed_host_before_network() {
        let broker = EgressBroker::new(NetworkPolicy {
            allowed_hosts: vec!["openrouter.ai".to_string()],
            deny_private_networks: true,
            max_response_bytes: Some(1024),
        });
        let backend = BrokeredHttpToolBackend::new(
            broker,
            "http-tool-a",
            serde_json::json!({ "hello": "buster" }),
        );
        let request = RuntimeRequest::new(
            crate::host_api::BodyScope::new("buster", "main", "http-tool-test"),
            RuntimeKind::Tool,
            "http.post_json",
            "https://evil.example/api",
        )
        .with_network();

        let failure = backend.execute(&request).unwrap_err();

        assert!(failure.reason.contains("evil.example"));
        assert!(failure.reason.contains("not allowed"));
    }
}
