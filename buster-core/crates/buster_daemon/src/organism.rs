//! Local cyber-organism primitives.
//!
//! v0 deliberately uses a local node key, HMAC signatures, and an append-only
//! hash chain. This is not the final Master Key design; it is the first
//! verifiable substrate that can later be witnessed by other nodes or anchored
//! to an external ledger.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

const NODE_KEY_LEN: usize = 32;
const NODE_KEY_PATH: &str = "secrets/node-key.b64";
const NODE_IDENTITY_PATH: &str = "state/node-identity.json";
const BODY_MANIFEST_PATH: &str = "state/body-manifest.json";
const EVENT_LOG_PATH: &str = "audit/events.jsonl";
const EVENT_HEAD_PATH: &str = "state/event-chain-head.txt";
const WITNESS_KEY_PATH: &str = "secrets/node-witness-ed25519.b64";
const KNOWN_NODES_PATH: &str = "state/known-nodes.json";
const WITNESS_RECEIPTS_PATH: &str = "audit/witness-receipts.jsonl";
const SIGNATURE_SCHEME_V0: &str = "hmac-sha256-local-node-key-v0";
const SIGNATURE_SCHEME_V1: &str = "hmac-sha256-local-node-key-v1-payload-hash";
const WITNESS_SIGNATURE_SCHEME: &str = "ed25519-witness-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeIdentity {
    pub node_id: String,
    pub key_scheme: String,
    pub created_at_secs: u64,
    pub node_key_fingerprint: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness_key_scheme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness_public_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyManifest {
    pub node_id: String,
    pub manifest_version: u32,
    pub generated_at_secs: u64,
    pub runtime: String,
    pub operating_context: String,
    pub workspace_root: String,
    pub capabilities: Vec<String>,
    pub protected_authority_files: Vec<AuthorityFileHash>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityFileHash {
    pub path: String,
    pub sha256: String,
    pub present: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignedEventRecord {
    pub event_id: String,
    pub event_type: String,
    pub timestamp_secs: u64,
    pub node_id: String,
    pub previous_event_hash: Option<String>,
    pub payload_hash: String,
    pub payload: serde_json::Value,
    pub event_hash: String,
    pub signature_scheme: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub checked_events: usize,
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSnapshot {
    pub snapshot_version: u32,
    pub generated_at_secs: u64,
    pub node_identity: NodeIdentity,
    pub body_manifest_hash: String,
    pub chain_head: Option<String>,
    pub event_count: usize,
    pub recent_event_hashes: Vec<String>,
    pub signature_scheme: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessReceipt {
    pub receipt_version: u32,
    pub observed_at_secs: u64,
    pub witness_node_id: String,
    pub witness_role: String,
    pub witness_public_key: String,
    pub subject_node_id: String,
    pub subject_role: String,
    pub subject_public_key: String,
    pub subject_chain_head: Option<String>,
    pub subject_event_count: usize,
    pub subject_snapshot_hash: String,
    pub subject_snapshot_signature: String,
    pub judgement: String,
    pub notes: Vec<String>,
    pub signature_scheme: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownNodesRegistry {
    pub version: u32,
    pub updated_at_secs: u64,
    pub nodes: Vec<KnownNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownNode {
    pub node_id: String,
    pub role: String,
    pub witness_public_key: String,
    pub first_seen_secs: u64,
    pub last_seen_secs: u64,
    pub last_chain_head: Option<String>,
    pub last_event_count: usize,
    pub last_snapshot_hash: Option<String>,
    pub last_receipt_hash: Option<String>,
}

#[derive(Debug, Serialize)]
struct EventHashPreimage<'a> {
    event_id: &'a str,
    event_type: &'a str,
    timestamp_secs: u64,
    node_id: &'a str,
    previous_event_hash: &'a Option<String>,
    payload_hash: &'a str,
    signature_scheme: &'a str,
}

#[derive(Debug, Serialize)]
struct NodeSnapshotPreimage<'a> {
    snapshot_version: u32,
    generated_at_secs: u64,
    node_identity: &'a NodeIdentity,
    body_manifest_hash: &'a str,
    chain_head: &'a Option<String>,
    event_count: usize,
    recent_event_hashes: &'a [String],
    signature_scheme: &'a str,
}

#[derive(Debug, Serialize)]
struct WitnessReceiptPreimage<'a> {
    receipt_version: u32,
    observed_at_secs: u64,
    witness_node_id: &'a str,
    witness_role: &'a str,
    witness_public_key: &'a str,
    subject_node_id: &'a str,
    subject_role: &'a str,
    subject_public_key: &'a str,
    subject_chain_head: &'a Option<String>,
    subject_event_count: usize,
    subject_snapshot_hash: &'a str,
    subject_snapshot_signature: &'a str,
    judgement: &'a str,
    notes: &'a [String],
    signature_scheme: &'a str,
}

pub fn ensure_node(root: &Path) -> std::io::Result<NodeIdentity> {
    if let Some(mut identity) = read_node_identity(root)? {
        let signing_key = ensure_witness_signing_key(root)?;
        let public_key = witness_public_key_b64(&signing_key);
        if identity.witness_public_key.as_deref() != Some(&public_key)
            || identity.witness_key_scheme.as_deref() != Some(WITNESS_SIGNATURE_SCHEME)
        {
            identity.witness_key_scheme = Some(WITNESS_SIGNATURE_SCHEME.to_string());
            identity.witness_public_key = Some(public_key);
            write_json_pretty(&root.join(NODE_IDENTITY_PATH), &identity)?;
        }
        return Ok(identity);
    }

    let mut key = [0u8; NODE_KEY_LEN];
    rand::thread_rng().fill_bytes(&mut key);
    write_node_key(root, &key)?;
    let witness_key = ensure_witness_signing_key(root)?;

    let fingerprint = sha256_hex(&key);
    let node_id = format!("buster-node-{}", &fingerprint[..16]);
    let identity = NodeIdentity {
        node_id,
        key_scheme: "hmac-sha256-local-node-key-v0".to_string(),
        created_at_secs: now_secs(),
        node_key_fingerprint: fingerprint,
        role: "local-primary-host".to_string(),
        witness_key_scheme: Some(WITNESS_SIGNATURE_SCHEME.to_string()),
        witness_public_key: Some(witness_public_key_b64(&witness_key)),
    };
    write_json_pretty(&root.join(NODE_IDENTITY_PATH), &identity)?;
    Ok(identity)
}

pub fn write_body_manifest(root: &Path, identity: &NodeIdentity) -> std::io::Result<BodyManifest> {
    let manifest = BodyManifest {
        node_id: identity.node_id.clone(),
        manifest_version: 1,
        generated_at_secs: now_secs(),
        runtime: "buster_daemon".to_string(),
        operating_context: operating_context(),
        workspace_root: root.to_string_lossy().into_owned(),
        capabilities: vec![
            "web_console".to_string(),
            "runtime_tick".to_string(),
            "forever_loop".to_string(),
            "openrouter_llm".to_string(),
            "local_audit".to_string(),
            "signed_event_log".to_string(),
        ],
        protected_authority_files: [
            "self.md",
            "GOVERNANCE.md",
            "BODY.md",
            "VALUE_MODEL.md",
            "CYBER_ORGANISM.md",
            "CYBER_ORGANISM_ROADMAP.md",
        ]
        .into_iter()
        .map(|name| authority_file_hash(root, name))
        .collect(),
        notes: vec![
            "This manifest describes the local node body, not Buster's full identity.".to_string(),
            "The local node key is not the Master Key.".to_string(),
        ],
    };
    write_json_pretty(&root.join(BODY_MANIFEST_PATH), &manifest)?;
    Ok(manifest)
}

pub fn append_signed_event(
    root: &Path,
    event_type: impl Into<String>,
    payload: serde_json::Value,
) -> std::io::Result<SignedEventRecord> {
    let identity = ensure_node(root)?;
    let key = read_node_key(root)?;
    let timestamp_secs = now_secs();
    let event_type = event_type.into();
    let previous_event_hash = read_chain_head(root)?;
    let payload_hash = sha256_json(&payload)?;
    let event_id = format!(
        "{}-{}-{}",
        timestamp_secs,
        identity.node_id,
        &payload_hash[..12]
    );
    let preimage = EventHashPreimage {
        event_id: &event_id,
        event_type: &event_type,
        timestamp_secs,
        node_id: &identity.node_id,
        previous_event_hash: &previous_event_hash,
        payload_hash: &payload_hash,
        signature_scheme: SIGNATURE_SCHEME_V1,
    };
    let event_hash = sha256_json(&preimage)?;
    let signature = hmac_hex(&key, event_hash.as_bytes())?;
    let record = SignedEventRecord {
        event_id,
        event_type,
        timestamp_secs,
        node_id: identity.node_id,
        previous_event_hash,
        payload_hash,
        payload,
        event_hash: event_hash.clone(),
        signature_scheme: SIGNATURE_SCHEME_V1.to_string(),
        signature,
    };

    append_jsonl(&root.join(EVENT_LOG_PATH), &record)?;
    write_text(&root.join(EVENT_HEAD_PATH), &event_hash)?;
    Ok(record)
}

pub fn append_heartbeat(root: &Path) -> std::io::Result<SignedEventRecord> {
    let identity = ensure_node(root)?;
    let manifest = write_body_manifest(root, &identity)?;
    append_signed_event(
        root,
        "node_heartbeat",
        serde_json::json!({
            "node_id": identity.node_id,
            "body_manifest_hash": sha256_json(&manifest)?,
            "status": "alive",
        }),
    )
}

pub fn export_snapshot(root: &Path) -> std::io::Result<NodeSnapshot> {
    let identity = ensure_node(root)?;
    let signing_key = ensure_witness_signing_key(root)?;
    let manifest = write_body_manifest(root, &identity)?;
    let body_manifest_hash = sha256_json(&manifest)?;
    let chain_head = read_chain_head(root)?;
    let events = read_event_records(root)?;
    let event_count = events.len();
    let recent_event_hashes = events
        .iter()
        .rev()
        .take(8)
        .map(|event| event.event_hash.clone())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let mut snapshot = NodeSnapshot {
        snapshot_version: 1,
        generated_at_secs: now_secs(),
        node_identity: identity,
        body_manifest_hash,
        chain_head,
        event_count,
        recent_event_hashes,
        signature_scheme: WITNESS_SIGNATURE_SCHEME.to_string(),
        signature: String::new(),
    };
    let preimage = snapshot_preimage(&snapshot);
    snapshot.signature = sign_json(&signing_key, &preimage)?;
    write_json_pretty(&root.join("state").join("node-snapshot.json"), &snapshot)?;
    Ok(snapshot)
}

pub fn witness_snapshot(root: &Path, snapshot: &NodeSnapshot) -> std::io::Result<WitnessReceipt> {
    verify_snapshot(root, snapshot)?;
    let witness = ensure_node(root)?;
    let signing_key = ensure_witness_signing_key(root)?;
    let witness_public_key = witness
        .witness_public_key
        .clone()
        .ok_or_else(|| std::io::Error::other("witness node has no public witness key"))?;
    let subject_public_key = snapshot
        .node_identity
        .witness_public_key
        .clone()
        .ok_or_else(|| std::io::Error::other("subject node has no public witness key"))?;
    let snapshot_hash = sha256_json(snapshot)?;
    let notes = vec![
        "snapshot signature verified".to_string(),
        "known-node public key matched or was bound by first observation".to_string(),
    ];
    let mut receipt = WitnessReceipt {
        receipt_version: 1,
        observed_at_secs: now_secs(),
        witness_node_id: witness.node_id,
        witness_role: witness.role,
        witness_public_key,
        subject_node_id: snapshot.node_identity.node_id.clone(),
        subject_role: snapshot.node_identity.role.clone(),
        subject_public_key,
        subject_chain_head: snapshot.chain_head.clone(),
        subject_event_count: snapshot.event_count,
        subject_snapshot_hash: snapshot_hash,
        subject_snapshot_signature: snapshot.signature.clone(),
        judgement: "observed".to_string(),
        notes,
        signature_scheme: WITNESS_SIGNATURE_SCHEME.to_string(),
        signature: String::new(),
    };
    let preimage = receipt_preimage(&receipt);
    receipt.signature = sign_json(&signing_key, &preimage)?;
    store_witness_receipt(root, &receipt)?;
    append_signed_event(
        root,
        "witness_observed_node",
        serde_json::json!({
            "witness_node_id": receipt.witness_node_id,
            "subject_node_id": receipt.subject_node_id,
            "subject_chain_head": receipt.subject_chain_head,
            "subject_event_count": receipt.subject_event_count,
            "subject_snapshot_hash": receipt.subject_snapshot_hash,
            "receipt_hash": sha256_json(&receipt)?,
        }),
    )?;
    Ok(receipt)
}

pub fn import_witness_receipt(root: &Path, receipt: &WitnessReceipt) -> std::io::Result<()> {
    verify_witness_receipt(root, receipt)?;
    store_witness_receipt(root, receipt)?;
    append_signed_event(
        root,
        "witness_receipt_imported",
        serde_json::json!({
            "witness_node_id": receipt.witness_node_id,
            "subject_node_id": receipt.subject_node_id,
            "subject_chain_head": receipt.subject_chain_head,
            "subject_event_count": receipt.subject_event_count,
            "subject_snapshot_hash": receipt.subject_snapshot_hash,
            "receipt_hash": sha256_json(receipt)?,
        }),
    )?;
    Ok(())
}

pub fn verify_snapshot(root: &Path, snapshot: &NodeSnapshot) -> std::io::Result<()> {
    if snapshot.signature_scheme != WITNESS_SIGNATURE_SCHEME {
        return Err(std::io::Error::other(format!(
            "unsupported snapshot signature scheme {}",
            snapshot.signature_scheme
        )));
    }
    if snapshot.node_identity.witness_key_scheme.as_deref() != Some(WITNESS_SIGNATURE_SCHEME) {
        return Err(std::io::Error::other(
            "snapshot identity has unsupported witness key scheme",
        ));
    }
    let public_key = snapshot
        .node_identity
        .witness_public_key
        .as_deref()
        .ok_or_else(|| std::io::Error::other("snapshot identity has no witness public key"))?;
    verify_json_signature(
        public_key,
        &snapshot_preimage(snapshot),
        &snapshot.signature,
    )?;
    if let Some(head) = &snapshot.chain_head {
        if !snapshot.recent_event_hashes.iter().any(|hash| hash == head) {
            return Err(std::io::Error::other(
                "snapshot chain head is not included in recent_event_hashes",
            ));
        }
    }
    bind_known_node_from_snapshot(root, snapshot)?;
    Ok(())
}

pub fn verify_witness_receipt(root: &Path, receipt: &WitnessReceipt) -> std::io::Result<()> {
    if receipt.signature_scheme != WITNESS_SIGNATURE_SCHEME {
        return Err(std::io::Error::other(format!(
            "unsupported receipt signature scheme {}",
            receipt.signature_scheme
        )));
    }
    verify_json_signature(
        &receipt.witness_public_key,
        &receipt_preimage(receipt),
        &receipt.signature,
    )?;
    bind_known_node(
        root,
        &receipt.witness_node_id,
        &receipt.witness_role,
        &receipt.witness_public_key,
        None,
        0,
        None,
        Some(sha256_json(receipt)?),
    )?;
    Ok(())
}

pub fn read_known_nodes(root: &Path) -> std::io::Result<KnownNodesRegistry> {
    read_registry(root)
}

pub fn verify_event_log(root: &Path) -> std::io::Result<VerifyReport> {
    let path = root.join(EVENT_LOG_PATH);
    if !path.exists() {
        return Ok(VerifyReport {
            checked_events: 0,
            valid: true,
            error: None,
        });
    }
    let key = read_node_key(root)?;
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut expected_previous = None;
    let mut checked = 0usize;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: SignedEventRecord =
            serde_json::from_str(&line).map_err(std::io::Error::other)?;
        if record.previous_event_hash != expected_previous {
            return Ok(VerifyReport {
                checked_events: checked,
                valid: false,
                error: Some(format!("event {} previous hash mismatch", record.event_id)),
            });
        }
        let raw_payload = extract_raw_payload(&line)?;
        let expected_payload_hash = sha256_bytes(raw_payload.as_bytes());
        if record.payload_hash != expected_payload_hash {
            return Ok(VerifyReport {
                checked_events: checked,
                valid: false,
                error: Some(format!("event {} payload hash mismatch", record.event_id)),
            });
        }
        let expected_event_hash = expected_event_hash(&record, &raw_payload)?;
        if record.event_hash != expected_event_hash {
            return Ok(VerifyReport {
                checked_events: checked,
                valid: false,
                error: Some(format!("event {} hash mismatch", record.event_id)),
            });
        }
        let expected_signature = hmac_hex(&key, record.event_hash.as_bytes())?;
        if record.signature != expected_signature {
            return Ok(VerifyReport {
                checked_events: checked,
                valid: false,
                error: Some(format!("event {} signature mismatch", record.event_id)),
            });
        }
        expected_previous = Some(record.event_hash);
        checked += 1;
    }

    Ok(VerifyReport {
        checked_events: checked,
        valid: true,
        error: None,
    })
}

pub fn node_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join(NODE_IDENTITY_PATH),
        root.join(BODY_MANIFEST_PATH),
        root.join(EVENT_LOG_PATH),
        root.join(EVENT_HEAD_PATH),
        root.join(KNOWN_NODES_PATH),
        root.join(WITNESS_RECEIPTS_PATH),
    ]
}

