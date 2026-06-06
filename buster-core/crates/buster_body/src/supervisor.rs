//! Body supervisor loop.
//!
//! The supervisor turns sensors and the controller into a bounded reconcile
//! loop. It is intentionally small and synchronous for v0: callers choose how
//! many ticks to run, and no host-wide process action is executed here.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::controller::{BodyController, BodyRuntimeWriter, ReconcileReport};
use crate::sensors::SensorSuite;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorConfig {
    pub tick_interval: Duration,
    pub max_ticks: Option<usize>,
    pub write_reports: bool,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_secs(30),
            max_ticks: Some(1),
            write_reports: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SupervisorTick {
    pub index: usize,
    pub report: ReconcileReport,
    pub written_paths: Vec<PathBuf>,
}

pub struct BodySupervisor {
    controller: BodyController,
    sensors: SensorSuite,
    writer: Option<BodyRuntimeWriter>,
    config: SupervisorConfig,
}

impl BodySupervisor {
    pub fn new(
        controller: BodyController,
        sensors: SensorSuite,
        writer: Option<BodyRuntimeWriter>,
        config: SupervisorConfig,
    ) -> Self {
        Self {
            controller,
            sensors,
            writer,
            config,
        }
    }

    pub fn tick(&mut self, index: usize) -> std::io::Result<SupervisorTick> {
        let signals = self.sensors.scan();
        let report = self.controller.reconcile(signals);
        let written_paths = if self.config.write_reports {
            match &self.writer {
                Some(writer) => writer.write_report(&report)?,
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Ok(SupervisorTick {
            index,
            report,
            written_paths,
        })
    }

    pub fn run(&mut self) -> std::io::Result<Vec<SupervisorTick>> {
        let max_ticks = self.config.max_ticks.unwrap_or(usize::MAX);
        let mut ticks = Vec::new();

        for index in 0..max_ticks {
            let tick = self.tick(index)?;
            ticks.push(tick);

            if index + 1 < max_ticks {
                thread::sleep(self.config.tick_interval);
            }
        }

        Ok(ticks)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::controller::{BodyRuntimePaths, BodyRuntimeWriter};
    use crate::sensors::{
        NetworkEgressSensor, NetworkObservation, OutputObservation, SecretOutputSensor,
    };
    use crate::state::{BodyMode, DesiredBodyState};

    #[test]
    fn supervisor_tick_scans_sensors_and_reconciles() {
        let controller = BodyController::new(DesiredBodyState::normal());
        let sensors = SensorSuite::new().with_sensor(NetworkEgressSensor::new(
            vec![NetworkObservation {
                unit_id: "unit-a".to_string(),
                host: "blocked.example".to_string(),
            }],
            vec!["openrouter.ai".to_string()],
        ));
        let config = SupervisorConfig {
            tick_interval: Duration::from_millis(0),
            max_ticks: Some(1),
            write_reports: false,
        };
        let mut supervisor = BodySupervisor::new(controller, sensors, None, config);

        let tick = supervisor.tick(0).unwrap();

        assert_eq!(tick.index, 0);
        assert_eq!(tick.report.next_mode, BodyMode::Restricted);
        assert_eq!(tick.report.signals.len(), 1);
        assert!(tick.written_paths.is_empty());
    }

    #[test]
    fn supervisor_run_can_write_reports_for_emergency_ticks() {
        let root = unique_temp_dir("buster-body-supervisor");
        let paths = BodyRuntimePaths::from_project_root(&root);
        let writer = BodyRuntimeWriter::new(paths);
        let controller = BodyController::new(DesiredBodyState::normal());
        let sensors =
            SensorSuite::new().with_sensor(SecretOutputSensor::new(vec![OutputObservation {
                unit_id: "unit-b".to_string(),
                text: "BEGIN PRIVATE KEY".to_string(),
            }]));
        let config = SupervisorConfig {
            tick_interval: Duration::from_millis(0),
            max_ticks: Some(1),
            write_reports: true,
        };
        let mut supervisor = BodySupervisor::new(controller, sensors, Some(writer), config);

        let ticks = supervisor.run().unwrap();

        assert_eq!(ticks.len(), 1);
        assert_eq!(ticks[0].report.next_mode, BodyMode::EmergencyContainment);
        assert_eq!(ticks[0].written_paths.len(), 3);

        let _ = fs::remove_dir_all(root);
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
