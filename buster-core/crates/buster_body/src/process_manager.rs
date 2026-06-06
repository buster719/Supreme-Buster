//! Real process management for Buster-owned units.
//!
//! This manager is intentionally narrow: it can only observe and terminate
//! child processes that it spawned itself. It is not a host antivirus and it
//! does not kill arbitrary system PIDs.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use crate::registry::{ManagedUnit, UnitKind, UnitStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSpawnRequest {
    pub unit_id: String,
    pub kind: UnitKind,
    pub owner: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub inherit_env: bool,
}

impl ProcessSpawnRequest {
    pub fn new(
        unit_id: impl Into<String>,
        kind: UnitKind,
        owner: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self {
            unit_id: unit_id.into(),
            kind,
            owner: owner.into(),
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            inherit_env: false,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSnapshot {
    pub unit_id: String,
    pub pid: u32,
    pub status: UnitStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessManagerError {
    DuplicateUnit { unit_id: String },
    UnknownUnit { unit_id: String },
    SpawnFailed { unit_id: String, reason: String },
    StatusFailed { unit_id: String, reason: String },
    TerminateFailed { unit_id: String, reason: String },
}

impl fmt::Display for ProcessManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateUnit { unit_id } => write!(formatter, "duplicate unit `{unit_id}`"),
            Self::UnknownUnit { unit_id } => write!(formatter, "unknown unit `{unit_id}`"),
            Self::SpawnFailed { unit_id, reason } => {
                write!(formatter, "failed to spawn `{unit_id}`: {reason}")
            }
            Self::StatusFailed { unit_id, reason } => {
                write!(formatter, "failed to inspect `{unit_id}`: {reason}")
            }
            Self::TerminateFailed { unit_id, reason } => {
                write!(formatter, "failed to terminate `{unit_id}`: {reason}")
            }
        }
    }
}

impl std::error::Error for ProcessManagerError {}

#[derive(Debug, Default)]
pub struct ProcessManager {
    children: HashMap<String, Child>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(
        &mut self,
        request: ProcessSpawnRequest,
    ) -> Result<ManagedUnit, ProcessManagerError> {
        if self.children.contains_key(&request.unit_id) {
            return Err(ProcessManagerError::DuplicateUnit {
                unit_id: request.unit_id,
            });
        }

        let mut command = Command::new(&request.command);
        command.args(&request.args);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        if !request.inherit_env {
            command.env_clear();
        }

        for (key, value) in &request.env {
            command.env(key, value);
        }

        if let Some(cwd) = &request.cwd {
            command.current_dir(cwd);
        }

        let child = command
            .spawn()
            .map_err(|error| ProcessManagerError::SpawnFailed {
                unit_id: request.unit_id.clone(),
                reason: error.to_string(),
            })?;
        let pid = child.id();

        let mut unit = ManagedUnit::new(&request.unit_id, request.kind, request.owner)
            .with_status(UnitStatus::Running);
        unit.notes.push(format!(
            "managed process pid {pid}; command `{}`",
            request.command
        ));

        self.children.insert(request.unit_id, child);
        Ok(unit)
    }

    pub fn status(&mut self, unit_id: &str) -> Result<ProcessSnapshot, ProcessManagerError> {
        let child =
            self.children
                .get_mut(unit_id)
                .ok_or_else(|| ProcessManagerError::UnknownUnit {
                    unit_id: unit_id.to_string(),
                })?;
        let pid = child.id();
        let status = match child
            .try_wait()
            .map_err(|error| ProcessManagerError::StatusFailed {
                unit_id: unit_id.to_string(),
                reason: error.to_string(),
            })? {
            Some(exit) if exit.success() => UnitStatus::Completed,
            Some(_) => UnitStatus::Failed,
            None => UnitStatus::Running,
        };

        Ok(ProcessSnapshot {
            unit_id: unit_id.to_string(),
            pid,
            status,
        })
    }

    pub fn terminate_owned(
        &mut self,
        unit_id: &str,
    ) -> Result<ProcessSnapshot, ProcessManagerError> {
        let mut child =
            self.children
                .remove(unit_id)
                .ok_or_else(|| ProcessManagerError::UnknownUnit {
                    unit_id: unit_id.to_string(),
                })?;
        let pid = child.id();

        match child.try_wait() {
            Ok(Some(exit)) => {
                let status = if exit.success() {
                    UnitStatus::Completed
                } else {
                    UnitStatus::Failed
                };
                Ok(ProcessSnapshot {
                    unit_id: unit_id.to_string(),
                    pid,
                    status,
                })
            }
            Ok(None) => {
                child
                    .kill()
                    .map_err(|error| ProcessManagerError::TerminateFailed {
                        unit_id: unit_id.to_string(),
                        reason: error.to_string(),
                    })?;
                child
                    .wait()
                    .map_err(|error| ProcessManagerError::TerminateFailed {
                        unit_id: unit_id.to_string(),
                        reason: error.to_string(),
                    })?;
                Ok(ProcessSnapshot {
                    unit_id: unit_id.to_string(),
                    pid,
                    status: UnitStatus::Quarantined,
                })
            }
            Err(error) => Err(ProcessManagerError::StatusFailed {
                unit_id: unit_id.to_string(),
                reason: error.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_manager_terminates_only_owned_child() {
        let mut manager = ProcessManager::new();
        let request = sleep_request("owned-sleeper");

        let unit = manager.spawn(request).unwrap();
        assert_eq!(unit.status, UnitStatus::Running);

        let running = manager.status("owned-sleeper").unwrap();
        assert_eq!(running.status, UnitStatus::Running);

        let stopped = manager.terminate_owned("owned-sleeper").unwrap();
        assert_eq!(stopped.status, UnitStatus::Quarantined);
    }

    #[test]
    fn process_manager_rejects_unknown_unit_termination() {
        let mut manager = ProcessManager::new();
        let error = manager.terminate_owned("not-owned").unwrap_err();

        assert!(matches!(
            error,
            ProcessManagerError::UnknownUnit { unit_id } if unit_id == "not-owned"
        ));
    }

    fn sleep_request(unit_id: &str) -> ProcessSpawnRequest {
        #[cfg(windows)]
        {
            ProcessSpawnRequest::new(unit_id, UnitKind::Script, "buster", "cmd")
                .with_args(["/C", "ping", "-n", "30", "127.0.0.1", ">NUL"])
                .inherit_env()
        }

        #[cfg(not(windows))]
        {
            ProcessSpawnRequest::new(unit_id, UnitKind::Script, "buster", "/bin/sh")
                .with_args(["-c", "sleep 30"])
        }
    }
}