fn snapshot_preimage(snapshot: &NodeSnapshot) -> NodeSnapshotPreimage<'_> {
    NodeSnapshotPreimage {
        snapshot_version: snapshot.snapshot_version,
        generated_at_secs: snapshot.generated_at_secs,
        node_identity: &snapshot.node_identity,
        body_manifest_hash: &snapshot.body_manifest_hash,
        chain_head: &snapshot.chain_head,
        event_count: snapshot.event_count,
        recent_event_hashes: &snapshot.recent_event_hashes,
        signature_scheme: &snapshot.signature_scheme,
    }
}

fn receipt_preimage(receipt: &WitnessReceipt) -> WitnessReceiptPreimage<'_> {
    WitnessReceiptPreimage {
        receipt_version: receipt.receipt_version,
        observed_at_secs: receipt.observed_at_secs,
        witness_node_id: &receipt.witness_node_id,
        witness_role: &receipt.witness_role,
        witness_public_key: &receipt.witness_public_key,
        subject_node_id: &receipt.subject_node_id,
        subject_role: &receipt.subject_role,
        subject_public_key: &receipt.subject_public_key,
        subject_chain_head: &receipt.subject_chain_head,
        subject_event_count: receipt.subject_event_count,
        subject_snapshot_hash: &receipt.subject_snapshot_hash,
        subject_snapshot_signature: &receipt.subject_snapshot_signature,
        judgement: &receipt.judgement,
        notes: &receipt.notes,
        signature_scheme: &receipt.signature_scheme,
    }
}

