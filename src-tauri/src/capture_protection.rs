//! Screen capture exclusion for the main window.
//!
//! Applied once at startup, unconditionally, rather than as a toggle. There
//! is no accessibility technology in Envryn's threat model that needs to
//! capture the window (a screen reader reads via the accessibility tree, not
//! by capturing pixels), so there is no legitimate reason to offer "off."
//!
//! This stops screenshots, screen recording, and remote-desktop mirroring of
//! the window. It does not stop a camera pointed at the physical monitor --
//! stated plainly here and in docs/THREAT_MODEL.md V-09, not implied away.

use tauri::Manager;

pub fn apply(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    #[cfg(windows)]
    {
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        // A failure here is a lost privacy nicety, not a lost security
        // boundary -- the vault's actual promises (encryption, lock-on-idle)
        // do not depend on it, so startup continues either way.
        if envryn_core::platform::exclude_window_from_capture(hwnd.0 as isize).is_err() {
            eprintln!("envryn: could not enable capture protection for the main window");
        }
    }

    #[cfg(not(windows))]
    {
        let _ = window;
    }
}
