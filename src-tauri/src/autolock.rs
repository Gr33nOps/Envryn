//! Idle-timeout auto-lock.
//!
//! Polls system-wide idle time (`envryn_core::platform::idle_seconds`) every
//! few seconds rather than reacting to a Windows session-lock event
//! (`WTS_SESSION_LOCK`). This is a deliberate scope choice, not an oversight:
//! the idle poll covers the common case -- the user walked away -- with a
//! fraction of the implementation cost of a native window-message hook, and
//! it works identically whether or not the OS session itself ever locks.
//! Reacting to the session-lock event directly is recorded as still open in
//! docs/ARCHITECTURE.md.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::ipc::VaultState;
use crate::settings;

const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Start the background poll. Runs for the lifetime of the application; there
/// is exactly one of these per process, spawned once from `.setup()`.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            tick(&app);
        }
    });
}

fn tick(app: &AppHandle) {
    let Ok(idle_seconds) = envryn_core::platform::idle_seconds() else {
        // No implementation on this platform, or the OS call failed. Neither
        // is a reason to stop polling -- just nothing to act on this tick.
        return;
    };

    let threshold_seconds = u64::from(settings::load(app).auto_lock_minutes) * 60;
    if idle_seconds < threshold_seconds {
        return;
    }

    let Some(state) = app.try_state::<VaultState>() else {
        return;
    };
    // A poisoned mutex means some other command panicked mid-operation.
    // Force-recovering it here to keep auto-lock running would mean acting on
    // state a panic already proved was in question -- skip this tick and
    // retry in five seconds instead, matching how every IPC command in
    // ipc.rs already treats poisoning as "can't proceed," never as
    // "proceed anyway."
    let Ok(mut guard) = state.0.lock() else {
        return;
    };

    let was_unlocked = guard.as_ref().is_some_and(envryn_core::Vault::is_unlocked);
    if !was_unlocked {
        return;
    }

    if let Some(vault) = guard.as_mut() {
        vault.lock();
    }
    *guard = None;
    drop(guard);

    let _ = app.emit("vault-locked", ());
}