fn bind_known_node_from_snapshot(root: &Path, snapshot: &NodeSnapshot) -> std::io::Result<()> {
    let public_key = snapshot
        .node_identity
        .witness_public_key
        .as_deref()
        .ok_or_else(|| std::io::Error::other("snapshot identity has no witness public key"))?;
    bind_known_node(
        root,
        &snapshot.node_identity.node_id,
        &snapshot.node_identity.role,
        public_key,
        snapshot.chain_head.clone(),
        snapshot.event_count,
        Some(sha256_json(snapshot)?),
        None,
    )
}

fn bind_known_node(
    root: &Path,
    node_id: &str,
    role: &str,
    public_key: &str,
    chain_head: Option<String>,
    event_count: usize,
    snapshot_hash: Option<String>,
    receipt_hash: Option<String>,
) -> std::io::Result<()> {
    let mut registry = read_registry(root)?;
    let now = now_secs();
    if let Some(node) = registry
        .nodes
        .iter_mut()
        .find(|node| node.node_id == node_id)
    {
        if node.witness_public_key != public_key {
            return Err(std::io::Error::other(format!(
                "known node {node_id} changed witness public key"
            )));
        }
        node.role = role.to_string();
        node.last_seen_secs = now;
        if chain_head.is_some() {
            node.last_chain_head = chain_head;
        }
        if event_count > 0 {
            node.last_event_count = event_count;
        }
        if snapshot_hash.is_some() {
            node.last_snapshot_hash = snapshot_hash;
        }
        if receipt_hash.is_some() {
            node.last_receipt_hash = receipt_hash;
        }
    } else {
        registry.nodes.push(KnownNode {
            node_id: node_id.to_string(),
            role: role.to_string(),
            witness_public_key: public_key.to_string(),
            first_seen_secs: now,
            last_seen_secs: now,
            last_chain_head: chain_head,
            last_event_count: event_count,
            last_snapshot_hash: snapshot_hash,
            last_receipt_hash: receipt_hash,
        });
    }
    registry.updated_at_secs = now;
    registry.nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    write_json_pretty(&root.join(KNOWN_NODES_PATH), &registry)
}

