//! Envryn desktop/mobile shell.
//!
//! This crate is deliberately thin. It owns window creation and the IPC
//! surface; every security decision lives in `envryn-core`, which has no
//! dependency on Tauri and can therefore be tested without a windowing system.

mod ipc;

use ipc::VaultState;

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
        .invoke_handler(tauri::generate_handler![
            ipc::vault_status,
            ipc::vault_create,
            ipc::vault_unlock,
            ipc::vault_lock,
            ipc::secret_list,
            ipc::secret_search,
            ipc::secret_reveal,
            ipc::secret_create,
            ipc::secret_update,
            ipc::secret_delete,
            ipc::secret_duplicates,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Envryn");
}
