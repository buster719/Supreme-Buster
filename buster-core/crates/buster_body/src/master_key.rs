//! Master key providers for Buster's local encrypted secret store.
//!
//! In WSL, Buster can keep its Rust runtime in Linux while asking the Windows
//! host body to protect the store master key in Windows Credential Manager.

use std::fmt;
use std::io::Write;
use std::process::{Command, Stdio};

use aes_gcm::aead::OsRng;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use base64::Engine;
use rand::RngCore;

const MASTER_KEY_LEN: usize = 32;
const DEFAULT_WINDOWS_TARGET: &str = "Buster.Body.MasterKey";

pub trait MasterKeyProvider {
    fn get_or_create_master_key(&self) -> Result<Vec<u8>, MasterKeyError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterKeyError {
    MissingEnv { var: String },
    InvalidEnv { var: String, reason: String },
    BridgeUnavailable { reason: String },
    BridgeFailed { reason: String },
    InvalidKeyMaterial { reason: String },
}

impl fmt::Display for MasterKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnv { var } => write!(formatter, "{var} is not set"),
            Self::InvalidEnv { var, reason } => write!(formatter, "{var} is invalid: {reason}"),
            Self::BridgeUnavailable { reason } => write!(formatter, "bridge unavailable: {reason}"),
            Self::BridgeFailed { reason } => write!(formatter, "bridge failed: {reason}"),
            Self::InvalidKeyMaterial { reason } => {
                write!(formatter, "invalid key material: {reason}")
            }
        }
    }
}

impl std::error::Error for MasterKeyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvMasterKeyProvider {
    var: String,
}

impl EnvMasterKeyProvider {
    pub fn new(var: impl Into<String>) -> Self {
        Self { var: var.into() }
    }
}

impl MasterKeyProvider for EnvMasterKeyProvider {
    fn get_or_create_master_key(&self) -> Result<Vec<u8>, MasterKeyError> {
        let raw = std::env::var(&self.var).map_err(|_| MasterKeyError::MissingEnv {
            var: self.var.clone(),
        })?;
        parse_master_key_material(&raw).map_err(|reason| MasterKeyError::InvalidEnv {
            var: self.var.clone(),
            reason,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsCredentialManagerBridge {
    target: String,
    powershell: String,
}

impl WindowsCredentialManagerBridge {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            powershell: "powershell.exe".to_string(),
        }
    }

    pub fn default_buster() -> Self {
        Self::new(DEFAULT_WINDOWS_TARGET)
    }

    pub fn with_powershell(mut self, powershell: impl Into<String>) -> Self {
        self.powershell = powershell.into();
        self
    }

    pub fn get_master_key(&self) -> Result<Option<Vec<u8>>, MasterKeyError> {
        match self.run_bridge("GET\n")? {
            BridgeOutput::Found(raw) => parse_master_key_material(raw.trim())
                .map(Some)
                .map_err(|reason| MasterKeyError::InvalidKeyMaterial { reason }),
            BridgeOutput::Missing => Ok(None),
        }
    }

    pub fn store_master_key(&self, key: &[u8]) -> Result<(), MasterKeyError> {
        validate_master_key(key)?;
        let encoded = STANDARD_NO_PAD.encode(key);
        match self.run_bridge(&format!("PUT {encoded}\n"))? {
            BridgeOutput::Found(_) => Ok(()),
            BridgeOutput::Missing => Err(MasterKeyError::BridgeFailed {
                reason: "PUT unexpectedly returned missing".to_string(),
            }),
        }
    }

    fn run_bridge(&self, input: &str) -> Result<BridgeOutput, MasterKeyError> {
        let mut child = Command::new(&self.powershell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                WINDOWS_CREDENTIAL_BRIDGE_SCRIPT,
            ])
            .env("BUSTER_CRED_TARGET", &self.target)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| MasterKeyError::BridgeUnavailable {
                reason: error.to_string(),
            })?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| MasterKeyError::BridgeFailed {
                reason: "failed to open PowerShell stdin".to_string(),
            })?;
        stdin
            .write_all(format!("{}\n{input}", self.target).as_bytes())
            .map_err(|error| MasterKeyError::BridgeFailed {
                reason: error.to_string(),
            })?;
        drop(stdin);