fn read_registry(root: &Path) -> std::io::Result<KnownNodesRegistry> {
    let path = root.join(KNOWN_NODES_PATH);
    if !path.exists() {
        return Ok(KnownNodesRegistry {
            version: 1,
            updated_at_secs: 0,
            nodes: Vec::new(),
        });
    }
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(std::io::Error::other)
}

fn store_witness_receipt(root: &Path, receipt: &WitnessReceipt) -> std::io::Result<()> {
    append_jsonl(&root.join(WITNESS_RECEIPTS_PATH), receipt)
}

fn read_event_records(root: &Path) -> std::io::Result<Vec<SignedEventRecord>> {
    let path = root.join(EVENT_LOG_PATH);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        records.push(serde_json::from_str(&line).map_err(std::io::Error::other)?);
    }
    Ok(records)
}

fn read_node_identity(root: &Path) -> std::io::Result<Option<NodeIdentity>> {
    let path = root.join(NODE_IDENTITY_PATH);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(std::io::Error::other)
}

fn write_node_key(root: &Path, key: &[u8]) -> std::io::Result<()> {
    let encoded = STANDARD_NO_PAD.encode(key);
    write_text(&root.join(NODE_KEY_PATH), &encoded)
}

fn ensure_witness_signing_key(root: &Path) -> std::io::Result<SigningKey> {
    let path = root.join(WITNESS_KEY_PATH);
    if path.exists() {
        let encoded = fs::read_to_string(path)?;
        let bytes = STANDARD_NO_PAD
            .decode(encoded.trim())
            .map_err(std::io::Error::other)?;
        let key_bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| std::io::Error::other("invalid witness signing key length"))?;
        return Ok(SigningKey::from_bytes(&key_bytes));
    }
    let signing_key = SigningKey::generate(&mut OsRng);
    write_text(&path, &STANDARD_NO_PAD.encode(signing_key.to_bytes()))?;
    Ok(signing_key)
}

