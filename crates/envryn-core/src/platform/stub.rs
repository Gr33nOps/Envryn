//! Non-Windows placeholder.
//!
//! Android is explicitly out of scope for the platform features implemented
//! so far (spec section 52; docs/ARCHITECTURE.md section 7). Every function
//! here fails cleanly rather than silently no-op-ing, so a caller cannot
//! mistake "not implemented" for "succeeded."

use zeroize::Zeroizing;

use crate::error::{Error, Result};

pub fn dpapi_protect(_secret: &[u8]) -> Result<Vec<u8>> {
    Err(Error::PlatformUnavailable)
}

pub fn dpapi_unprotect(_blob: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    Err(Error::PlatformUnavailable)
}

pub fn idle_seconds() -> Result<u64> {
    Err(Error::PlatformUnavailable)
}

pub fn set_clipboard_text_excluded(_text: &str) -> Result<()> {
    Err(Error::PlatformUnavailable)
}

pub fn read_clipboard_text() -> Result<Option<String>> {
    Err(Error::PlatformUnavailable)
}

pub fn clear_clipboard() -> Result<()> {
    Err(Error::PlatformUnavailable)
}

pub fn exclude_window_from_capture(_hwnd: isize) -> Result<()> {
    Err(Error::PlatformUnavailable)
}

/// Non-Windows placeholder for `windows_impl::KillOnCloseJob` -- there is
/// no equivalent primitive implemented for this platform yet. Callers
/// (`crate::ai::worker_client::WorkerClient`) already treat this as a
/// best-effort hardening layer, not the only thing keeping a spawned
/// worker process supervised, so failing here does not change behaviour
/// beyond not having the extra safety net.
pub struct KillOnCloseJob;

impl KillOnCloseJob {
    pub fn new() -> Result<Self> {
        Err(Error::PlatformUnavailable)
    }

    pub fn assign(&self, _process_handle: isize) -> Result<()> {
        Err(Error::PlatformUnavailable)
    }
}
