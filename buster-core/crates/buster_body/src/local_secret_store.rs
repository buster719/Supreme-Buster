//! Buster's local encrypted secret store.
//!
//! This is a small body-layer backend for secrets that should not live in
//! `.env`. It encrypts each value before writing it to disk. The master key is
//! still a level-4 concern and should come from a stronger host facility later
//! such as Windows Credential Manager, TPM-backed storage, or IronClaw's
//! keychain-backed master key flow.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine;
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::secrets::{SecretBackend, SecretHandle};

const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalSecretStoreError {
    Io(String),
    Serde(String),
    Crypto(String),
    Decode(String),
    Missing { handle: SecretHandle },
}

impl fmt::Display for LocalSecretStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(reason) => write!(formatter, "secret store io error: {reason}"),
            Self::Serde(reason) => write!(formatter, "secret store format error: {reason}"),
            Self::Crypto(reason) => write!(formatter, "secret store crypto error: {reason}"),
            Self::Decode(reason) => write!(formatter, "secret store decode error: {reason}"),
            Self::Missing { handle } => write!(formatter, "secret `{}` is missing", handle.0),
        }
    }
}

impl std::error::Error for LocalSecretStoreError {}

#[derive(Clone)]
pub struct LocalEncryptedSecretStore {
    path: PathBuf,
    master_key: [u8; 32],
}

impl LocalEncryptedSecretStore {
    pub fn new(path: impl Into<PathBuf>, master_key_material: impl AsRef<[u8]>) -> Self {
        Self {
            path: path.into(),
            master_key: derive_store_key(master_key_material.as_ref()),
        }
    }

    pub fn put_secret_result(
        &self,
        handle: SecretHandle,
        secret: String,
    ) -> Result<(), LocalSecretStoreError> {
        let mut file = self.load_file()?;
        let salt = random_bytes::<SALT_LEN>();
        let nonce = random_bytes::<NONCE_LEN>();
        let key = self.derive_secret_key(&salt)?;
        let cipher = Aes256Gcm::new((&key).into());
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), secret.as_bytes())
            .map_err(|error| LocalSecretStoreError::Crypto(error.to_string()))?;

        file.secrets.insert(
            handle.0,
            EncryptedSecretEntry {
                alg: "AES-256-GCM".to_string(),
                kdf: "HKDF-SHA256".to_string(),
                salt: STANDARD_NO_PAD.encode(salt),
                nonce: STANDARD_NO_PAD.encode(nonce),
                ciphertext: STANDARD_NO_PAD.encode(ciphertext),
            },
        );
        self.write_file(&file)
    }

    pub fn get_secret_result(
        &self,
        handle: &SecretHandle,
    ) -> Result<String, LocalSecretStoreError> {
        let file = self.load_file()?;
        let entry = file
            .secrets
            .get(&handle.0)
            .ok_or_else(|| LocalSecretStoreError::Missing {
                handle: handle.clone(),
            })?;
        let nonce = STANDARD_NO_PAD
            .decode(&entry.nonce)
            .map_err(|error| LocalSecretStoreError::Decode(error.to_string()))?;
        if nonce.len() != NONCE_LEN {
            return Err(LocalSecretStoreError::Decode(format!(
                "nonce must be {NONCE_LEN} bytes"
            )));
        }
        let salt = STANDARD_NO_PAD
            .decode(&entry.salt)
            .map_err(|error| LocalSecretStoreError::Decode(error.to_string()))?;
        if salt.len() != SALT_LEN {
            return Err(LocalSecretStoreError::Decode(format!(
                "salt must be {SALT_LEN} bytes"
            )));
        }
        let ciphertext = STANDARD_NO_PAD
            .decode(&entry.ciphertext)
            .map_err(|error| LocalSecretStoreError::Decode(error.to_string()))?;
        let key = self.derive_secret_key(&salt)?;
        let cipher = Aes256Gcm::new((&key).into());
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|error| LocalSecretStoreError::Crypto(error.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|error| LocalSecretStoreError::Decode(error.to_string()))
    }

    fn load_file(&self) -> Result<SecretStoreFile, LocalSecretStoreError> {
        match fs::read_to_string(&self.path) {
            Ok(raw) => serde_json::from_str(&raw)
                .map_err(|error| LocalSecretStoreError::Serde(error.to_string())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(SecretStoreFile::default())
            }
            Err(error) => Err(LocalSecretStoreError::Io(error.to_string())),
        }
    }

    fn write_file(&self, file: &SecretStoreFile) -> Result<(), LocalSecretStoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| LocalSecretStoreError::Io(error.to_string()))?;
        }
        let raw = serde_json::to_string_pretty(file)
            .map_err(|error| LocalSecretStoreError::Serde(error.to_string()))?;
        fs::write(&self.path, raw).map_err(|error| LocalSecretStoreError::Io(error.to_string()))
    }

    fn derive_secret_key(&self, salt: &[u8]) -> Result<[u8; 32], LocalSecretStoreError> {
        let hk = Hkdf::<Sha256>::new(Some(salt), &self.master_key);
        let mut key = [0u8; 32];
        hk.expand(b"buster-body-local-secret-v1", &mut key)
            .map_err(|_| LocalSecretStoreError::Crypto("HKDF expansion failed".to_string()))?;
        Ok(key)
    }
}