fn witness_public_key_b64(signing_key: &SigningKey) -> String {
    STANDARD_NO_PAD.encode(signing_key.verifying_key().to_bytes())
}

fn read_node_key(root: &Path) -> std::io::Result<Vec<u8>> {
    let path = root.join(NODE_KEY_PATH);
    if !path.exists() {
        let _ = ensure_node(root)?;
    }
    let encoded = fs::read_to_string(path)?;
    STANDARD_NO_PAD
        .decode(encoded.trim())
        .map_err(std::io::Error::other)
}

fn read_chain_head(root: &Path) -> std::io::Result<Option<String>> {
    let path = root.join(EVENT_HEAD_PATH);
    if !path.exists() {
        return Ok(None);
    }
    let value = fs::read_to_string(path)?.trim().to_string();
    Ok((!value.is_empty()).then_some(value))
}

fn authority_file_hash(root: &Path, name: &str) -> AuthorityFileHash {
    let path = root.join(name);
    match fs::read(&path) {
        Ok(bytes) => AuthorityFileHash {
            path: name.to_string(),
            sha256: sha256_bytes(&bytes),
            present: true,
        },
        Err(_) => AuthorityFileHash {
            path: name.to_string(),
            sha256: String::new(),
            present: false,
        },
    }
}

fn operating_context() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string());
    format!("{os}/{arch}/{host}")
}

