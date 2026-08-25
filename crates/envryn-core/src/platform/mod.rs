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
