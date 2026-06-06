//! Sensors that produce body-layer signals.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::registry::{ManagedUnit, UnitStatus};
use crate::resources::ResourceBudget;
use crate::secrets::SecretRedactor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignalSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    AuthorityFileChanged,
    AuthorityFileMissing,
    SuspiciousOwnedUnit,
    ResourcePressure,
    NetworkAnomaly,
    SecretExposureRisk,
    PolicyViolation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorSignal {
    pub kind: SignalKind,
    pub severity: SignalSeverity,
    pub source: String,
    pub summary: String,
}

impl SensorSignal {
    pub fn info(kind: SignalKind, source: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            kind,
            severity: SignalSeverity::Info,
            source: source.into(),
            summary: summary.into(),
        }
    }

    pub fn warning(
        kind: SignalKind,
        source: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            severity: SignalSeverity::Warning,
            source: source.into(),
            summary: summary.into(),
        }
    }

    pub fn critical(
        kind: SignalKind,
        source: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            severity: SignalSeverity::Critical,
            source: source.into(),
            summary: summary.into(),
        }
    }
}

pub trait BodySensor {
    fn scan(&self) -> Vec<SensorSignal>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityFileSnapshot {
    pub path: PathBuf,
    pub len: u64,
    pub modified_secs: Option<u64>,
}

impl AuthorityFileSnapshot {
    pub fn capture(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        let metadata = fs::metadata(&path)?;
        let modified_secs = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs());