fn write_json_pretty(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    write_text(path, &text)
}

fn write_text(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn append_jsonl(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value).map_err(std::io::Error::other)?;
    file.write_all(b"\n")
}

fn sha256_json(value: &impl Serialize) -> std::io::Result<String> {
    let bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    Ok(sha256_bytes(&bytes))
}

fn expected_event_hash(record: &SignedEventRecord, raw_payload: &str) -> std::io::Result<String> {
    if record.signature_scheme == SIGNATURE_SCHEME_V1 {
        return sha256_json(&EventHashPreimage {
            event_id: &record.event_id,
            event_type: &record.event_type,
            timestamp_secs: record.timestamp_secs,
            node_id: &record.node_id,
            previous_event_hash: &record.previous_event_hash,
            payload_hash: &record.payload_hash,
            signature_scheme: &record.signature_scheme,
        });
    }

    if record.signature_scheme == SIGNATURE_SCHEME_V0 {
        return legacy_event_hash_from_raw_payload(record, raw_payload);
    }

    Err(std::io::Error::other(format!(
        "unsupported signature scheme {}",
        record.signature_scheme
    )))
}

fn legacy_event_hash_from_raw_payload(
    record: &SignedEventRecord,
    raw_payload: &str,
) -> std::io::Result<String> {
    let mut preimage = String::new();
    preimage.push('{');
    push_json_field(&mut preimage, "event_id", &record.event_id)?;
    preimage.push(',');
    push_json_field(&mut preimage, "event_type", &record.event_type)?;
    preimage.push(',');
    push_json_field(&mut preimage, "timestamp_secs", &record.timestamp_secs)?;
    preimage.push(',');
    push_json_field(&mut preimage, "node_id", &record.node_id)?;
    preimage.push(',');
    push_json_field(
        &mut preimage,
        "previous_event_hash",
        &record.previous_event_hash,
    )?;
    preimage.push(',');
    push_json_field(&mut preimage, "payload_hash", &record.payload_hash)?;
    preimage.push(',');
    preimage.push_str("\"payload\":");
    preimage.push_str(raw_payload);
    preimage.push('}');
    Ok(sha256_bytes(preimage.as_bytes()))
}

