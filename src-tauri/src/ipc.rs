//! The IPC surface.
//!
//! This is the entire boundary between the UI and the vault. Every command
//! here is a deliberate widening of what the frontend can do, so the file is
//! meant to stay short and to be the obvious place a reviewer looks.
//!
//! Rules for anything added here:
//!
//! * No command returns a key, a KDF parameter, or a wrapped blob.
//! * No command takes a filesystem path that redirects *the vault's own*
//!   storage location -- that is always derived from the OS app-data
//!   directory, never supplied by the caller, so a compromised webview cannot
//!   redirect vault reads or writes elsewhere. `backup_create` and
//!   `backup_restore` are the one place a path *does* cross this boundary,
//!   and it names a destination or source for an explicit user-chosen
//!   export/import file, not the vault -- the same distinction a native
//!   "save as" dialog would draw.
//! * Errors returned to the UI carry no detail that distinguishes "no vault"
//!   from "wrong password" (INV-006).
//! * There is no bulk reveal: `secret_list` returns summaries that cannot
//!   hold a value, and `secret_reveal` is single-record. `backup_create` is
//!   the one deliberate exception -- a backup is a bulk export by definition
//!   -- and it grants no capability an attacker with unlock access does not
//!   already have via repeated `secret_reveal` calls; see
//!   `envryn_core::vault::Vault::export_all`.

use std::sync::Mutex;

use envryn_core::model::{
    NewSecret, SecretId, SecretRecord, SecretSummary, SecretUpdate, VaultProject,
};
use envryn_core::vault::Vault;
use envryn_core::{crypto::kdf, Error};
use serde::Serialize;
use tauri::{Manager, State};
use zeroize::Zeroizing;

use crate::settings;

#[cfg(target_os = "android")]
use envryn_android_clipboard::SensitiveClipboardExt;

/// Error shape crossing to the UI.
///
/// A code the UI can branch on plus an already-safe message. Deliberately not
/// `#[from] envryn_core::Error` with its `Debug` -- an accidental `{:?}` in a
/// serialiser is exactly how internal detail escapes.
#[derive(Debug, Serialize)]
pub struct IpcError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
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
            Error::PlatformUnavailable => "platform_unavailable",
            _ => "internal",
        };
        Self {
            message: err.user_message(),
            code,
        }
    }
}

pub(crate) type IpcResult<T> = std::result::Result<T, IpcError>;

pub(crate) fn internal(message: &str) -> IpcError {
    IpcError {
        code: "internal",
        message: message.to_string(),
    }
}

pub(crate) fn invalid(message: &str) -> IpcError {
    IpcError {
        code: "invalid_input",
        message: message.to_string(),
    }
}

/// Vault state, guarded so concurrent commands cannot observe a half-unlocked
/// vault. A poisoned lock is treated as locked rather than unwrapped: losing
/// access is recoverable, continuing with unknown state is not.
#[derive(Default)]
pub struct VaultState(pub Mutex<Option<Vault>>);

impl VaultState {
    pub(crate) fn with<T>(&self, f: impl FnOnce(&mut Vault) -> Result<T, Error>) -> IpcResult<T> {
        let mut guard = self
            .0
            .lock()
            .map_err(|_| internal("vault state unavailable"))?;
        let vault = guard.as_mut().ok_or(Error::Locked)?;
        Ok(f(vault)?)
    }

    pub(crate) fn install(&self, vault: Vault) -> IpcResult<()> {
        let mut guard = self
            .0
            .lock()
            .map_err(|_| internal("vault state unavailable"))?;
        *guard = Some(vault);
        Ok(())
    }
}

/// Where the vault file lives.
///
/// Derived from the OS app-data directory, never supplied by the caller. This
/// is why no IPC command accepts a path for this -- see the module docs.
pub(crate) fn vault_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, IpcError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| internal("could not locate the application data directory"))?;
    std::fs::create_dir_all(&dir)
        .map_err(|_| internal("could not create the application data directory"))?;
    Ok(dir.join("envryn.db"))
}

