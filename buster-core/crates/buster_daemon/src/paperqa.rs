//! Controlled PaperQA2 integration for scientific literature research.
//!
//! PaperQA is a Python literature-RAG system. Buster treats it as a layer-3
//! research tool, while BodyGate and Harness keep the side effects bounded.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use buster_body::runtime::{
    BackendCompletion, BackendFailure, RuntimeBackend, RuntimeKind, RuntimeOutcome, RuntimePolicy,
    RuntimeRequest,
};
use buster_body::{CapabilityLease, ChangeLevel, ResourceBudget, ResourceUsage, RiskLine};
use buster_value_model::ResearchDomain;
use serde::{Deserialize, Serialize};

use crate::research_framework::{self, ResearchQualityInput};
use crate::{append_daemon_jsonl, body_gate_bridge, harness, now_secs};

pub const PAPERQA_AUDIT: &str = "audit/paperqa.jsonl";
pub const PAPERQA_REPORT_DIR: &str = "research/paperqa/reports";
pub const PAPERQA_PAPER_DIR: &str = "research/paperqa/papers";
pub const PAPERQA_HOME_DIR: &str = "research/paperqa/.pqa";
const DEFAULT_TIMEOUT_MS: u64 = 240_000;
const MAX_QUESTION_CHARS: usize = 1_200;
const MAX_OUTPUT_CHARS: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperQaRequest {
    pub question: String,
    pub domain: ResearchDomain,
    pub task_id: Option<String>,
    pub settings: Option<String>,
    pub index_name: Option<String>,
    pub llm: Option<String>,
}

