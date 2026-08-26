//! Platform-specific, windowless OS integration.
//!
//! Everything here is a thin, mechanical wrapper around one OS call each --
//! DPAPI protection, clipboard exclusion, idle-time query, capture exclusion.
//! None of it makes a security *decision*; [`crate::vault`] and the Tauri
//! shell compose these primitives into one. See docs/ARCHITECTURE.md section 7
//! for the platform matrix and what remains open (Android has no
//! implementation yet -- see the `stub` module below).
//!
//! `exclude_window_from_capture` takes a raw `isize` rather than a window
//! type, so this crate does not need to depend on Tauri or any windowing
//! library merely to flip one flag on a window it did not create.

#[cfg(windows)]
mod windows_impl;
#[cfg(windows)]
pub use windows_impl::*;

#[cfg(windows)]
mod hello;
#[cfg(windows)]
pub use hello::{hello_enroll, hello_forget, hello_supported, hello_verify};

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::*;

/// Whether DPAPI-equivalent platform key protection exists on this OS.
pub fn dpapi_available() -> bool {
    cfg!(windows)
}

/// Clear the clipboard only if it still holds exactly `expected` -- never
/// destroys something the user copied since Envryn last wrote to it.
///
/// This is the actual safety property behind Envryn's clipboard-clear timer
/// (`src-tauri/src/ipc.rs`'s `clipboard_copy`), pulled out here specifically
/// so it has a real test against the real OS clipboard. The IPC command
/// itself takes a `tauri::AppHandle`, which cannot be exercised under
/// `tauri::test::MockRuntime` (the same structural gap already documented
/// for the other `AppHandle`-taking commands) -- this function carries the
/// one part of that command's logic that is a real security property worth
/// testing directly, independent of that gap.
pub fn clear_clipboard_if_matches(expected: &str) -> crate::error::Result<()> {
    if let Ok(Some(current)) = read_clipboard_text() {
        if current == expected {
            clear_clipboard()?;
        }
    }
    Ok(())
}

/// The real OS clipboard is one global, exclusively-locked resource, but
/// `cargo test` runs tests in parallel by default -- so every test in this
/// crate that touches the real clipboard (here and in `windows_impl`'s own
/// `clipboard_round_trip`) must serialize against this same lock, or they
/// race each other for `OpenClipboard` and fail with spurious
/// "clipboard unavailable" errors that have nothing to do with the code
/// under test. `windows_impl::tests::clipboard_round_trip` takes this lock
/// too.
#[cfg(test)]
pub(crate) fn clipboard_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(all(test, windows))]
mod clipboard_tests {
    use super::*;

    #[test]
    fn clears_the_clipboard_when_it_still_holds_the_expected_value() {
        let _guard = clipboard_test_lock();
        set_clipboard_text_excluded("adversarial-clipboard-value-1").unwrap();
        assert_eq!(
            read_clipboard_text().unwrap().as_deref(),
            Some("adversarial-clipboard-value-1")
        );

        clear_clipboard_if_matches("adversarial-clipboard-value-1").unwrap();

        assert_ne!(
            read_clipboard_text().unwrap().as_deref(),
            Some("adversarial-clipboard-value-1"),
            "clipboard still holds the secret after clear_clipboard_if_matches"
        );
    }

    #[test]
    fn does_not_touch_the_clipboard_when_the_user_copied_something_else_since() {
        let _guard = clipboard_test_lock();
        set_clipboard_text_excluded("original-secret-value").unwrap();

        // The user copies something unrelated before the clear timer fires.
        set_clipboard_text_excluded("something-the-user-copied-afterward").unwrap();

        // The stale timer for the *original* value must not destroy this.
        clear_clipboard_if_matches("original-secret-value").unwrap();

        assert_eq!(
            read_clipboard_text().unwrap().as_deref(),
            Some("something-the-user-copied-afterward"),
            "clearing a stale timer destroyed clipboard content the user copied since"
        );
    }
}
