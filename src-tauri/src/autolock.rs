//! Auto-lock: two independent triggers converging on the same lock sequence.
//!
//! The idle poll (`spawn`/`tick`) polls system-wide idle time
//! (`envryn_core::platform::idle_seconds`) every few seconds -- it covers the
//! common case (the user walked away) cheaply and works identically whether
//! or not the OS session itself ever locks. `watch_session_lock` closes the
//! gap that poll alone leaves: on Windows, it reacts to `WM_WTSSESSION_CHANGE`
//! / `WTS_SESSION_LOCK` directly (`Win+L`, the screen saver locking, a remote
//! session disconnecting), so the vault locks the instant the OS session
//! does rather than waiting for the next idle-poll tick to notice. Both
//! triggers call the same `lock_now`, so there is exactly one lock sequence
//! to keep correct.

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

    lock_now(app);
}

/// Lock the vault, stop the AI worker, and notify the frontend -- the one
/// lock sequence both `tick` (idle poll) and `watch_session_lock` (direct
/// Windows session-lock event) trigger. A no-op if the vault is already
/// locked, so either trigger can fire redundantly (both an idle timeout and
/// a session lock happening close together, say) without double-emitting.
fn lock_now(app: &AppHandle) {
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

    if let Some(ai_state) = app.try_state::<crate::ai::AiState>() {
        crate::ai::stop(&ai_state);
    }

    let _ = app.emit("vault-locked", ());
}

/// Install the direct Windows session-lock hook alongside the idle poll --
/// closes the item `docs/ARCHITECTURE.md` section 7 previously recorded as
/// open. Best-effort: if the main window isn't available yet, this isn't
/// Windows, or the OS call itself fails, the idle poll remains the only lock
/// trigger -- the same coverage this app already shipped with, so failing
/// here costs a nicety, not a security boundary.
pub fn watch_session_lock(app: AppHandle) {
    #[cfg(windows)]
    {
        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        let hwnd = hwnd.0 as isize;
        let result = envryn_core::platform::watch_session_lock(hwnd, move || {
            lock_now(&app);
        });
        if result.is_err() {
            eprintln!(
                "envryn: could not register for Windows session-lock notifications; \
                 idle-based auto-lock remains active"
            );
        }
    }
    #[cfg(not(windows))]
    {
        let _ = app;
    }
}
