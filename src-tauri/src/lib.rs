//! Envryn desktop/mobile shell.
//!
//! This crate is deliberately thin. It owns window creation and the IPC
//! surface; every security decision lives in `envryn-core`, which has no
//! dependency on Tauri and can therefore be tested without a windowing system.

// `ai`/`ipc`/`settings`/`sync` are `pub` (rather than the private `mod` an
// application-only crate would otherwise use) solely so `tests/ipc_mock.rs`
// -- a real Cargo integration test target -- can reach their `#[tauri::command]`
// functions and state types to dispatch them through `tauri::test`'s real IPC
// path. This crate is `publish = false` and has no external consumers, so the
// wider surface carries no real cost; nothing about the command boundary
// itself changes; see `tests/ipc_mock.rs`'s module doc for why the tests live
// there instead of an internal `#[cfg(test)] mod tests`.
pub mod ai;
mod autolock;
mod capture_protection;
pub mod ipc;
pub mod settings;
pub mod sync;
mod window_chrome;

use ai::AiState;
use ipc::VaultState;
use sync::{PairingState, SyncListenState};

/// Start the application.
///
/// This is the one place in Envryn permitted to panic. If the webview or window
/// cannot be created there is no UI to report the failure into, and continuing
/// would leave a process running with no way to interact with it. Everywhere
/// else -- and especially in any lock path -- panicking is forbidden.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(clippy::expect_used)]
pub fn run() {
    tauri::Builder::default()
        .manage(VaultState::default())
        .manage(PairingState::default())
        .manage(SyncListenState::default())
        .manage(AiState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            capture_protection::apply(&handle);
            window_chrome::apply(&handle);
            autolock::watch_session_lock(handle.clone());
            autolock::spawn(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::vault_status,
            ipc::vault_create,
            ipc::vault_unlock,
            ipc::vault_unlock_with_platform,
            ipc::vault_lock,
            ipc::vault_enable_platform_protection,
            ipc::vault_disable_platform_protection,
            ipc::vault_enable_hello_gate,
            ipc::vault_disable_hello_gate,
            ipc::vault_change_password,
            ipc::secret_list,
            ipc::secret_search,
            ipc::secret_reveal,
            ipc::secret_create,
            ipc::secret_update,
            ipc::secret_delete,
            ipc::secret_duplicates,
            ipc::secret_conflicts,
            ipc::conflict_count,
            ipc::conflict_list_all,
            ipc::conflict_recover,
            ipc::conflict_discard,
            ipc::clipboard_copy,
            ipc::backup_create,
            ipc::backup_restore,
            settings::settings_get,
            settings::settings_set,
            sync::device_identity,
            sync::trusted_device_list,
            sync::trusted_device_rename,
            sync::trusted_device_revoke,
            sync::discovery_browse,
            sync::sync_now,
            sync::sync_listen_start,
            sync::sync_listen_stop,
            sync::pairing_host_start,
            sync::pairing_join_start,
            sync::pairing_confirm,
            sync::pairing_cancel,
            ai::classify_deterministic,
            ai::ai_status,
            ai::ai_download_model,
            ai::ai_start,
            ai::ai_stop,
            ai::ai_classify_pasted_value,
            ai::ai_suggest_name,
            ai::ai_classify_env_names,
            ai::ai_extract_structured_fields,
            ai::ai_parse_search_intent,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Envryn");
}
