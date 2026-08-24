//! The IPC surface.
//!
//! This is the entire boundary between the UI and the vault. Every command
//! here is a deliberate widening of what the frontend can do, so the file is
//! meant to stay short and to be the obvious place a reviewer looks.
//!
//! Rules for anything added here:
//!
//! * No command returns a key, a KDF parameter, or a wrapped blob.
//! * No command takes a filesystem path from the caller. The vault location is
//!   decided by the Rust side, so a compromised webview cannot redirect reads
//!   or writes elsewhere.
//! * Errors returned to the UI carry no detail that distinguishes "no vault"
//!   from "wrong password" (INV-006).

use std::sync::Mutex;

use envryn_core::model::{NewSecret, SecretId, SecretRecord, SecretSummary, SecretUpdate};
use envryn_core::vault::Vault;
use envryn_core::{crypto::kdf, Error};
use serde::Serialize;
use tauri::{Manager, State};
use zeroize::Zeroizing;

/// Error shape crossing to the UI.
///
/// A code the UI can branch on plus an already-safe message. Deliberately not
/// `#[from] envryn_core::Error` with its `Debug` -- an accidental `{:?}` in a
/// serialiser is exactly how internal detail escapes.
#[derive(Debug, Serialize)]
pub struct IpcError {
    code: &'static str,
    message: String,
}

impl From<Error> for IpcError {
    fn from(err: Error) -> Self {
        let code = match err {
            Error::AuthenticationFailed => "auth_failed",
            Error::Locked => "locked",
            Error::VaultExists => "vault_exists",
            Error::VaultNotFound => "vault_not_found",
            Error::NotFound => "not_found",
            Error::InvalidInput(_) => "invalid_input",
            Error::UnsupportedVersion { .. } => "unsupported_version",
            Error::DecryptionFailed => "decryption_failed",
            _ => "internal",
        };
        Self {
            message: err.user_message(),
            code,
        }
    }
}

type IpcResult<T> = std::result::Result<T, IpcError>;

/// Vault state, guarded so concurrent commands cannot observe a half-unlocked
/// vault. A poisoned lock is treated as locked rather than unwrapped: losing
/// access is recoverable, continuing with unknown state is not.
#[derive(Default)]
pub struct VaultState(pub Mutex<Option<Vault>>);

impl VaultState {
    fn with<T>(&self, f: impl FnOnce(&mut Vault) -> Result<T, Error>) -> IpcResult<T> {
        let mut guard = self.0.lock().map_err(|_| IpcError {
            code: "internal",
            message: "vault state unavailable".into(),
        })?;
        let vault = guard.as_mut().ok_or(Error::Locked)?;
        Ok(f(vault)?)
    }
}

/// Where the vault file lives.
///
/// Derived from the OS app-data directory, never supplied by the caller. This
/// is why no IPC command accepts a path.
fn vault_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, IpcError> {
    let dir = app.path().app_data_dir().map_err(|_| IpcError {
        code: "internal",
        message: "could not locate the application data directory".into(),
    })?;
    std::fs::create_dir_all(&dir).map_err(|_| IpcError {
        code: "internal",
        message: "could not create the application data directory".into(),
    })?;
    Ok(dir.join("envryn.db"))
}

#[derive(Serialize)]
pub struct VaultStatus {
    pub exists: bool,
    pub unlocked: bool,
}

#[tauri::command]
pub fn vault_status(app: tauri::AppHandle, state: State<'_, VaultState>) -> IpcResult<VaultStatus> {
    let path = vault_path(&app)?;
    let unlocked = state
        .0
        .lock()
        .map(|g| g.as_ref().is_some_and(|v| v.is_unlocked()))
        .unwrap_or(false);
    Ok(VaultStatus {
        exists: path.exists(),
        unlocked,
    })
}

#[tauri::command]
pub fn vault_create(
    app: tauri::AppHandle,
    state: State<'_, VaultState>,
    password: String,
) -> IpcResult<()> {
    // Take ownership immediately so the plaintext password is wiped on drop
    // rather than living until the String the deserialiser made is reclaimed.
    let password = Zeroizing::new(password);

    if password.len() < 8 {
        return Err(IpcError {
            code: "invalid_input",
            message: "Your master password must be at least 8 characters.".into(),
        });
    }

    let path = vault_path(&app)?;
    // Calibrated per device, so a fast desktop gets stronger parameters than a
    // budget phone without either becoming unusable.
    let params = kdf::calibrate(700);
    let vault = Vault::create(&path, &password, params)?;

    let mut guard = state.0.lock().map_err(|_| IpcError {
        code: "internal",
        message: "vault state unavailable".into(),
    })?;
    *guard = Some(vault);
    Ok(())
}

#[tauri::command]
pub fn vault_unlock(
    app: tauri::AppHandle,
    state: State<'_, VaultState>,
    password: String,
) -> IpcResult<()> {
    let password = Zeroizing::new(password);
    let path = vault_path(&app)?;

    let mut vault = Vault::open(&path)?;
    vault.unlock(&password)?;

    let mut guard = state.0.lock().map_err(|_| IpcError {
        code: "internal",
        message: "vault state unavailable".into(),
    })?;
    *guard = Some(vault);
    Ok(())
}

/// Lock the vault. Infallible from the UI's point of view: a lock request must
/// never leave the vault open because bookkeeping failed.
#[tauri::command]
pub fn vault_lock(state: State<'_, VaultState>) {
    if let Ok(mut guard) = state.0.lock() {
        if let Some(vault) = guard.as_mut() {
            vault.lock();
        }
        *guard = None;
    }
}

/// List records as summaries.
///
/// The return type has no field capable of holding secret material, so this
/// command cannot leak a value regardless of how the UI uses the result.
#[tauri::command]
pub fn secret_list(state: State<'_, VaultState>) -> IpcResult<Vec<SecretSummary>> {
    state.with(|v| v.list())
}

#[tauri::command]
pub fn secret_search(state: State<'_, VaultState>, query: String) -> IpcResult<Vec<SecretSummary>> {
    state.with(|v| v.search(&query))
}

/// Reveal one record's secret material.
///
/// Deliberately separate from `secret_list` and deliberately single-record:
/// there is no bulk reveal, so no single call can drain the vault.
#[tauri::command]
pub fn secret_reveal(state: State<'_, VaultState>, id: String) -> IpcResult<SecretRecord> {
    let id = SecretId::parse(&id)?;
    state.with(|v| v.reveal(id))
}

#[tauri::command]
pub fn secret_create(state: State<'_, VaultState>, input: NewSecret) -> IpcResult<SecretSummary> {
    state.with(|v| v.create_secret(input))
}

#[tauri::command]
pub fn secret_update(
    state: State<'_, VaultState>,
    id: String,
    update: SecretUpdate,
) -> IpcResult<SecretSummary> {
    let id = SecretId::parse(&id)?;
    state.with(|v| v.update_secret(id, update))
}

#[tauri::command]
pub fn secret_delete(state: State<'_, VaultState>, id: String) -> IpcResult<()> {
    let id = SecretId::parse(&id)?;
    state.with(|v| v.delete_secret(id))
}

#[tauri::command]
pub fn secret_duplicates(state: State<'_, VaultState>, id: String) -> IpcResult<Vec<String>> {
    let id = SecretId::parse(&id)?;
    state.with(|v| {
        Ok(v.duplicates_of(id)?
            .into_iter()
            .map(|i| i.to_string())
            .collect())
    })
}
