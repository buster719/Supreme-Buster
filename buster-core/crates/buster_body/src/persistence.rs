//! Durable body state storage.
//!
//! The first store is intentionally file based: JSON snapshots for current
//! state and JSONL streams for append-only audit. This keeps the immune memory
//! inspectable while Buster's schema is still evolving.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

use crate::audit::AuditEvent;
use crate::host_api::{ChangeLevel, RiskLine};
use crate::registry::{ManagedUnit, UnitRegistry};

pub trait BodyStore {
    fn save_units(&self, registry: &UnitRegistry) -> Result<(), BodyStoreError>;
    fn load_units(&self) -> Result<UnitRegistry, BodyStoreError>;
    fn append_audit_event(&self, event: &AuditEvent) -> Result<(), BodyStoreError>;
    fn append_tool_action<T>(&self, record: &T) -> Result<(), BodyStoreError>
    where
        T: Serialize;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlBodyStore {
    root: PathBuf,
}

impl JsonlBodyStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write_snapshot<T>(&self, relative: &str, value: &T) -> Result<(), BodyStoreError>
    where
        T: Serialize,
    {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(value)?;
        fs::write(path, raw)?;
        Ok(())
    }

    pub fn read_snapshot<T>(&self, relative: &str) -> Result<T, BodyStoreError>
    where
        T: DeserializeOwned,
    {
        let raw = fs::read_to_string(self.root.join(relative))?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn read_snapshot_or_default<T>(&self, relative: &str) -> Result<T, BodyStoreError>
    where
        T: DeserializeOwned + Default,
    {
        let path = self.root.join(relative);
        if !path.exists() {
            return Ok(T::default());
        }
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn append_jsonl<T>(&self, relative: &str, value: &T) -> Result<(), BodyStoreError>
    where
        T: Serialize,
    {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string(value)?;
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        file.write_all(raw.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }
}

impl BodyStore for JsonlBodyStore {
    fn save_units(&self, registry: &UnitRegistry) -> Result<(), BodyStoreError> {
        self.write_snapshot("state/units.json", &registry.units())
    }

    fn load_units(&self) -> Result<UnitRegistry, BodyStoreError> {
        let units: Vec<ManagedUnit> = self.read_snapshot_or_default("state/units.json")?;
        Ok(UnitRegistry::from_units(units))
    }

    fn append_audit_event(&self, event: &AuditEvent) -> Result<(), BodyStoreError> {
        self.append_jsonl("audit/body-events.jsonl", event)
    }

    fn append_tool_action<T>(&self, record: &T) -> Result<(), BodyStoreError>
    where
        T: Serialize,
    {
        self.append_jsonl("audit/tool-actions.jsonl", record)
    }
}

#[derive(Debug)]
pub struct SqliteBodyStore {
    conn: Connection,
}

impl SqliteBodyStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BodyStoreError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, BodyStoreError> {
        let store = Self {
            conn: Connection::open_in_memory()?,
        };
        store.run_migrations()?;
        Ok(store)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    fn run_migrations(&self) -> Result<(), BodyStoreError> {
        self.conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS _body_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );
            "#,
        )?;

        for migration in BODY_MIGRATIONS {
            self.apply_migration(migration)?;
        }
        Ok(())
    }

    fn apply_migration(&self, migration: &BodyMigration) -> Result<(), BodyStoreError> {
        let checksum = sha256_hex(migration.sql);
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT checksum FROM _body_migrations WHERE version = ?1",
                params![migration.version],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(existing) = existing {
            if existing != checksum {
                return Err(BodyStoreError::MigrationChecksumMismatch {
                    version: migration.version,
                    name: migration.name.to_string(),
                    expected: existing,
                    actual: checksum,
                });
            }
            return Ok(());
        }

        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| {
            self.conn.execute_batch(migration.sql)?;
            self.conn.execute(
                "INSERT INTO _body_migrations (version, name, checksum) VALUES (?1, ?2, ?3)",
                params![migration.version, migration.name, checksum],
            )?;
            Ok::<(), rusqlite::Error>(())
        })();
        match result {
            Ok(()) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(BodyStoreError::Sqlite(error))
            }
        }
    }

    pub fn audit_event_count(&self) -> Result<u64, BodyStoreError> {
        let count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM body_audit_events", [], |row| {
                    row.get(0)
                })?;
        Ok(count.max(0) as u64)
    }

    pub fn tool_action_count(&self) -> Result<u64, BodyStoreError> {
        let count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM tool_action_audit", [], |row| {
                    row.get(0)
                })?;
        Ok(count.max(0) as u64)
    }
}