#[derive(Serialize, ts_rs::TS)]
#[ts(export)]
pub struct VaultStatus {
    pub exists: bool,
    pub unlocked: bool,
    /// Whether this OS build supports a platform key at all -- used to decide
    /// whether the settings UI offers the option.
    pub platform_protection_available: bool,
    /// Whether *this* vault currently has the platform slot set up. Readable
    /// while locked, since it decides whether the unlock screen should offer
    /// "unlock with this Windows account" before anything has been typed.
    pub platform_protection_enabled: bool,
    /// Whether Windows Hello for Apps is available on this machine
    /// (hardware plus an enrolled biometric/PIN) -- used to decide whether
    /// Settings offers the Hello-gate option at all. Not the same question
    /// as `platform_protection_available`: a machine can have DPAPI (every
    /// Windows install does) without having Windows Hello enrolled.
    pub hello_gate_available: bool,
    /// Whether *this* vault currently requires the Windows Hello gate before
    /// its platform-slot unlock. See `platform::hello`'s module doc for what
    /// this does and does not cryptographically guarantee.
    pub hello_gate_enabled: bool,
}

#[tauri::command]
pub fn vault_status(app: tauri::AppHandle, state: State<'_, VaultState>) -> IpcResult<VaultStatus> {
    let path = vault_path(&app)?;
    let exists = path.exists();
    let unlocked = state
        .0
        .lock()
        .map(|g| g.as_ref().is_some_and(Vault::is_unlocked))
        .unwrap_or(false);

    let platform_protection_enabled = exists
        && Vault::open(&path)
            .and_then(|v| v.platform_protection_enabled())
            .unwrap_or(false);
    let hello_gate_enabled = exists
        && Vault::open(&path)
            .and_then(|v| v.hello_gate_enabled())
            .unwrap_or(false);

    Ok(VaultStatus {
        exists,
        unlocked,
        platform_protection_available: envryn_core::platform::dpapi_available(),
        platform_protection_enabled,
        hello_gate_available: envryn_core::platform::hello_supported(),
        hello_gate_enabled,
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
        return Err(invalid(
            "Your master password must be at least 8 characters.",
        ));
    }

    let path = vault_path(&app)?;
    // Calibrated per device, so a fast desktop gets stronger parameters than a
    // budget phone without either becoming unusable.
    let params = kdf::calibrate(700);
    let mut vault = Vault::create(&path, &password, params)?;
    vault.set_local_device_id(crate::sync::local_device_id(&app)?)?;
    state.install(vault)
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
    vault.set_local_device_id(crate::sync::local_device_id(&app)?)?;
    state.install(vault)
}

/// Unlock using the platform slot instead of the master password. Only
/// reachable when `vault_status` already reported the slot enabled.
///
/// If this vault has the Windows Hello gate enabled, `platform::hello_verify`
/// runs first and must succeed (a real OS biometric/PIN prompt) before the
/// DPAPI unwrap is even attempted -- see `platform::hello`'s module doc for
/// what that gate does and does not guarantee.
#[tauri::command]
pub fn vault_unlock_with_platform(
    app: tauri::AppHandle,
    state: State<'_, VaultState>,
) -> IpcResult<()> {
    let path = vault_path(&app)?;
    let mut vault = Vault::open(&path)?;
    if vault.hello_gate_enabled()? {
        envryn_core::platform::hello_verify()?;
    }
    vault.unlock_with_platform()?;
    vault.set_local_device_id(crate::sync::local_device_id(&app)?)?;
    state.install(vault)
}

/// Lock the vault. Infallible from the UI's point of view: a lock request must
/// never leave the vault open because bookkeeping failed.
#[tauri::command]
pub fn vault_lock(state: State<'_, VaultState>, ai_state: State<'_, crate::ai::AiState>) {
    if let Ok(mut guard) = state.0.lock() {
        if let Some(vault) = guard.as_mut() {
            vault.lock();
        }
        *guard = None;
    }
    // Killing the AI worker on lock, not just clearing its context, is the
    // only thing docs/AI_SECURITY.md section 3 trusts to remove whatever
    // plaintext was in its inference buffers.
    crate::ai::stop(&ai_state);
}

/// Enable the platform slot: unlock without the master password, tied to the
/// current Windows user account. Requires the current master password again
/// -- enabling an alternate route into the vault deserves the same friction
/// as changing the primary one.
#[tauri::command]
pub fn vault_enable_platform_protection(
    state: State<'_, VaultState>,
    password: String,
) -> IpcResult<()> {
    let password = Zeroizing::new(password);
    state.with(|v| v.enable_platform_protection(&password))
}

/// Disable the platform slot. Never touches the password slot, so this can
/// never be the operation that locks someone out (INV-007).
#[tauri::command]
pub fn vault_disable_platform_protection(state: State<'_, VaultState>) -> IpcResult<()> {
    state.with(Vault::disable_platform_protection)
}

/// Turn on the Windows Hello gate in front of the platform slot. Requires
/// the platform slot to already be enabled, and triggers the OS Windows
/// Hello enrollment/consent UI (`platform::hello_enroll`). See
/// `platform::hello`'s module doc for exactly what this does and does not
/// guarantee -- it is a UX/authentication gate, not a stronger key wrap.
#[tauri::command]
pub fn vault_enable_hello_gate(state: State<'_, VaultState>) -> IpcResult<()> {
    state.with(Vault::enable_hello_gate)
}

/// Turn off the Windows Hello gate. The platform slot itself is untouched.
#[tauri::command]
pub fn vault_disable_hello_gate(state: State<'_, VaultState>) -> IpcResult<()> {
    state.with(Vault::disable_hello_gate)
}

/// Change the master password. Requires the current one; see
/// `Vault::change_password` for why this rewraps the VMK rather than
/// re-encrypting every record.
#[tauri::command]
pub fn vault_change_password(
    state: State<'_, VaultState>,
    current_password: String,
    new_password: String,
) -> IpcResult<()> {
    let new_password = Zeroizing::new(new_password);
    if new_password.len() < 8 {
        return Err(invalid(
            "Your new master password must be at least 8 characters.",
        ));
    }
    let current_password = Zeroizing::new(current_password);
    let params = kdf::calibrate(700);
    state.with(|v| v.change_password(&current_password, &new_password, params))
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

#[tauri::command]
pub fn project_list(state: State<'_, VaultState>) -> IpcResult<Vec<VaultProject>> {
    state.with(|vault| vault.list_projects())
}

#[tauri::command]
pub fn project_create(state: State<'_, VaultState>, name: String) -> IpcResult<VaultProject> {
    state.with(|vault| vault.create_project(&name))
}

#[tauri::command]
pub fn project_rename(
    state: State<'_, VaultState>,
    id: String,
    name: String,
) -> IpcResult<VaultProject> {
    state.with(|vault| vault.rename_project(&id, &name))
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

/// Preserved sync conflicts for one record (INV-109) -- the losing side(s)
/// of a genuine concurrent edit, kept rather than silently discarded. The
/// live value returned by `secret_reveal`/`secret_list` already reflects
/// whichever side won the original tiebreak.
#[tauri::command]
pub fn secret_conflicts(
    state: State<'_, VaultState>,
    id: String,
) -> IpcResult<Vec<envryn_core::vault::ConflictSummary>> {
    let id = SecretId::parse(&id)?;
    state.with(|v| v.list_conflicts(id))
}

/// Total preserved conflicts across the vault, for a summary badge.
#[tauri::command]
pub fn conflict_count(state: State<'_, VaultState>) -> IpcResult<i64> {
    state.with(|v| v.count_conflicts())
}

/// Every preserved conflict across the vault, for a review screen -- unlike
/// `secret_conflicts`, not scoped to a caller-supplied id.
#[tauri::command]
pub fn conflict_list_all(
    state: State<'_, VaultState>,
) -> IpcResult<Vec<envryn_core::vault::ConflictSummary>> {
    state.with(|v| v.list_all_conflicts())
}

/// Keep a preserved conflict as a brand-new record.
#[tauri::command]
pub fn conflict_recover(
    state: State<'_, VaultState>,
    conflict_id: String,
) -> IpcResult<SecretSummary> {
    state.with(|v| v.recover_conflict(&conflict_id))
}

/// Discard a preserved conflict -- the user reviewed it and does not want to
/// keep the losing side.
#[tauri::command]
pub fn conflict_discard(state: State<'_, VaultState>, conflict_id: String) -> IpcResult<()> {
    state.with(|v| v.discard_conflict(&conflict_id))
}

/// Copy a value to the clipboard, tagged to skip clipboard history and cloud
/// sync, and scheduled to clear itself after the configured delay.
///
/// The value crosses the IPC boundary once, here, and is never returned to
/// the UI again -- the caller already has it (this is always called
/// immediately after a `secret_reveal`), so there is nothing to hand back.
#[tauri::command]
pub fn clipboard_copy(app: tauri::AppHandle, value: String) -> IpcResult<()> {
    #[cfg(windows)]
    envryn_core::platform::set_clipboard_text_excluded(&value)?;

    #[cfg(target_os = "android")]
    app.sensitive_clipboard()
        .write_sensitive_text(value.clone())
        .map_err(|_| internal("Clipboard is unavailable."))?;

    #[cfg(not(any(windows, target_os = "android")))]
    return Err(Error::PlatformUnavailable.into());

    let clear_after =
        std::time::Duration::from_secs(u64::from(settings::load(&app).clipboard_clear_seconds));

    // Fire-and-forget: the command returns immediately, and a failure to
    // clear later has no meaningful way to report back to a call that has
    // already completed. The "only clear if the clipboard still holds
    // exactly what we put there" safety property lives in
    // `envryn_core::platform::clear_clipboard_if_matches`, which has a real
    // test against the real OS clipboard -- this command's own `AppHandle`
    // parameter keeps it out of `tauri::test::MockRuntime`'s reach, so the
    // logic that actually matters is tested one layer down instead.
    #[cfg(target_os = "android")]
    let clear_app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(clear_after).await;
        #[cfg(windows)]
        let _ = envryn_core::platform::clear_clipboard_if_matches(&value);

        #[cfg(target_os = "android")]
        if clear_app
            .sensitive_clipboard()
            .read_text()
            .is_ok_and(|current| current == value)
        {
            let _ = clear_app.sensitive_clipboard().clear();
        }
    });

    Ok(())
}

#[derive(Serialize, ts_rs::TS)]
#[ts(export)]
pub struct RestoreSummary {
    pub restored: usize,
}

/// Write an encrypted backup of every record to `path`.
///
/// `path` is a user-chosen export destination, not the vault's own storage
/// location -- see the module docs' note on this being the one deliberate
/// exception to "no path from the caller."
#[tauri::command]
pub fn backup_create(
    state: State<'_, VaultState>,
    path: String,
    password: String,
) -> IpcResult<()> {
    if path.trim().is_empty() {
        return Err(invalid("Choose a location to save the backup."));
    }
    let password = Zeroizing::new(password);
    if password.len() < 8 {
        return Err(invalid(
            "Your backup password must be at least 8 characters.",
        ));
    }
    let records = state.with(|v| v.export_all())?;
    let file = envryn_core::backup::create(&records, &password)?;
    std::fs::write(&path, file)
        .map_err(|_| internal("Could not write the backup file. Check the location and try again."))
}

/// Restore a backup, replacing the current vault.
///
/// The existing vault file (and its WAL/SHM sidecars, if present) is renamed
/// aside with a timestamp rather than deleted, so a mistaken restore is still
/// recoverable. Restoring always sets a *new* master password -- see
/// `envryn_core::backup` for why a backup never carries the original one.
#[tauri::command]
pub fn backup_restore(
    app: tauri::AppHandle,
    state: State<'_, VaultState>,
    path: String,
    backup_password: String,
    new_master_password: String,
) -> IpcResult<RestoreSummary> {
    let new_master_password = Zeroizing::new(new_master_password);
    if new_master_password.len() < 8 {
        return Err(invalid(
            "Your new master password must be at least 8 characters.",
        ));
    }

    let bytes = std::fs::read(&path).map_err(|_| invalid("Could not read that backup file."))?;
    let backup_password = Zeroizing::new(backup_password);
    let records = envryn_core::backup::restore(&bytes, &backup_password)?;

    let vault_path = vault_path(&app)?;
    if vault_path.exists() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        for suffix in ["", "-wal", "-shm"] {
            let src = vault_path.with_file_name(format!("envryn.db{suffix}"));
            if src.exists() {
                let dst =
                    vault_path.with_file_name(format!("envryn.db{suffix}.pre-restore-{stamp}"));
                let _ = std::fs::rename(&src, &dst);
            }
        }
    }

    let params = kdf::calibrate(700);
    let mut vault = Vault::create(&vault_path, &new_master_password, params)?;
    for record in &records {
        vault.import_record(record.clone())?;
    }

    let restored = records.len();
    state.install(vault)?;
    Ok(RestoreSummary { restored })
}
