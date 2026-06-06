//! Minimal controlled script execution for body-runtime experiments.
//!
//! This is not a security boundary like WASM, Docker, or a micro-VM. It is a
//! narrow v0 harness that runs a command, captures output, applies a timeout,
//! and converts declared behavior into body sensors. Do not pass real secrets to
//! scripts launched through this module.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::registry::{ManagedUnit, UnitKind, UnitStatus};
use crate::resources::ResourceUsage;
use crate::sensors::{
    NetworkObservation, OutputObservation, SecretOutputSensor, SensorSignal, SensorSuite,
};
use crate::{NetworkEgressSensor, ResourceBudget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptSandboxConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub timeout: Duration,
    pub allowed_hosts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptSandboxRun {
    pub unit: ManagedUnit,
    pub stdout: String,
    pub stderr: String,
    pub network_observations: Vec<NetworkObservation>,
    pub output_observations: Vec<OutputObservation>,
    pub signals: Vec<SensorSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptSandboxError {
    pub reason: String,
}

pub struct ScriptSandbox {
    config: ScriptSandboxConfig,
}

impl ScriptSandbox {
    pub fn new(config: ScriptSandboxConfig) -> Self {
        Self { config }
    }

    pub fn run(&self, unit_id: impl Into<String>) -> Result<ScriptSandboxRun, ScriptSandboxError> {
        let unit_id = unit_id.into();
        let started = Instant::now();
        let mut command = Command::new(&self.config.command);
        command.args(&self.config.args);
        if let Some(cwd) = &self.config.cwd {
            command.current_dir(cwd);
        }
        command.env_clear();
        command.env("BUSTER_SANDBOX", "1");
        command.env("BUSTER_EGRESS_BROKER_REQUIRED", "1");

        let output = command.output().map_err(|error| ScriptSandboxError {
            reason: format!("failed to run script: {error}"),
        })?;
        let elapsed = started.elapsed();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let combined = format!("{stdout}\n{stderr}");

        let mut unit = ManagedUnit::new(&unit_id, UnitKind::Script, "buster").with_status(
            if output.status.success() {
                UnitStatus::Completed
            } else {
                UnitStatus::Failed
            },
        );
        unit.usage = ResourceUsage {
            wall_clock_ms: elapsed.as_millis() as u64,
            ..ResourceUsage::default()
        };

        if elapsed > self.config.timeout {
            unit.notes.push("anomaly timeout exceeded".to_string());
        }

        let network_observations = parse_network_intents(&unit_id, &combined);
        let output_observations = vec![OutputObservation {
            unit_id: unit_id.clone(),
            text: combined.clone(),
        }];
        let mut signals = SensorSuite::new()
            .with_sensor(NetworkEgressSensor::new(
                network_observations.clone(),
                self.config.allowed_hosts.clone(),
            ))
            .with_sensor(SecretOutputSensor::new(output_observations.clone()))
            .scan();

        if elapsed > self.config.timeout {
            signals.push(SensorSignal::critical(
                crate::SignalKind::ResourcePressure,
                unit_id.clone(),
                "script exceeded sandbox timeout",
            ));
        }

        for summary in parse_direct_network_attempts(&combined) {
            signals.push(SensorSignal::critical(
                crate::SignalKind::PolicyViolation,
                unit_id.clone(),
                summary,
            ));
        }

        if !signals.is_empty() {
            unit.status = UnitStatus::Running;
            unit.notes
                .push("suspicious script behavior observed".to_string());
        }

        Ok(ScriptSandboxRun {
            unit,
            stdout,
            stderr,
            network_observations,
            output_observations,
            signals,
        })
    }
}

fn parse_network_intents(unit_id: &str, text: &str) -> Vec<NetworkObservation> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("NETWORK ")?;
            let host = rest.split_whitespace().next()?;
            Some(NetworkObservation {
                unit_id: unit_id.to_string(),
                host: host.to_string(),
            })
        })
        .collect()
}

fn parse_direct_network_attempts(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("DIRECT_NETWORK ")?;
            Some(format!(
                "script declared a direct network attempt outside EgressBroker: {rest}"
            ))
        })
        .collect()
}

pub fn default_script_budget(timeout: Duration) -> ResourceBudget {
    ResourceBudget {
        max_wall_clock_ms: Some(timeout.as_millis() as u64),
        ..ResourceBudget::default()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::sensors::SignalKind;

    #[test]
    fn sandbox_converts_script_output_to_network_and_secret_signals() {
        let dir = unique_temp_dir("buster-script-sandbox");
        let script = dir.join("demo.sh");
        fs::write(
            &script,
            "#!/usr/bin/env bash\nprintf 'NETWORK evil.example\\n'\nprintf 'DIRECT_NETWORK curl https://evil.example\\n'\nprintf 'sk-demo-redacted\\n'\n",
        )
        .unwrap();

        let sandbox = ScriptSandbox::new(ScriptSandboxConfig {
            command: "bash".to_string(),
            args: vec![script.to_string_lossy().into_owned()],
            cwd: Some(dir.clone()),
            timeout: Duration::from_secs(3),
            allowed_hosts: vec!["openrouter.ai".to_string()],
        });

        let run = sandbox.run("script-a").unwrap();

        assert!(run
            .signals
            .iter()
            .any(|signal| signal.kind == SignalKind::NetworkAnomaly));
        assert!(run
            .signals
            .iter()
            .any(|signal| signal.kind == SignalKind::SecretExposureRisk));
        assert!(run
            .signals
            .iter()
            .any(|signal| signal.kind == SignalKind::PolicyViolation));

        let _ = fs::remove_dir_all(dir);
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