        let output = child
            .wait_with_output()
            .map_err(|error| MasterKeyError::BridgeFailed {
                reason: error.to_string(),
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if output.status.success() {
            return Ok(BridgeOutput::Found(stdout));
        }
        if output.status.code() == Some(3) {
            return Ok(BridgeOutput::Missing);
        }

        Err(MasterKeyError::BridgeFailed {
            reason: if stderr.is_empty() {
                format!("PowerShell exited with {}", output.status)
            } else {
                stderr
            },
        })
    }
}

impl MasterKeyProvider for WindowsCredentialManagerBridge {
    fn get_or_create_master_key(&self) -> Result<Vec<u8>, MasterKeyError> {
        if let Some(existing) = self.get_master_key()? {
            return Ok(existing);
        }

        let mut key = vec![0u8; MASTER_KEY_LEN];
        OsRng.fill_bytes(&mut key);
        self.store_master_key(&key)?;
        Ok(key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BridgeOutput {
    Found(String),
    Missing,
}

pub fn default_master_key() -> Result<Vec<u8>, MasterKeyError> {
    match EnvMasterKeyProvider::new("BUSTER_SECRETS_MASTER_KEY").get_or_create_master_key() {
        Ok(key) => Ok(key),
        Err(MasterKeyError::MissingEnv { .. }) => {
            WindowsCredentialManagerBridge::default_buster().get_or_create_master_key()
        }
        Err(error) => Err(error),
    }
}

fn parse_master_key_material(raw: &str) -> Result<Vec<u8>, String> {
    if raw.len() == MASTER_KEY_LEN * 2 && raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        let mut bytes = Vec::with_capacity(MASTER_KEY_LEN);
        for chunk in raw.as_bytes().chunks_exact(2) {
            let hex = std::str::from_utf8(chunk).map_err(|error| error.to_string())?;
            bytes.push(u8::from_str_radix(hex, 16).map_err(|error| error.to_string())?);
        }
        validate_master_key(&bytes).map_err(|error| error.to_string())?;
        return Ok(bytes);
    }

    let decoded = STANDARD_NO_PAD
        .decode(raw)
        .or_else(|_| STANDARD.decode(raw))
        .map_err(|error| error.to_string())?;
    validate_master_key(&decoded).map_err(|error| error.to_string())?;
    Ok(decoded)
}

fn validate_master_key(key: &[u8]) -> Result<(), MasterKeyError> {
    if key.len() != MASTER_KEY_LEN {
        return Err(MasterKeyError::InvalidKeyMaterial {
            reason: format!("master key must be {MASTER_KEY_LEN} bytes"),
        });
    }
    Ok(())
}

const WINDOWS_CREDENTIAL_BRIDGE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$rawInput = [Console]::In.ReadToEnd()
$lines = $rawInput -split "`r?`n", 2
$target = $lines[0].Trim()
$inputText = if ($lines.Length -gt 1) { $lines[1].Trim() } else { '' }
if ([string]::IsNullOrWhiteSpace($target)) {
  [Console]::Error.WriteLine('BUSTER_CRED_TARGET is required')
  exit 2
}

Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class BusterCredMan {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  public struct CREDENTIAL {
    public UInt32 Flags;
    public UInt32 Type;
    public string TargetName;
    public string Comment;
    public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
    public UInt32 CredentialBlobSize;
    public IntPtr CredentialBlob;
    public UInt32 Persist;
    public UInt32 AttributeCount;
    public IntPtr Attributes;
    public string TargetAlias;
    public string UserName;
  }

  [DllImport("advapi32.dll", EntryPoint = "CredReadW", CharSet = CharSet.Unicode, SetLastError = true)]
  public static extern bool CredRead(string target, UInt32 type, UInt32 reservedFlag, out IntPtr credentialPtr);

  [DllImport("advapi32.dll", EntryPoint = "CredWriteW", CharSet = CharSet.Unicode, SetLastError = true)]
  public static extern bool CredWrite(ref CREDENTIAL userCredential, UInt32 flags);

  [DllImport("advapi32.dll", SetLastError = true)]
  public static extern void CredFree(IntPtr buffer);
}
'@

function Read-BusterCredential([string]$targetName) {
  $ptr = [IntPtr]::Zero
  $ok = [BusterCredMan]::CredRead($targetName, 1, 0, [ref]$ptr)
  if (-not $ok) {
    $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    if ($err -eq 1168) { exit 3 }
    throw "CredRead failed with Win32 error $err"
  }
  try {
    $cred = [Runtime.InteropServices.Marshal]::PtrToStructure($ptr, [type][BusterCredMan+CREDENTIAL])
    $bytes = New-Object byte[] $cred.CredentialBlobSize
    [Runtime.InteropServices.Marshal]::Copy($cred.CredentialBlob, $bytes, 0, $bytes.Length)
    [Text.Encoding]::Unicode.GetString($bytes).TrimEnd([char]0)
  } finally {
    [BusterCredMan]::CredFree($ptr)
  }
}

function Write-BusterCredential([string]$targetName, [string]$secretText) {
  $bytes = [Text.Encoding]::Unicode.GetBytes($secretText)
  $blob = [Runtime.InteropServices.Marshal]::AllocCoTaskMem($bytes.Length)
  try {
    [Runtime.InteropServices.Marshal]::Copy($bytes, 0, $blob, $bytes.Length)
    $cred = New-Object BusterCredMan+CREDENTIAL
    $cred.Flags = 0
    $cred.Type = 1
    $cred.TargetName = $targetName
    $cred.Comment = 'Buster body local secret store master key'
    $cred.CredentialBlobSize = $bytes.Length
    $cred.CredentialBlob = $blob
    $cred.Persist = 2
    $cred.AttributeCount = 0
    $cred.Attributes = [IntPtr]::Zero
    $cred.TargetAlias = $null
    $cred.UserName = 'buster'
    $ok = [BusterCredMan]::CredWrite([ref]$cred, 0)
    if (-not $ok) {
      $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
      throw "CredWrite failed with Win32 error $err"
    }
  } finally {
    [Runtime.InteropServices.Marshal]::FreeCoTaskMem($blob)
  }
}

if ($inputText -eq 'GET') {
  Read-BusterCredential $target
  exit 0
}
if ($inputText.StartsWith('PUT ')) {
  $secretText = $inputText.Substring(4).Trim()
  if ([string]::IsNullOrWhiteSpace($secretText)) { throw 'empty key material' }
  Write-BusterCredential $target $secretText
  'OK'
  exit 0
}
throw 'expected GET or PUT'
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_provider_accepts_hex_key() {
        let var = "BUSTER_TEST_MASTER_KEY_HEX";
        std::env::set_var(var, "a".repeat(64));

        let key = EnvMasterKeyProvider::new(var)
            .get_or_create_master_key()
            .unwrap();

        assert_eq!(key.len(), MASTER_KEY_LEN);
        std::env::remove_var(var);
    }

    #[test]
    fn env_provider_rejects_short_key() {
        let var = "BUSTER_TEST_MASTER_KEY_SHORT";
        std::env::set_var(var, "too-short");

        let error = EnvMasterKeyProvider::new(var)
            .get_or_create_master_key()
            .unwrap_err();

        assert!(matches!(error, MasterKeyError::InvalidEnv { .. }));
        std::env::remove_var(var);
    }
}