fn push_json_field(output: &mut String, name: &str, value: &impl Serialize) -> std::io::Result<()> {
    output.push('"');
    output.push_str(name);
    output.push_str("\":");
    output.push_str(&serde_json::to_string(value).map_err(std::io::Error::other)?);
    Ok(())
}

fn sign_json(value: &SigningKey, preimage: &impl Serialize) -> std::io::Result<String> {
    let bytes = serde_json::to_vec(preimage).map_err(std::io::Error::other)?;
    Ok(STANDARD_NO_PAD.encode(value.sign(&bytes).to_bytes()))
}

fn verify_json_signature(
    public_key_b64: &str,
    preimage: &impl Serialize,
    signature_b64: &str,
) -> std::io::Result<()> {
    let public_key_bytes = STANDARD_NO_PAD
        .decode(public_key_b64)
        .map_err(std::io::Error::other)?;
    let public_key_bytes: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| std::io::Error::other("invalid witness public key length"))?;
    let verifying_key =
        VerifyingKey::from_bytes(&public_key_bytes).map_err(std::io::Error::other)?;
    let signature_bytes = STANDARD_NO_PAD
        .decode(signature_b64)
        .map_err(std::io::Error::other)?;
    let signature_bytes: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| std::io::Error::other("invalid witness signature length"))?;
    let signature = Signature::from_bytes(&signature_bytes);
    let bytes = serde_json::to_vec(preimage).map_err(std::io::Error::other)?;
    verifying_key
        .verify(&bytes, &signature)
        .map_err(std::io::Error::other)
}

