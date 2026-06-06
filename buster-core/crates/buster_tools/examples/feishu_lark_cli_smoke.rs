use std::env;
use std::path::PathBuf;

use buster_body::host_api::BodyScope;
use buster_body::runtime::RuntimeOutcome;
use buster_body::{BodyGate, CliToolBackend, CliToolCommand, SqliteBodyStore};
use buster_tools::{
    feishu_lark_cli_tool, ToolActionRequest, ToolExecutor, ToolInvocation, ToolRegistry,
};

fn main() {
    let script = env::var("BUSTER_LARK_CLI_SCRIPT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../package/scripts/run.js"));
    let script = script.canonicalize().unwrap_or(script);

    let mut registry = ToolRegistry::new();
    registry
        .register(feishu_lark_cli_tool())
        .expect("feishu lark cli manifest should register");
    let executor = ToolExecutor::new(registry);
    let invocation = ToolInvocation::new(
        "feishu.lark_cli",
        serde_json::json!({ "args": ["--help"] }),
        BodyScope::new("buster", "main", "feishu-lark-cli-smoke"),
        "smoke test the official Lark/Feishu CLI help command",
    )
    .with_action(ToolActionRequest::new("help"));
    let backend = CliToolBackend::new(
        "feishu-lark-cli",
        lark_cli_help_command(&script).with_max_output_bytes(16 * 1024),
    );
    let store = SqliteBodyStore::open_in_memory().expect("body gate store should open");
    let gate = BodyGate::new(store, buster_body::BodyMode::Normal);

    match executor.invoke_with_gate(&gate, invocation, backend, "lark-cli --help") {
        Ok(report) => match report.outcome {
            RuntimeOutcome::Completed(completion) => {
                println!("{}", completion.output_summary);
                println!(
                    "body gate audit events: {}",
                    gate.store()
                        .audit_event_count()
                        .expect("audit count should be readable")
                );
                println!(
                    "tool action audit records: {}",
                    gate.store()
                        .tool_action_count()
                        .expect("tool action count should be readable")
                );
            }
            RuntimeOutcome::Blocked(block) => {
                eprintln!("blocked: {:?}", block.reason);
                std::process::exit(2);
            }
            RuntimeOutcome::Failed(failure) => {
                eprintln!("failed: {}", failure.reason);
                std::process::exit(1);
            }
        },
        Err(error) => {
            eprintln!("executor error: {error}");
            std::process::exit(1);
        }
    }
}

fn lark_cli_help_command(script: &PathBuf) -> CliToolCommand {
    let script_text = script.to_string_lossy();
    if let Some(windows_path) = wsl_windows_path(&script_text) {
        return CliToolCommand::new("powershell.exe")
            .with_args([
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                format!("node '{}' --help", windows_path.replace('\'', "''")),
            ])
            .inherit_env();
    }

    CliToolCommand::new("node")
        .with_args([script_text.into_owned(), "--help".to_string()])
        .inherit_env()
}

fn wsl_windows_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/mnt/")?;
    let (drive, tail) = rest.split_once('/')?;
    if drive.len() != 1 {
        return None;
    }
    Some(format!(
        "{}:\\{}",
        drive.to_ascii_uppercase(),
        tail.replace('/', "\\")
    ))
}
