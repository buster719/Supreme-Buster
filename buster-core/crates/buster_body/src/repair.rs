//! Controlled repair helpers for body-runtime experiments.
//!
//! These helpers are deliberately narrow: they restore missing authority files
//! from a known-good source directory into a target runtime root. They are meant
//! for sandboxed recovery flows and future approved body-repair mechanisms, not
//! arbitrary mutation of the live identity documents.

use std::fs;
use std::path::{Path, PathBuf};

use crate::sensors::{SensorSignal, SignalKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairOutcome {
    pub target: PathBuf,
    pub source: PathBuf,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityFileRepairer {
    target_root: PathBuf,
    source_root: PathBuf,
}

impl AuthorityFileRepairer {
    pub fn new(target_root: impl Into<PathBuf>, source_root: impl Into<PathBuf>) -> Self {
        Self {
            target_root: target_root.into(),
            source_root: source_root.into(),
        }
    }

    pub fn repair_missing_files(
        &self,
        signals: &[SensorSignal],
    ) -> std::io::Result<Vec<RepairOutcome>> {
        let mut outcomes = Vec::new();

        for signal in signals
            .iter()
            .filter(|signal| signal.kind == SignalKind::AuthorityFileMissing)
        {
            let Some(file_name) = authority_file_name(&signal.source) else {
                continue;
            };
            let source = self.source_root.join(file_name);
            let target = self.target_root.join(file_name);

            if target.exists() {
                continue;
            }

            fs::copy(&source, &target)?;
            outcomes.push(RepairOutcome {
                target,
                source,
                summary: format!(
                    "restored missing authority file `{file_name}` from known-good source"
                ),
            });
        }

        Ok(outcomes)
    }
}

fn authority_file_name(source: &str) -> Option<&'static str> {
    let path = Path::new(source);
    match path.file_name().and_then(|file_name| file_name.to_str()) {
        Some("self.md") => Some("self.md"),
        Some("GOVERNANCE.md") => Some("GOVERNANCE.md"),
        Some("BODY.md") => Some("BODY.md"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::sensors::{SensorSignal, SignalKind};

    #[test]
    fn repairer_restores_missing_body_from_known_good_source() {
        let root = unique_temp_dir("buster-repair-root");
        let source = unique_temp_dir("buster-repair-source");
        fs::write(source.join("BODY.md"), "healthy body").unwrap();

        let signal = SensorSignal::critical(
            SignalKind::AuthorityFileMissing,
            root.join("BODY.md").display().to_string(),
            "required authority file is missing",
        );
        let repairer = AuthorityFileRepairer::new(&root, &source);

        let outcomes = repairer.repair_missing_files(&[signal]).unwrap();

        assert_eq!(outcomes.len(), 1);
        assert_eq!(
            fs::read_to_string(root.join("BODY.md")).unwrap(),
            "healthy body"
        );

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(source);
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