impl PaperQaRequest {
    pub fn new(question: impl Into<String>) -> Self {
        Self {
            question: question.into(),
            domain: ResearchDomain::Other,
            task_id: None,
            settings: Some("fast".to_string()),
            index_name: Some("buster-papers".to_string()),
            llm: env::var("BUSTER_PAPERQA_LLM")
                .ok()
                .filter(|value| !value.trim().is_empty()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperQaRecord {
    pub timestamp_secs: u64,
    pub contract_id: Option<String>,
    pub task_id: Option<String>,
    pub domain: ResearchDomain,
    pub question: String,
    pub status: String,
    pub command: Vec<String>,
    pub paper_dir: PathBuf,
    pub pqa_home: PathBuf,
    pub report_path: Option<PathBuf>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub environment_keys: Vec<String>,
    pub research_quality_score: Option<f32>,
    pub research_claim_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperQaDoctor {
    pub timestamp_secs: u64,
    pub available: bool,
    pub command: String,
    pub status: String,
    pub stdout_tail: String,
    pub stderr_tail: String,
}

pub fn doctor(root: &Path) -> std::io::Result<PaperQaDoctor> {
    ensure_layout(root)?;
    let bin = paperqa_bin(root);
    let resolved_bin = match resolve_program(&bin) {
        Ok(path) => path,
        Err(error) => {
            return Ok(PaperQaDoctor {
                timestamp_secs: now_secs(),
                available: false,
                command: format!("{bin} --help"),
                status: "unavailable".to_string(),
                stdout_tail: String::new(),
                stderr_tail: error,
            });
        }
    };
    let mut command = Command::new(&resolved_bin);
    command.arg("--help");
    apply_safe_env(root, &mut command);
    command.current_dir(paper_dir(root));
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let output = match run_with_timeout(command, Duration::from_secs(30), &[]) {
        Ok(output) => output,
        Err(error) => {
            return Ok(PaperQaDoctor {
                timestamp_secs: now_secs(),
                available: false,
                command: format!("{resolved_bin} --help"),
                status: "unavailable".to_string(),
                stdout_tail: String::new(),
                stderr_tail: error.to_string(),
            });
        }
    };
    let available = output
        .status
        .as_ref()
        .map(|status| status.success())
        .unwrap_or(false);
    Ok(PaperQaDoctor {
        timestamp_secs: now_secs(),
        available,
        command: format!("{bin} --help"),
        status: if available {
            "available"
        } else {
            "unavailable"
        }
        .to_string(),
        stdout_tail: truncate_chars(&output.stdout, 4_000),
        stderr_tail: truncate_chars(&output.stderr, 4_000),
    })
}

pub fn ask(root: &Path, request: PaperQaRequest) -> std::io::Result<PaperQaRecord> {
    ensure_layout(root)?;
    let timestamp_secs = now_secs();
    let question = truncate_chars(request.question.trim(), MAX_QUESTION_CHARS);
    let contract = harness::paperqa_contract(timestamp_secs, request.domain, &question);
    let contract_id = contract.contract_id.clone();
    harness::append_contract(root, &contract)?;

    let command_plan = paperqa_command_args(root, &request, &question);
    let env_keys = safe_env_keys();
    let secrets = known_secret_values(root);
    let backend = PaperQaBackend {
        root: root.to_path_buf(),
        command: command_plan.clone(),
        secrets,
        timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
    };
    let scope = body_gate_bridge::scope("research.paperqa");
    let runtime_request = RuntimeRequest::new(
        scope.clone(),
        RuntimeKind::Tool,
        "research.paperqa",
        format!("PaperQA literature question: {}", one_line(&question, 160)),
    )
    .with_risk(ChangeLevel::CapabilityExecution, RiskLine::Yellow)
    .with_network()
    .with_secret()
    .with_estimated_usage(ResourceUsage {
        tokens: 8_000,
        wall_clock_ms: DEFAULT_TIMEOUT_MS,
        network_bytes: 16 * 1024 * 1024,
        usd: 0.0,
    });
    let policy = RuntimePolicy::deny_by_default(ResourceBudget {
        max_tokens: Some(8_000),
        max_wall_clock_ms: Some(DEFAULT_TIMEOUT_MS),
        max_network_bytes: Some(16 * 1024 * 1024),
        max_usd: None,
    })
    .with_network()
    .with_secret()
    .with_lease(CapabilityLease {
        scope,
        capability_name: "research.paperqa".to_string(),
        expires_at: None,
        reason: "Buster requested bounded PaperQA literature research".to_string(),
    });

    let started = Instant::now();
    let outcome = body_gate_bridge::open_gate(root)
        .map_err(std::io::Error::other)?
        .invoke_runtime(runtime_request, &policy, backend)
        .map_err(std::io::Error::other)?;
    let duration_ms = started.elapsed().as_millis() as u64;

    let (status, exit_code, stdout_tail, stderr_tail, answer) = match outcome {
        RuntimeOutcome::Completed(completion) => {
            let parsed = serde_json::from_str::<PaperQaBackendOutput>(&completion.output_summary)
                .map_err(std::io::Error::other)?;
            (
                if parsed.exit_code == Some(0) {
                    "ok"
                } else {
                    "failed"
                }
                .to_string(),
                parsed.exit_code,
                parsed.stdout_tail,
                parsed.stderr_tail,
                parsed.stdout,
            )
        }
        RuntimeOutcome::Blocked(block) => (
            "blocked".to_string(),
            None,
            String::new(),
            format!("BodyGate blocked PaperQA: {:?}", block.reason),
            String::new(),
        ),
        RuntimeOutcome::Failed(failure) => (
            "failed".to_string(),
            None,
            String::new(),
            failure.reason,
            String::new(),
        ),
    };

    let report_path = if status == "ok" || !answer.trim().is_empty() || !stderr_tail.is_empty() {
        Some(write_report(
            root,
            timestamp_secs,
            request.domain,
            request.task_id.as_deref(),
            &question,
            &command_plan.redacted_args(),
            &status,
            &answer,
            &stderr_tail,
        )?)
    } else {
        None
    };

    let quality = research_framework::evaluate_and_record(
        root,
        ResearchQualityInput {
            contract_id: Some(&contract_id),
            task_id: request.task_id.as_deref(),
            domain: request.domain,
            question: &question,
            source_count: inferred_citation_count(&answer),
            content: Some(&answer),
            synthesis_succeeded: status == "ok",
            report_path: report_path.clone(),
        },
    )?;

    let record = PaperQaRecord {
        timestamp_secs,
        contract_id: Some(contract_id.clone()),
        task_id: request.task_id,
        domain: request.domain,
        question,
        status: status.clone(),
        command: command_plan.redacted_args(),
        paper_dir: paper_dir(root),
        pqa_home: pqa_home(root),
        report_path: report_path.clone(),
        exit_code,
        duration_ms,
        stdout_tail,
        stderr_tail,
        environment_keys: env_keys,
        research_quality_score: Some(quality.quality_score),
        research_claim_status: Some(quality.claim_status.as_str().to_string()),
    };
    append_daemon_jsonl(&root.join(PAPERQA_AUDIT), &record)?;

    let mut outcome = harness::HarnessOutcome::new(
        contract_id,
        if status == "ok" {
            harness::HarnessStatus::Completed
        } else {
            harness::HarnessStatus::Partial
        },
        format!("PaperQA ended with status {status}"),
    )
    .with_output("audit", PAPERQA_AUDIT, "PaperQA audit record");
    if let Some(path) = report_path {
        outcome = outcome.with_output(
            "paperqa_report",
            path.display().to_string(),
            "PaperQA literature answer",
        );
    }
    if status != "ok" {
        outcome = outcome.with_error(record.stderr_tail.clone());
    }
    harness::append_outcome(root, &outcome)?;
    Ok(record)
}

pub fn tail(root: &Path, max_lines: usize) -> std::io::Result<Vec<String>> {
    let text = fs::read_to_string(root.join(PAPERQA_AUDIT)).unwrap_or_default();
    let mut lines = text.lines().map(ToString::to_string).collect::<Vec<_>>();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    Ok(lines)
}

fn ensure_layout(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(paper_dir(root))?;
    fs::create_dir_all(pqa_home(root))?;
    fs::create_dir_all(root.join(PAPERQA_REPORT_DIR))?;
    fs::create_dir_all(root.join("audit"))?;
    Ok(())
}

#[derive(Debug, Clone)]
struct PaperQaCommand {
    bin: String,
    args: Vec<String>,
}

impl PaperQaCommand {
    fn redacted_args(&self) -> Vec<String> {
        std::iter::once(self.bin.clone())
            .chain(self.args.iter().cloned())
            .collect()
    }
}

fn paperqa_command_args(root: &Path, request: &PaperQaRequest, question: &str) -> PaperQaCommand {
    let mut args = Vec::new();
    if let Some(settings) = request
        .settings
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        args.push("--settings".to_string());
        args.push(settings.to_string());
    }
    if let Some(index_name) = request
        .index_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        args.push("-i".to_string());
        args.push(index_name.to_string());
    }
    args.extend([
        "--agent.index.paper_directory".to_string(),
        ".".to_string(),
        "--agent.index.recurse_subdirectories".to_string(),
        "true".to_string(),
        "--agent.index.sync_with_paper_directory".to_string(),
        "true".to_string(),
        "--agent.rebuild_index".to_string(),
        config_value(root, "BUSTER_PAPERQA_REBUILD_INDEX").unwrap_or_else(|| "true".to_string()),
    ]);
    if let Some(llm) = effective_llm(root, request) {
        args.push("--llm".to_string());
        args.push(llm);
    }
    args.push("ask".to_string());
    args.push(question.to_string());
    PaperQaCommand {
        bin: paperqa_bin(root),
        args,
    }
}

#[derive(Debug, Clone)]
struct PaperQaBackend {
    root: PathBuf,
    command: PaperQaCommand,
    secrets: Vec<String>,
    timeout: Duration,
}

impl RuntimeBackend for PaperQaBackend {
    fn runtime_kind(&self) -> RuntimeKind {
        RuntimeKind::Tool
    }

    fn execute(&self, _request: &RuntimeRequest) -> Result<BackendCompletion, BackendFailure> {
        let resolved_bin =
            resolve_program(&self.command.bin).map_err(|reason| BackendFailure { reason })?;

        let mut index_command = Command::new(&resolved_bin);
        index_command.args(index_args_for_ask(&self.command.args));
        apply_safe_env(&self.root, &mut index_command);
        index_command.current_dir(paper_dir(&self.root));
        index_command.stdout(Stdio::piped());
        index_command.stderr(Stdio::piped());
        let index_output =
            run_with_timeout(index_command, self.timeout, &self.secrets).map_err(|error| {
                BackendFailure {
                    reason: error.to_string(),
                }
            })?;
        if !index_output
            .status
            .as_ref()
            .map(|status| status.success())
            .unwrap_or(false)
        {
            return Ok(BackendCompletion {
                output_summary: serde_json::to_string(&PaperQaBackendOutput {
                    exit_code: index_output
                        .status
                        .as_ref()
                        .and_then(|status| status.code()),
                    stdout_tail: truncate_chars(&index_output.stdout, 6_000),
                    stderr_tail: truncate_chars(&index_output.stderr, 6_000),
                    stdout: truncate_chars(&index_output.stdout, MAX_OUTPUT_CHARS),
                })
                .map_err(|error| BackendFailure {
                    reason: error.to_string(),
                })?,
                actual_usage: ResourceUsage {
                    tokens: 0,
                    wall_clock_ms: index_output.duration_ms,
                    network_bytes: 0,
                    usd: 0.0,
                },
            });
        }

        let mut command = Command::new(resolved_bin);
        command.args(&self.command.args);
        apply_safe_env(&self.root, &mut command);
        command.current_dir(paper_dir(&self.root));
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let mut output =
            run_with_timeout(command, self.timeout, &self.secrets).map_err(|error| {
                BackendFailure {
                    reason: error.to_string(),
                }
            })?;
        output.stdout = format!(
            "Index phase:\n{}\n\nAsk phase:\n{}",
            index_output.stdout, output.stdout
        );
        output.stderr = format!(
            "Index phase:\n{}\n\nAsk phase:\n{}",
            index_output.stderr, output.stderr
        );
        let summary = serde_json::to_string(&PaperQaBackendOutput {
            exit_code: output.status.as_ref().and_then(|status| status.code()),
            stdout_tail: truncate_chars(&output.stdout, 6_000),
            stderr_tail: truncate_chars(&output.stderr, 6_000),
            stdout: truncate_chars(&output.stdout, MAX_OUTPUT_CHARS),
        })
        .map_err(|error| BackendFailure {
            reason: error.to_string(),
        })?;
        Ok(BackendCompletion {
            output_summary: summary,
            actual_usage: ResourceUsage {
                tokens: 0,
                wall_clock_ms: output.duration_ms,
                network_bytes: 0,
                usd: 0.0,
            },
        })
    }
}

fn index_args_for_ask(args: &[String]) -> Vec<String> {
    let mut index_args = args
        .iter()
        .take_while(|arg| arg.as_str() != "ask")
        .cloned()
        .collect::<Vec<_>>();
    index_args.push("index".to_string());
    index_args.push(".".to_string());
    index_args
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PaperQaBackendOutput {
    exit_code: Option<i32>,
    stdout_tail: String,
    stderr_tail: String,
    stdout: String,
}

#[derive(Debug, Clone)]
struct CommandOutput {
    status: Option<ExitStatus>,
    stdout: String,
    stderr: String,
    duration_ms: u64,
}

fn run_with_timeout(
    mut command: Command,
    timeout: Duration,
    secrets: &[String],
) -> std::io::Result<CommandOutput> {
    let started = Instant::now();
    let mut child = command.spawn()?;
    let mut timed_out = false;
    loop {
        if let Some(_status) = child.try_wait()? {
            break;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
    let output = child.wait_with_output()?;
    let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if timed_out {
        stderr.push_str("\nPaperQA process timed out and was terminated by Buster.");
    }
    Ok(CommandOutput {
        status: Some(output.status),
        stdout: redact_known_secrets(&String::from_utf8_lossy(&output.stdout), secrets),
        stderr: redact_known_secrets(&stderr, secrets),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn apply_safe_env(root: &Path, command: &mut Command) {
    command.env_clear();
    for key in safe_env_keys() {
        if let Some(value) = config_value(root, &key) {
            if !value.is_empty() {
                command.env(key, value);
            }
        }
    }
    apply_litellm_bridge(root, command);
    command.env("PQA_HOME", pqa_home(root));
}

fn apply_litellm_bridge(root: &Path, command: &mut Command) {
    let backend = config_value(root, "BUSTER_LLM_BACKEND")
        .or_else(|| config_value(root, "LLM_BACKEND"))
        .unwrap_or_else(|| {
            if config_value(root, "MIMO_API_KEY")
                .or_else(|| config_value(root, "XIAOMI_API_KEY"))
                .is_some()
            {
                "mimo".to_string()
            } else {
                "openrouter".to_string()
            }
        });
    match backend.trim().to_ascii_lowercase().as_str() {
        "mimo" | "xiaomi" | "xiaomi-mimo" => {
            if config_value(root, "OPENAI_API_KEY").is_none() {
                if let Some(key) = config_value(root, "MIMO_API_KEY")
                    .or_else(|| config_value(root, "XIAOMI_API_KEY"))
                {
                    command.env("OPENAI_API_KEY", key);
                }
            }
            if config_value(root, "OPENAI_BASE_URL")
                .or_else(|| config_value(root, "OPENAI_API_BASE"))
                .is_none()
            {
                if let Some(endpoint) = config_value(root, "MIMO_ENDPOINT") {
                    command.env("OPENAI_BASE_URL", openai_base_from_endpoint(&endpoint));
                }
            }
        }
        "openrouter" => {
            if config_value(root, "OPENAI_API_KEY").is_none() {
                if let Some(key) = config_value(root, "OPENROUTER_API_KEY") {
                    command.env("OPENAI_API_KEY", key);
                }
            }
            if config_value(root, "OPENAI_BASE_URL")
                .or_else(|| config_value(root, "OPENAI_API_BASE"))
                .is_none()
            {
                command.env("OPENAI_BASE_URL", "https://openrouter.ai/api/v1");
            }
        }
        _ => {}
    }
}

fn effective_llm(root: &Path, request: &PaperQaRequest) -> Option<String> {
    if let Some(llm) = request
        .llm
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(llm.to_string());
    }
    let backend = config_value(root, "BUSTER_LLM_BACKEND")
        .or_else(|| config_value(root, "LLM_BACKEND"))
        .unwrap_or_else(|| {
            if config_value(root, "MIMO_API_KEY")
                .or_else(|| config_value(root, "XIAOMI_API_KEY"))
                .is_some()
            {
                "mimo".to_string()
            } else {
                "openrouter".to_string()
            }
        });
    match backend.trim().to_ascii_lowercase().as_str() {
        "mimo" | "xiaomi" | "xiaomi-mimo" => config_value(root, "MIMO_MODEL")
            .or_else(|| config_value(root, "XIAOMI_MODEL"))
            .map(|model| format!("openai/{model}")),
        "openrouter" => config_value(root, "OPENROUTER_MODEL")
            .or_else(|| config_value(root, "BUSTER_OPENROUTER_MODEL"))
            .map(|model| format!("openrouter/{model}")),
        _ => None,
    }
}

fn safe_env_keys() -> Vec<String> {
    [
        "PATH",
        "Path",
        "HOME",
        "USERPROFILE",
        "SYSTEMROOT",
        "WINDIR",
        "TMP",
        "TEMP",
        "TMPDIR",
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "https_proxy",
        "http_proxy",
        "all_proxy",
        "no_proxy",
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "OPENAI_API_BASE",
        "OPENROUTER_API_KEY",
        "LITELLM_API_KEY",
        "XIAOMI_API_KEY",
        "MIMO_API_KEY",
        "CROSSREF_API_KEY",
        "SEMANTIC_SCHOLAR_API_KEY",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn known_secret_values(root: &Path) -> Vec<String> {
    safe_env_keys()
        .into_iter()
        .filter(|key| key.contains("KEY") || key.contains("SECRET") || key.contains("TOKEN"))
        .filter_map(|key| config_value(root, &key))
        .filter(|value| value.len() >= 8)
        .collect()
}

fn config_value(root: &Path, key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| env_file_value(root.join(".env"), key))
        .or_else(|| {
            if env::var("BUSTER_DISABLE_HOME_ENV").ok().as_deref() == Some("1") {
                None
            } else {
                env_file_value("/home/buster/.ironclaw/.env", key)
            }
        })
}

fn env_file_value(path: impl AsRef<Path>, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((left, right)) = trimmed.split_once('=') else {
            continue;
        };
        if left.trim() == key {
            return Some(
                right
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            )
            .filter(|value| !value.trim().is_empty());
        }
    }
    None
}

fn openai_base_from_endpoint(endpoint: &str) -> String {
    endpoint
        .trim()
        .strip_suffix("/chat/completions")
        .unwrap_or_else(|| endpoint.trim().trim_end_matches('/'))
        .to_string()
}

fn write_report(
    root: &Path,
    timestamp_secs: u64,
    domain: ResearchDomain,
    task_id: Option<&str>,
    question: &str,
    command: &[String],
    status: &str,
    answer: &str,
    stderr_tail: &str,
) -> std::io::Result<PathBuf> {
    let slug = task_id
        .map(safe_slug)
        .unwrap_or_else(|| safe_slug(&format!("{domain:?}")));
    let path = root
        .join(PAPERQA_REPORT_DIR)
        .join(format!("{timestamp_secs}-{slug}.md"));
    let markdown = format!(
        "# PaperQA Literature Report\n\n- timestamp_secs: {timestamp_secs}\n- domain: {:?}\n- task_id: {}\n- status: {status}\n- command: `{}`\n\n## Question\n\n{}\n\n## Answer\n\n{}\n\n## Tool Stderr Tail\n\n```text\n{}\n```\n",
        domain,
        task_id.unwrap_or("ad-hoc"),
        command.join(" "),
        question,
        answer.trim(),
        stderr_tail.trim()
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    body_gate_bridge::record_memory_write(
        root,
        "paperqa_report",
        "paperqa.write_report",
        &markdown,
    )
    .map_err(std::io::Error::other)?;
    fs::write(&path, markdown)?;
    Ok(path)
}

fn paper_dir(root: &Path) -> PathBuf {
    root.join(PAPERQA_PAPER_DIR)
}

fn pqa_home(root: &Path) -> PathBuf {
    root.join(PAPERQA_HOME_DIR)
}

fn paperqa_bin(root: &Path) -> String {
    env::var("BUSTER_PAPERQA_BIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            let workspace_bin = root
                .join("tools")
                .join("paperqa")
                .join(".venv")
                .join("bin")
                .join("pqa");
            if workspace_bin.is_file() {
                workspace_bin.to_string_lossy().into_owned()
            } else {
                let windows_bin = root
                    .join("tools")
                    .join("paperqa")
                    .join(".venv")
                    .join("Scripts")
                    .join("pqa.exe");
                if windows_bin.is_file() {
                    windows_bin.to_string_lossy().into_owned()
                } else {
                    "pqa".to_string()
                }
            }
        })
}

fn resolve_program(bin: &str) -> Result<String, String> {
    if bin.contains('/') || bin.contains('\\') {
        let path = PathBuf::from(bin);
        if path.is_file() {
            return Ok(path
                .canonicalize()
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned());
        }
        return Err(format!("PaperQA binary `{bin}` is not an executable file"));
    }

    if let Some(found) = env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .flat_map(|dir| executable_candidates(&dir, bin))
        .find(|candidate| candidate.is_file())
    {
        return Ok(found
            .canonicalize()
            .unwrap_or(found)
            .to_string_lossy()
            .into_owned());
    }

    Err(format!(
        "PaperQA binary `{bin}` was not found in PATH; install paper-qa or set BUSTER_PAPERQA_BIN"
    ))
}

fn executable_candidates(dir: &Path, bin: &str) -> Vec<PathBuf> {
    let mut candidates = vec![dir.join(bin)];
    if cfg!(windows) && !bin.to_ascii_lowercase().ends_with(".exe") {
        candidates.push(dir.join(format!("{bin}.exe")));
    }
    candidates
}

fn inferred_citation_count(answer: &str) -> usize {
    let lower = answer.to_ascii_lowercase();
    let markers = [" doi", "arxiv", " pages ", " et al", "citation", "source"];
    markers
        .iter()
        .filter(|marker| lower.contains(**marker))
        .count()
}

fn redact_known_secrets(text: &str, secrets: &[String]) -> String {
    let mut redacted = text.to_string();
    for secret in secrets {
        if secret.len() >= 8 {
            redacted = redacted.replace(secret, "[REDACTED_SECRET]");
        }
    }
    redacted
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("\n...[truncated]");
    }
    out
}

fn one_line(text: &str, max_chars: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&compact, max_chars)
}

fn safe_slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "paperqa".to_string()
    } else {
        truncate_chars(slug, 64).replace('\n', "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_uses_bounded_defaults() {
        let request = PaperQaRequest::new("What is PaperQA2?");
        let command = paperqa_command_args(
            Path::new("/tmp/buster-paperqa-test"),
            &request,
            "What is PaperQA2?",
        );
        assert_eq!(command.bin, "pqa");
        assert!(command.args.contains(&"--settings".to_string()));
        assert!(command.args.contains(&"fast".to_string()));
        assert!(command.args.contains(&"ask".to_string()));
    }

    #[test]
    fn redacts_known_secret_values() {
        let text = "token abcdefgh123456 should not appear";
        let redacted = redact_known_secrets(text, &["abcdefgh123456".to_string()]);
        assert!(!redacted.contains("abcdefgh123456"));
        assert!(redacted.contains("[REDACTED_SECRET]"));
    }

    #[test]
    fn safe_slug_is_filesystem_friendly() {
        assert_eq!(safe_slug("Quantum Fusion: 2026?"), "quantum-fusion--2026");
    }
}