impl SecretBackend for LocalEncryptedSecretStore {
    fn get_secret(&self, handle: &SecretHandle) -> Option<String> {
        self.get_secret_result(handle).ok()
    }

    fn put_secret(&mut self, handle: SecretHandle, secret: String) {
        let _ = self.put_secret_result(handle, secret);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecretStoreFile {
    version: u32,
    secrets: HashMap<String, EncryptedSecretEntry>,
}

impl Default for SecretStoreFile {
    fn default() -> Self {
        Self {
            version: 1,
            secrets: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedSecretEntry {
    alg: String,
    kdf: String,
    salt: String,
    nonce: String,
    ciphertext: String,
}

fn derive_store_key(master_key_material: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"buster-body-local-secret-store-v1");
    hasher.update(master_key_material);
    hasher.finalize().into()
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{SecretBroker, SecretClass, SecretRecord, SecretRegistry};
    use crate::state::BodyMode;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn local_store_encrypts_at_rest_and_feeds_secret_broker() {
        let path = unique_temp_file("buster-local-secret-store");
        let handle = SecretHandle("openrouter_api_key".to_string());
        let mut store = LocalEncryptedSecretStore::new(&path, b"test-master-key");
        store.put_secret(handle.clone(), "sk-local-secret".to_string());

        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("sk-local-secret"));
        assert!(raw.contains("\"kdf\": \"HKDF-SHA256\""));
        assert!(raw.contains("\"salt\""));
        assert_eq!(
            store.get_secret(&handle),
            Some("sk-local-secret".to_string())
        );

        let mut registry = SecretRegistry::new();
        registry.register(SecretRecord::new(
            handle.clone(),
            SecretClass::LlmApiKey,
            vec!["llm.openrouter".to_string()],
            "stored in Buster local encrypted store",
        ));
        let lease = registry
            .issue_lease(
                &handle,
                "llm.openrouter",
                Some(60),
                "test inference",
                BodyMode::Normal,
            )
            .unwrap();
        let mut broker = SecretBroker::new(registry, store);

        let header = broker
            .use_secret_once(
                &lease.lease_id,
                "llm.openrouter",
                BodyMode::Normal,
                |secret| format!("Bearer {secret}"),
            )
            .unwrap();

        assert_eq!(header, "Bearer sk-local-secret");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn wrong_master_key_cannot_decrypt_secret() {
        let path = unique_temp_file("buster-local-secret-store-wrong-key");
        let handle = SecretHandle("openrouter_api_key".to_string());
        let mut store = LocalEncryptedSecretStore::new(&path, b"right-key");
        store.put_secret(handle.clone(), "sk-local-secret".to_string());

        let wrong_store = LocalEncryptedSecretStore::new(&path, b"wrong-key");

        assert!(wrong_store.get_secret_result(&handle).is_err());
        let _ = fs::remove_file(path);
    }

    fn unique_temp_file(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{stamp}.json"))
    }
}