fn extract_raw_payload(line: &str) -> std::io::Result<String> {
    let marker = "\"payload\":";
    let start = line
        .find(marker)
        .map(|index| index + marker.len())
        .ok_or_else(|| std::io::Error::other("event record missing payload field"))?;
    let bytes = line.as_bytes();
    let mut index = start;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(line[start..=index].to_string());
                }
            }
            _ => {}
        }
        index += 1;
    }
    Err(std::io::Error::other("event record payload was not closed"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    sha256_bytes(bytes)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

fn hmac_hex(key: &[u8], bytes: &[u8]) -> std::io::Result<String> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(std::io::Error::other)?;
    mac.update(bytes);
    Ok(hex_lower(&mac.finalize().into_bytes()))
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_event_log_detects_payload_tampering() {
        let root = unique_temp_dir("buster-events");
        let identity = ensure_node(&root).unwrap();
        assert!(identity.node_id.starts_with("buster-node-"));
        append_signed_event(&root, "test_event", serde_json::json!({"a": 1})).unwrap();
        append_signed_event(&root, "test_event", serde_json::json!({"b": 2})).unwrap();

        let report = verify_event_log(&root).unwrap();
        assert!(report.valid);
        assert_eq!(report.checked_events, 2);

        let path = root.join(EVENT_LOG_PATH);
        let text = fs::read_to_string(&path)
            .unwrap()
            .replace("\"b\":2", "\"b\":3");
        fs::write(&path, text).unwrap();
        let report = verify_event_log(&root).unwrap();
        assert!(!report.valid);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn signed_event_log_round_trips_float_payloads() {
        let root = unique_temp_dir("buster-float-events");
        append_signed_event(
            &root,
            "float_event",
            serde_json::json!({
                "value_score": 0.9079999923706055_f64,
                "summary": "float payloads must not break verification",
            }),
        )
        .unwrap();

        let report = verify_event_log(&root).unwrap();
        assert!(report.valid, "{report:?}");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_event_log_verifies_float_payload_from_raw_text() {
        let root = unique_temp_dir("buster-legacy-float-events");
        ensure_node(&root).unwrap();
        let event = r#"{"event_id":"1780904937-buster-node-b0ffb8d8a05f9b42-b94ad86791e8","event_type":"runtime_cycle","timestamp_secs":1780904937,"node_id":"buster-node-b0ffb8d8a05f9b42","previous_event_hash":"65042727de7a71c474bdbd75bd443a21d072f50950e359a33b5584cf8cafa2c7","payload_hash":"b94ad86791e851d6018c2b741837ff43b7f9df561648fdfc4ef8f4303c72fe3e","payload":{"body_mode":"normal","body_signal_count":0,"execution_record_path":"../audit/research.jsonl","execution_summary":"research LLM call failed: LLM HTTP error 200: error decoding response body","governance_level":2,"reason":"Selected ScientificResearch with value score 0.908; governance level 2.","selected_action_kind":"ScientificResearch","selected_action_summary":"research task seed-aerospace-001: Which aerospace capabilities most directly support long-term civilization resilience and deeper access to the universe?","tick_index":0,"timestamp_secs":1780904906,"value_score":0.9079999923706055,"written_body_paths":[]},"event_hash":"b16f674e6ad35e8a0caf986a26aa3f56c7950cc2d41b270fad5a2ff6ba1b9c16","signature_scheme":"hmac-sha256-local-node-key-v0","signature":"ignored"}"#;
        let record: SignedEventRecord = serde_json::from_str(event).unwrap();
        let raw_payload = extract_raw_payload(event).unwrap();

        assert_eq!(
            expected_event_hash(&record, &raw_payload).unwrap(),
            record.event_hash
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn node_snapshot_can_be_witnessed_and_imported() {
        let subject = unique_temp_dir("buster-witness-subject");
        let witness = unique_temp_dir("buster-witness-observer");
        append_signed_event(
            &subject,
            "subject_event",
            serde_json::json!({"alive": true}),
        )
        .unwrap();

        let snapshot = export_snapshot(&subject).unwrap();
        let receipt = witness_snapshot(&witness, &snapshot).unwrap();
        assert_eq!(receipt.subject_node_id, snapshot.node_identity.node_id);
        assert_eq!(receipt.judgement, "observed");

        import_witness_receipt(&subject, &receipt).unwrap();
        let subject_registry = read_known_nodes(&subject).unwrap();
        assert!(subject_registry
            .nodes
            .iter()
            .any(|node| node.node_id == receipt.witness_node_id));
        let witness_registry = read_known_nodes(&witness).unwrap();
        assert!(witness_registry
            .nodes
            .iter()
            .any(|node| node.node_id == receipt.subject_node_id));

        let _ = fs::remove_dir_all(subject);
        let _ = fs::remove_dir_all(witness);
    }

    #[test]
    fn known_node_public_key_change_is_rejected() {
        let subject = unique_temp_dir("buster-known-subject");
        let impostor = unique_temp_dir("buster-known-impostor");
        let witness = unique_temp_dir("buster-known-witness");

        let subject_snapshot = export_snapshot(&subject).unwrap();
        witness_snapshot(&witness, &subject_snapshot).unwrap();

        let mut impostor_identity = ensure_node(&impostor).unwrap();
        impostor_identity.node_id = subject_snapshot.node_identity.node_id.clone();
        write_json_pretty(&impostor.join(NODE_IDENTITY_PATH), &impostor_identity).unwrap();
        let impostor_snapshot = export_snapshot(&impostor).unwrap();

        let error = witness_snapshot(&witness, &impostor_snapshot).unwrap_err();
        assert!(error.to_string().contains("changed witness public key"));

        let _ = fs::remove_dir_all(subject);
        let _ = fs::remove_dir_all(impostor);
        let _ = fs::remove_dir_all(witness);
    }

    #[test]
    fn body_manifest_hashes_authority_files() {
        let root = unique_temp_dir("buster-manifest");
        fs::write(root.join("self.md"), "# Self\n").unwrap();
        let identity = ensure_node(&root).unwrap();
        let manifest = write_body_manifest(&root, &identity).unwrap();

        let self_hash = manifest
            .protected_authority_files
            .iter()
            .find(|file| file.path == "self.md")
            .unwrap();
        assert!(self_hash.present);
        assert!(!self_hash.sha256.is_empty());

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
