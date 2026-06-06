//! Network boundary, egress policy, and HTTP broker.
//!
//! The broker is the body-layer choke point for outbound HTTP. Callers should
//! ask it to perform network requests instead of creating raw clients.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::resources::ResourceUsage;
use crate::runtime::{
    BackendCompletion, BackendFailure, RuntimeBackend, RuntimeKind, RuntimeRequest,
};

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
        Self {
            policy,
            client: reqwest::blocking::Client::new(),
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
        let mut builder = self.client.post(&request.url).json(&request.json_body);

        if let Some(token) = request.bearer_token {
            builder = builder.bearer_auth(token);
        }
        for (name, value) in request.headers {
            builder = builder.header(name, value);
        }

        let response = builder.send().map_err(|error| EgressError::Http {
            status: error.status().map(|status| status.as_u16()),
            message: error.to_string(),
        })?;
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
            message: error.to_string(),
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
