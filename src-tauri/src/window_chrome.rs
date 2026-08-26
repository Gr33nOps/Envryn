//! Cosmetic window-chrome setup for the main window.
//!
//! Applied once at startup, alongside `capture_protection::apply`. The
//! frameless window (`decorations: false` in `tauri.conf.json`, replaced by
//! `TitleBar.tsx`'s custom chrome) leaves one thing the webview cannot
//! reach: Windows 11 draws its own 1px border around every top-level
//! window, decorated or not, in a system/theme colour that has no idea
//! Envryn's background is near-black -- it shows up as a mismatched pale
//! line around otherwise fully custom chrome. `DwmSetWindowAttribute` is
//! the only way to tell DWM what colour that border should actually be.

use tauri::Manager;

pub fn apply(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    #[cfg(windows)]
    {
        /// Envryn's `--background` design token (`apps/ui/src/styles.css`),
        /// `oklch(0.135 0.004 155)` converted to sRGB -- computed
        /// independently with the standard CSS Color 4 OKLab matrices, not
        /// eyeballed, so this border matches the real rendered background
        /// rather than an approximation of it. Declared inside this `#[cfg]`
        /// block, not at module scope, because it is meaningless on any
        /// platform other than Windows (this file also builds for Android).
        const BACKGROUND_RGB_HEX: u32 = 0x07_09_08;

        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        // Cosmetic only -- a failure here (e.g. Windows 10, which has no
        // such border to colour) leaves the window fully usable, just with
        // whatever default border that Windows version would have drawn
        // anyway.
        if envryn_core::platform::set_window_border_color(hwnd.0 as isize, BACKGROUND_RGB_HEX)
            .is_err()
        {
            eprintln!("envryn: could not set the main window's border colour");
        }
    }

    #[cfg(not(windows))]
    {
        let _ = window;
    }
}
