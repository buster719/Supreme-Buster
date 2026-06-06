//! CLI tool backend for layer-3 tools.
//!
//! This backend executes a concrete command without shell interpolation and
//! returns bounded stdout/stderr to the body runtime. It is a bridge for
//! external CLIs such as `lark-cli`; it is not a sandbox by itself.

use std::fmt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::runtime::{
    BackendCompletion, BackendFailure, RuntimeBackend, RuntimeKind, RuntimeRequest,
};
use crate::secrets::SecretRedactor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliToolCommand {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub inherit_env: bool,
    pub max_output_bytes: usize,
    pub allowed_arg_prefixes: Vec<String>,
    pub blocked_args: Vec<String>,
    pub redactor: Option<SecretRedactor>,
}

impl CliToolCommand {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            inherit_env: false,
            max_output_bytes: 32 * 1024,
            allowed_arg_prefixes: Vec::new(),
            blocked_args: Vec::new(),
            redactor: None,
        }
    }

    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn inherit_env(mut self) -> Self {
        self.inherit_env = true;
        self
    }

    pub fn with_max_output_bytes(mut self, max_output_bytes: usize) -> Self {
        self.max_output_bytes = max_output_bytes;
        self
    }

    pub fn with_allowed_arg_prefixes(
        mut self,
        prefixes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.allowed_arg_prefixes = prefixes.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_blocked_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.blocked_args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_redactor(mut self, redactor: SecretRedactor) -> Self {
        self.redactor = Some(redactor);
        self
    }

    pub fn validate_args(&self) -> Result<(), CliToolError> {
        for arg in &self.args {
            if self.blocked_args.iter().any(|blocked| blocked == arg) {
                return Err(CliToolError::ArgumentBlocked { arg: arg.clone() });
            }
        }

        if self.allowed_arg_prefixes.is_empty() {
            return Ok(());
        }

        let joined = self.args.join(" ");
        if self
            .allowed_arg_prefixes
            .iter()
            .any(|prefix| joined.starts_with(prefix))
        {
            return Ok(());
        }

        Err(CliToolError::ArgumentNotAllowed {
            args: self.args.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliToolError {
    SpawnFailed(String),
    OutputDecodeFailed(String),
    ArgumentBlocked { arg: String },
    ArgumentNotAllowed { args: Vec<String> },
}

impl fmt::Display for CliToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpawnFailed(reason) => write!(formatter, "failed to run CLI tool: {reason}"),
            Self::OutputDecodeFailed(reason) => {
                write!(formatter, "failed to decode CLI output: {reason}")
            }
            Self::ArgumentBlocked { arg } => write!(formatter, "CLI argument `{arg}` is blocked"),
            Self::ArgumentNotAllowed { args } => {
                write!(formatter, "CLI arguments are not allowed: {args:?}")
            }
        }
    }
}

impl std::error::Error for CliToolError {}

#[derive(Debug, Clone)]
pub struct CliToolBackend {
    unit_id: String,
    command: CliToolCommand,
}

impl CliToolBackend {
    pub fn new(unit_id: impl Into<String>, command: CliToolCommand) -> Self {
        Self {
            unit_id: unit_id.into(),
            command,
        }
    }

    fn run_command(&self) -> Result<CliToolRun, CliToolError> {
        self.command.validate_args()?;

        let started = Instant::now();
        let mut command = Command::new(&self.command.command);
        command.args(&self.command.args);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        if !self.command.inherit_env {
            command.env_clear();
        }
        command.env("BUSTER_TOOL_UNIT_ID", &self.unit_id);
        command.env("BUSTER_EGRESS_BROKER_REQUIRED", "1");
        for (key, value) in &self.command.env {
            command.env(key, value);
        }
        if let Some(cwd) = &self.command.cwd {
            command.current_dir(cwd);
        }

        let output = command
            .output()
            .map_err(|error| CliToolError::SpawnFailed(error.to_string()))?;
        let mut stdout = bounded_utf8(&output.stdout, self.command.max_output_bytes)?;
        let mut stderr = bounded_utf8(&output.stderr, self.command.max_output_bytes)?;
        if let Some(redactor) = &self.command.redactor {
            stdout = redactor.redact(&stdout);
            stderr = redactor.redact(&stderr);
        }

        Ok(CliToolRun {
            success: output.status.success(),
            code: output.status.code(),
            stdout,
            stderr,
            wall_clock_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        })
    }
}

impl RuntimeBackend for CliToolBackend {
    fn runtime_kind(&self) -> RuntimeKind {
        RuntimeKind::Tool
    }

    fn execute(&self, request: &RuntimeRequest) -> Result<BackendCompletion, BackendFailure> {
        let run = self.run_command().map_err(|error| BackendFailure {
            reason: error.to_string(),
        })?;
        let mut usage = request.estimated_usage.clone();
        usage.wall_clock_ms = run.wall_clock_ms;

        let output_summary = run.render_summary();
        if run.success {
            Ok(BackendCompletion {
                output_summary,
                actual_usage: usage,
            })
        } else {
            Err(BackendFailure {
                reason: output_summary,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliToolRun {
    success: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
    wall_clock_ms: u64,
}

impl CliToolRun {
    fn render_summary(&self) -> String {
        format!(
            "exit={:?}\nstdout:\n{}\nstderr:\n{}",
            self.code,
            self.stdout.trim(),
            self.stderr.trim()
        )
    }
}

fn bounded_utf8(bytes: &[u8], limit: usize) -> Result<String, CliToolError> {
    let mut slice = bytes;
    let truncated = bytes.len() > limit;
    if truncated {
        slice = &bytes[..limit];
    }
    let mut text = String::from_utf8(slice.to_vec())
        .map_err(|error| CliToolError::OutputDecodeFailed(error.to_string()))?;
    if truncated {
        text.push_str("\n[TRUNCATED]");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_api::BodyScope;

    #[test]
    fn cli_backend_runs_command_without_shell_interpolation() {
        let backend = CliToolBackend::new("cli-smoke", smoke_command());
        let request = RuntimeRequest::new(
            BodyScope::new("buster", "main", "cli-test"),
            RuntimeKind::Tool,
            "cli.smoke",
            "print buster-cli",
        );

        let completion = backend.execute(&request).unwrap();

        assert!(completion.output_summary.contains("buster-cli"));
    }

    #[test]
    fn cli_command_blocks_disallowed_args_before_spawn() {
        let command = smoke_command().with_allowed_arg_prefixes(["safe"]);
        let error = command.validate_args().unwrap_err();

        assert!(matches!(error, CliToolError::ArgumentNotAllowed { .. }));
    }

    #[test]
    fn cli_output_can_be_redacted() {
        let redactor = SecretRedactor::new(
            vec![crate::secrets::SecretFingerprint::from_secret(
                "sk-test-secret",
                b"test-key",
            )],
            b"test-key".to_vec(),
        );
        let backend =
            CliToolBackend::new("cli-redact", secret_echo_command().with_redactor(redactor));
        let request = RuntimeRequest::new(
            BodyScope::new("buster", "main", "cli-redact-test"),
            RuntimeKind::Tool,
            "cli.redact",
            "print redacted secret",
        );

        let completion = backend.execute(&request).unwrap();

        assert!(!completion.output_summary.contains("sk-test-secret"));
        assert!(completion.output_summary.contains("[SECRET_REDACTED]"));
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

    fn secret_echo_command() -> CliToolCommand {
        #[cfg(windows)]
        {
            CliToolCommand::new("cmd")
                .with_args(["/C", "echo", "sk-test-secret"])
                .inherit_env()
        }

        #[cfg(not(windows))]
        {
            CliToolCommand::new("/bin/sh")
                .with_args(["-c", "printf sk-test-secret"])
                .inherit_env()
        }
    }
}