        Ok(Self {
            path,
            len: metadata.len(),
            modified_secs,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityFileSensor {
    baseline: Vec<AuthorityFileSnapshot>,
}

impl AuthorityFileSensor {
    pub fn new(baseline: Vec<AuthorityFileSnapshot>) -> Self {
        Self { baseline }
    }

    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>) -> std::io::Result<Self> {
        let mut baseline = Vec::new();
        for path in paths {
            baseline.push(AuthorityFileSnapshot::capture(path)?);
        }
        Ok(Self::new(baseline))
    }

    pub fn scan(&self) -> Vec<SensorSignal> {
        let mut signals = Vec::new();

        for expected in &self.baseline {
            match AuthorityFileSnapshot::capture(&expected.path) {
                Ok(actual) if actual == *expected => {}
                Ok(actual) => signals.push(SensorSignal::critical(
                    SignalKind::AuthorityFileChanged,
                    path_label(&expected.path),
                    format!(
                        "authority file changed: len {} -> {}, modified {:?} -> {:?}",
                        expected.len, actual.len, expected.modified_secs, actual.modified_secs
                    ),
                )),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    signals.push(SensorSignal::critical(
                        SignalKind::AuthorityFileMissing,
                        path_label(&expected.path),
                        "authority file is missing",
                    ));
                }
                Err(error) => signals.push(SensorSignal::warning(
                    SignalKind::PolicyViolation,
                    path_label(&expected.path),
                    format!("authority file could not be inspected: {error}"),
                )),
            }
        }

        signals
    }
}

impl BodySensor for AuthorityFileSensor {
    fn scan(&self) -> Vec<SensorSignal> {
        self.scan()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OwnedUnitSensor {
    units: Vec<ManagedUnit>,
    budget: ResourceBudget,
}

impl OwnedUnitSensor {
    pub fn new(units: Vec<ManagedUnit>, budget: ResourceBudget) -> Self {
        Self { units, budget }
    }
}

impl BodySensor for OwnedUnitSensor {
    fn scan(&self) -> Vec<SensorSignal> {
        let mut signals = Vec::new();

        for unit in &self.units {
            if matches!(unit.status, UnitStatus::Running)
                && unit
                    .notes
                    .iter()
                    .any(|note| note.contains("suspicious") || note.contains("anomaly"))
            {
                signals.push(SensorSignal::warning(
                    SignalKind::SuspiciousOwnedUnit,
                    unit.id.clone(),
                    "running Buster-owned unit is marked suspicious",
                ));
            }

            if !self.budget.allows(&unit.usage) {
                signals.push(SensorSignal::critical(
                    SignalKind::ResourcePressure,
                    unit.id.clone(),
                    "Buster-owned unit exceeded its resource budget",
                ));
            }
        }

        signals
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkObservation {
    pub unit_id: String,
    pub host: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkEgressSensor {
    observations: Vec<NetworkObservation>,
    allowed_hosts: Vec<String>,
}

impl NetworkEgressSensor {
    pub fn new(observations: Vec<NetworkObservation>, allowed_hosts: Vec<String>) -> Self {
        Self {
            observations,
            allowed_hosts,
        }
    }
}

impl BodySensor for NetworkEgressSensor {
    fn scan(&self) -> Vec<SensorSignal> {
        self.observations
            .iter()
            .filter(|observation| !host_allowed(&observation.host, &self.allowed_hosts))
            .map(|observation| {
                SensorSignal::warning(
                    SignalKind::NetworkAnomaly,
                    observation.unit_id.clone(),
                    format!("network egress to unapproved host `{}`", observation.host),
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputObservation {
    pub unit_id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretOutputSensor {
    observations: Vec<OutputObservation>,
    redactor: Option<SecretRedactor>,
}

impl SecretOutputSensor {
    pub fn new(observations: Vec<OutputObservation>) -> Self {
        Self {
            observations,
            redactor: None,
        }
    }

    pub fn with_redactor(mut self, redactor: SecretRedactor) -> Self {
        self.redactor = Some(redactor);
        self
    }
}

impl BodySensor for SecretOutputSensor {
    fn scan(&self) -> Vec<SensorSignal> {
        self.observations
            .iter()
            .filter(|observation| {
                looks_like_secret(&observation.text)
                    || self
                        .redactor
                        .as_ref()
                        .is_some_and(|redactor| redactor.detects_leak(&observation.text))
            })
            .map(|observation| {
                SensorSignal::critical(
                    SignalKind::SecretExposureRisk,
                    observation.unit_id.clone(),
                    "tool output appears to contain secret material",
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalScannerKind {
    WindowsDefender,
    ClamAv,
    Yara,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalScanFinding {
    pub scanner: ExternalScannerKind,
    pub target: String,
    pub threat: String,
    pub high_confidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalScanSensor {
    findings: Vec<ExternalScanFinding>,
}

impl ExternalScanSensor {
    pub fn new(findings: Vec<ExternalScanFinding>) -> Self {
        Self { findings }
    }
}

impl BodySensor for ExternalScanSensor {
    fn scan(&self) -> Vec<SensorSignal> {
        self.findings
            .iter()
            .map(|finding| {
                let summary = format!("{:?} reported `{}`", finding.scanner, finding.threat);
                if finding.high_confidence {
                    SensorSignal::critical(
                        SignalKind::PolicyViolation,
                        finding.target.clone(),
                        summary,
                    )
                } else {
                    SensorSignal::warning(
                        SignalKind::PolicyViolation,
                        finding.target.clone(),
                        summary,
                    )
                }
            })
            .collect()
    }
}

#[derive(Default)]
pub struct SensorSuite {
    sensors: Vec<Box<dyn BodySensor>>,
}

impl SensorSuite {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sensor(mut self, sensor: impl BodySensor + 'static) -> Self {
        self.sensors.push(Box::new(sensor));
        self
    }

    pub fn scan(&self) -> Vec<SensorSignal> {
        self.sensors
            .iter()
            .flat_map(|sensor| sensor.scan())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticSignalSensor {
    signals: Vec<SensorSignal>,
}

impl StaticSignalSensor {
    pub fn new(signals: Vec<SensorSignal>) -> Self {
        Self { signals }
    }
}

impl BodySensor for StaticSignalSensor {
    fn scan(&self) -> Vec<SensorSignal> {
        self.signals.clone()
    }
}

fn path_label(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn host_allowed(host: &str, allowed_hosts: &[String]) -> bool {
    allowed_hosts
        .iter()
        .any(|allowed| host == allowed || host.ends_with(&format!(".{allowed}")))
}

fn looks_like_secret(text: &str) -> bool {
    let markers = [
        "BEGIN PRIVATE KEY",
        "BEGIN OPENSSH PRIVATE KEY",
        "sk-",
        "ghp_",
        "github_pat_",
        "xoxb-",
        "AKIA",
        "Bearer ",
    ];

    markers.iter().any(|marker| text.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{ManagedUnit, UnitKind};
    use crate::resources::ResourceUsage;

    #[test]
    fn owned_unit_sensor_reports_suspicious_running_unit() {
        let mut unit =
            ManagedUnit::new("tool-a", UnitKind::Tool, "buster").with_status(UnitStatus::Running);
        unit.notes.push("suspicious egress anomaly".to_string());

        let signals = OwnedUnitSensor::new(vec![unit], ResourceBudget::default()).scan();

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].kind, SignalKind::SuspiciousOwnedUnit);
        assert_eq!(signals[0].severity, SignalSeverity::Warning);
    }

    #[test]
    fn owned_unit_sensor_reports_budget_exhaustion_as_critical() {
        let mut unit =
            ManagedUnit::new("tool-a", UnitKind::Tool, "buster").with_status(UnitStatus::Running);
        unit.usage = ResourceUsage {
            tokens: 101,
            ..ResourceUsage::default()
        };
        let budget = ResourceBudget {
            max_tokens: Some(100),
            ..ResourceBudget::default()
        };

        let signals = OwnedUnitSensor::new(vec![unit], budget).scan();

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].kind, SignalKind::ResourcePressure);
        assert_eq!(signals[0].severity, SignalSeverity::Critical);
    }

    #[test]
    fn network_sensor_reports_unapproved_hosts() {
        let observations = vec![
            NetworkObservation {
                unit_id: "llm".to_string(),
                host: "openrouter.ai".to_string(),
            },
            NetworkObservation {
                unit_id: "unknown".to_string(),
                host: "malicious.example".to_string(),
            },
        ];

        let signals =
            NetworkEgressSensor::new(observations, vec!["openrouter.ai".to_string()]).scan();

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].kind, SignalKind::NetworkAnomaly);
        assert_eq!(signals[0].source, "unknown");
    }

    #[test]
    fn secret_output_sensor_reports_secret_like_output() {
        let observations = vec![OutputObservation {
            unit_id: "tool-a".to_string(),
            text: "token: sk-test".to_string(),
        }];

        let signals = SecretOutputSensor::new(observations).scan();

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].kind, SignalKind::SecretExposureRisk);
        assert_eq!(signals[0].severity, SignalSeverity::Critical);
    }

    #[test]
    fn secret_output_sensor_uses_registered_secret_redactor() {
        let redactor = SecretRedactor::new(
            vec![crate::secrets::SecretFingerprint::from_secret(
                "or-key-1234",
                b"test-fingerprint-key",
            )],
            b"test-fingerprint-key".to_vec(),
        );
        let observations = vec![OutputObservation {
            unit_id: "tool-a".to_string(),
            text: "leaked or-key-1234".to_string(),
        }];

        let signals = SecretOutputSensor::new(observations)
            .with_redactor(redactor)
            .scan();

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].kind, SignalKind::SecretExposureRisk);
    }

    #[test]
    fn external_scan_sensor_converts_high_confidence_findings_to_critical() {
        let findings = vec![ExternalScanFinding {
            scanner: ExternalScannerKind::WindowsDefender,
            target: "tool.exe".to_string(),
            threat: "Trojan:Example".to_string(),
            high_confidence: true,
        }];

        let signals = ExternalScanSensor::new(findings).scan();

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].severity, SignalSeverity::Critical);
    }

    #[test]
    fn sensor_suite_collects_all_sensor_signals() {
        let suite = SensorSuite::new()
            .with_sensor(NetworkEgressSensor::new(
                vec![NetworkObservation {
                    unit_id: "unit-a".to_string(),
                    host: "blocked.example".to_string(),
                }],
                vec!["openrouter.ai".to_string()],
            ))
            .with_sensor(SecretOutputSensor::new(vec![OutputObservation {
                unit_id: "unit-b".to_string(),
                text: "BEGIN PRIVATE KEY".to_string(),
            }]));

        let signals = suite.scan();

        assert_eq!(signals.len(), 2);
    }
}