impl BodyStore for SqliteBodyStore {
    fn save_units(&self, registry: &UnitRegistry) -> Result<(), BodyStoreError> {
        self.conn.execute("DELETE FROM body_units", [])?;
        for unit in registry.units() {
            self.conn.execute(
                r#"
                INSERT INTO body_units
                    (id, kind, status, owner, capability_name, lease_reason, unit_json, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                "#,
                params![
                    unit.id,
                    format!("{:?}", unit.kind),
                    format!("{:?}", unit.status),
                    unit.owner,
                    unit.lease
                        .as_ref()
                        .map(|lease| lease.capability_name.as_str()),
                    unit.lease.as_ref().map(|lease| lease.reason.as_str()),
                    serde_json::to_string(unit)?,
                ],
            )?;
        }
        Ok(())
    }

    fn load_units(&self) -> Result<UnitRegistry, BodyStoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT unit_json FROM body_units ORDER BY id")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut units = Vec::new();
        for row in rows {
            let raw = row?;
            units.push(serde_json::from_str::<ManagedUnit>(&raw)?);
        }
        Ok(UnitRegistry::from_units(units))
    }

    fn append_audit_event(&self, event: &AuditEvent) -> Result<(), BodyStoreError> {
        self.conn.execute(
            r#"
            INSERT INTO body_audit_events
                (level, change_level, risk_line, summary, consequence, event_json)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                format!("{:?}", event.level),
                change_level_number(event.change_level),
                risk_line_name(event.risk_line),
                event.summary,
                event.consequence,
                serde_json::to_string(event)?,
            ],
        )?;
        Ok(())
    }

    fn append_tool_action<T>(&self, record: &T) -> Result<(), BodyStoreError>
    where
        T: Serialize,
    {
        let value = serde_json::to_value(record)?;
        self.conn.execute(
            r#"
            INSERT INTO tool_action_audit
                (tool_name, action, identity, target_hmac, input_hmac, input_len,
                 message_id, lease_id, result, record_json)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                json_string(&value, "tool_name"),
                json_string(&value, "action"),
                json_string(&value, "identity"),
                json_string(&value, "target_hmac"),
                json_string(&value, "input_hmac"),
                json_u64(&value, "input_len").map(|value| value as i64),
                json_string(&value, "message_id"),
                json_string(&value, "lease_id"),
                json_string(&value, "result"),
                serde_json::to_string(&value)?,
            ],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct BodyMigration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

const BODY_MIGRATIONS: &[BodyMigration] = &[BodyMigration {
    version: 1,
    name: "body_core",
    sql: r#"
CREATE TABLE IF NOT EXISTS body_units (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    owner TEXT NOT NULL,
    capability_name TEXT,
    lease_reason TEXT,
    unit_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_body_units_status ON body_units(status);
CREATE INDEX IF NOT EXISTS idx_body_units_kind ON body_units(kind);
CREATE INDEX IF NOT EXISTS idx_body_units_capability ON body_units(capability_name);

CREATE TABLE IF NOT EXISTS body_audit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    level TEXT NOT NULL,
    change_level INTEGER NOT NULL,
    risk_line TEXT NOT NULL,
    summary TEXT NOT NULL,
    consequence TEXT,
    event_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_body_audit_created ON body_audit_events(created_at);
CREATE INDEX IF NOT EXISTS idx_body_audit_level ON body_audit_events(level);
CREATE INDEX IF NOT EXISTS idx_body_audit_change_level ON body_audit_events(change_level);

CREATE TABLE IF NOT EXISTS tool_action_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    tool_name TEXT,
    action TEXT,
    identity TEXT,
    target_hmac TEXT,
    input_hmac TEXT,
    input_len INTEGER,
    message_id TEXT,
    lease_id TEXT,
    result TEXT,
    record_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tool_action_tool ON tool_action_audit(tool_name);
CREATE INDEX IF NOT EXISTS idx_tool_action_action ON tool_action_audit(action);
CREATE INDEX IF NOT EXISTS idx_tool_action_message_id ON tool_action_audit(message_id);
CREATE INDEX IF NOT EXISTS idx_tool_action_created ON tool_action_audit(created_at);
"#,
}];

#[derive(Debug)]
pub enum BodyStoreError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Sqlite(rusqlite::Error),
    MigrationChecksumMismatch {
        version: u32,
        name: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for BodyStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "body store I/O error: {error}"),
            Self::Json(error) => write!(formatter, "body store JSON error: {error}"),
            Self::Sqlite(error) => write!(formatter, "body store SQLite error: {error}"),
            Self::MigrationChecksumMismatch {
                version,
                name,
                expected,
                actual,
            } => write!(
                formatter,
                "body migration V{version} ({name}) checksum mismatch: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for BodyStoreError {}

impl From<std::io::Error> for BodyStoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for BodyStoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<rusqlite::Error> for BodyStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

fn change_level_number(change_level: ChangeLevel) -> i64 {
    change_level as i64
}

fn risk_line_name(risk_line: RiskLine) -> &'static str {
    match risk_line {
        RiskLine::Green => "green",
        RiskLine::Yellow => "yellow",
        RiskLine::Red => "red",
    }
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn json_u64(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(serde_json::Value::as_u64)
}

fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let bytes = hasher.finalize();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditLevel;
    use crate::host_api::{BodyScope, ChangeLevel, RiskLine};
    use crate::registry::{ManagedUnit, UnitKind, UnitStatus};

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("buster-body-store-{name}-{}", std::process::id()))
    }

    #[test]
    fn store_round_trips_unit_registry_snapshot() {
        let root = temp_root("units");
        let _ = fs::remove_dir_all(&root);
        let store = JsonlBodyStore::new(&root);
        let mut registry = UnitRegistry::default();
        registry.register(
            ManagedUnit::new("tool-1", UnitKind::Tool, "buster").with_status(UnitStatus::Running),
        );

        store.save_units(&registry).unwrap();
        let restored = store.load_units().unwrap();

        assert_eq!(restored.units(), registry.units());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn store_appends_audit_jsonl_without_overwriting() {
        let root = temp_root("audit");
        let _ = fs::remove_dir_all(&root);
        let store = JsonlBodyStore::new(&root);
        let event = AuditEvent {
            level: AuditLevel::Warning,
            change_level: ChangeLevel::BoundaryResource,
            risk_line: RiskLine::Yellow,
            summary: "tool quarantine recommended".to_string(),
            consequence: Some("failure count exceeded".to_string()),
        };

        store.append_audit_event(&event).unwrap();
        store.append_audit_event(&event).unwrap();

        let raw = fs::read_to_string(root.join("audit/body-events.jsonl")).unwrap();
        assert_eq!(raw.lines().count(), 2);
        assert!(raw.contains("tool quarantine recommended"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_snapshot_loads_default_registry() {
        let root = temp_root("missing");
        let _ = fs::remove_dir_all(&root);
        let store = JsonlBodyStore::new(&root);

        let registry = store.load_units().unwrap();

        assert!(registry.units().is_empty());
    }

    #[test]
    fn generic_tool_action_stream_accepts_json_records() {
        let root = temp_root("tool-actions");
        let _ = fs::remove_dir_all(&root);
        let store = JsonlBodyStore::new(&root);
        let record = serde_json::json!({
            "tool": "feishu.lark_cli",
            "action": "im.message_send",
            "target_hmac": "abc123"
        });

        store.append_tool_action(&record).unwrap();

        let raw = fs::read_to_string(root.join("audit/tool-actions.jsonl")).unwrap();
        assert!(raw.contains("feishu.lark_cli"));
        assert!(!raw.contains("hello from buster"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn capability_lease_inside_unit_is_serializable() {
        let root = temp_root("lease");
        let _ = fs::remove_dir_all(&root);
        let store = JsonlBodyStore::new(&root);
        let mut registry = UnitRegistry::default();
        registry.register(
            ManagedUnit::new("script-1", UnitKind::Script, "buster").with_lease(
                crate::capabilities::CapabilityLease {
                    scope: BodyScope::new("buster", "main", "task"),
                    capability_name: "script.run".to_string(),
                    expires_at: Some("2026-05-31T00:00:00Z".to_string()),
                    reason: "persistence test".to_string(),
                },
            ),
        );

        store.save_units(&registry).unwrap();
        let restored = store.load_units().unwrap();

        assert_eq!(
            restored.units()[0].lease.as_ref().unwrap().capability_name,
            "script.run"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sqlite_store_round_trips_unit_registry_snapshot() {
        let store = SqliteBodyStore::open_in_memory().unwrap();
        let mut registry = UnitRegistry::default();
        registry.register(
            ManagedUnit::new("tool-1", UnitKind::Tool, "buster").with_status(UnitStatus::Running),
        );

        store.save_units(&registry).unwrap();
        let restored = store.load_units().unwrap();

        assert_eq!(restored.units(), registry.units());
    }

    #[test]
    fn sqlite_store_appends_body_audit_events() {
        let store = SqliteBodyStore::open_in_memory().unwrap();
        let event = AuditEvent {
            level: AuditLevel::Critical,
            change_level: ChangeLevel::BoundaryResource,
            risk_line: RiskLine::Red,
            summary: "secret leak blocked".to_string(),
            consequence: Some("entered containment".to_string()),
        };

        store.append_audit_event(&event).unwrap();
        store.append_audit_event(&event).unwrap();

        assert_eq!(store.audit_event_count().unwrap(), 2);
    }

    #[test]
    fn sqlite_store_indexes_tool_action_summary_without_plain_content() {
        let store = SqliteBodyStore::open_in_memory().unwrap();
        let record = serde_json::json!({
            "tool_name": "feishu.lark_cli",
            "action": "im.message_send",
            "identity": "bot",
            "target_hmac": "target-hmac",
            "input_hmac": "input-hmac",
            "input_len": 18,
            "message_id": "om_test_123",
            "lease_id": "tool-lease-feishu",
            "result": "Success"
        });

        store.append_tool_action(&record).unwrap();

        assert_eq!(store.tool_action_count().unwrap(), 1);
        let tool_name: String = store
            .connection()
            .query_row(
                "SELECT tool_name FROM tool_action_audit WHERE message_id = ?1",
                params!["om_test_123"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tool_name, "feishu.lark_cli");
    }

    #[test]
    fn sqlite_store_rejects_changed_applied_migration_checksum() {
        let store = SqliteBodyStore::open_in_memory().unwrap();
        store
            .connection()
            .execute(
                "UPDATE _body_migrations SET checksum = ?1 WHERE version = 1",
                params!["tampered"],
            )
            .unwrap();
        let error = store.run_migrations().unwrap_err();

        assert!(matches!(
            error,
            BodyStoreError::MigrationChecksumMismatch { version: 1, .. }
        ));
    }
}
