//! Win32 implementation of the platform primitives.
//!
//! This is the one module in `envryn-core` permitted to contain `unsafe` --
//! the crate-level lint is `deny`, not `forbid`, specifically so this
//! exemption can be scoped here and nowhere else (see Cargo.toml). Every
//! `unsafe` block below carries a safety comment. None of this module handles
//! the Vault Master Key or record plaintext directly; it hands back raw bytes
//! that [`crate::vault`] feeds through the same AEAD wrap/unwrap path used for
//! the password slot, so a bug here cannot silently weaken record encryption.
#![allow(unsafe_code)]

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, LocalFree, HANDLE, HGLOBAL, HLOCAL, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, SetWindowDisplayAffinity, SetWindowLongPtrW, GWLP_WNDPROC,
    WDA_EXCLUDEFROMCAPTURE, WM_WTSSESSION_CHANGE, WNDPROC, WTS_SESSION_LOCK,
};
use zeroize::Zeroizing;

use crate::error::{Error, Result};

/// A label shown by some Windows credential UI if DPAPI ever prompts. It never
/// will here (`CRYPTPROTECT_UI_FORBIDDEN`), but the API requires a description.
const DPAPI_DESCRIPTION: &str = "Envryn vault platform key";

/// The clipboard-history / cloud-sync opt-out format Windows checks for.
/// See <https://learn.microsoft.com/windows/win32/dataxchg/clipboard-formats>
/// and docs/CRYPTOGRAPHY.md's clipboard note. Absence of this format on older
/// Windows builds is a stated residual risk, not a bug -- see THREAT_MODEL.md
/// V-08.
const EXCLUDE_FORMAT_NAME: &str = "ExcludeClipboardContentFromMonitorProcessing";

/// Protect `secret` under the current Windows user account.
///
/// The returned blob decrypts only for the same Windows user on the same
/// machine (DPAPI's standard scope). It is opaque ciphertext, safe to store
/// alongside the vault database.
pub fn dpapi_protect(secret: &[u8]) -> Result<Vec<u8>> {
    // SAFETY: `input` borrows `secret` for the duration of the call only;
    // CryptProtectData reads it and does not retain the pointer. `output` is
    // zero-initialised and only DPAPI writes into it. The `pbData` it returns
    // is allocated by DPAPI via LocalAlloc, per the Win32 contract for this
    // API, and is freed below with LocalFree before returning.
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: secret.len() as u32,
            pbData: secret.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();

        CryptProtectData(
            &input,
            &HSTRING::from(DPAPI_DESCRIPTION),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| Error::PlatformUnavailable)?;

        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
        Ok(bytes)
    }
}

/// Recover a value protected by [`dpapi_protect`].
///
/// Fails if the blob is corrupt, was protected under a different Windows
/// user account, or the account's DPAPI master key is unavailable (for
/// example, a roaming profile that has not synced). All three surface as
/// [`Error::PlatformUnavailable`] with no further distinction, matching the
/// no-detail rule INV-006 applies to password authentication.
pub fn dpapi_unprotect(blob: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    // SAFETY: as above. `ppszdatadescr` is `None`, so DPAPI does not allocate
    // or write a description string we would otherwise have to free.
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: blob.len() as u32,
            pbData: blob.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();

        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| Error::PlatformUnavailable)?;

        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        // Zero DPAPI's own copy before freeing it. LocalFree does not zero the
        // memory it releases, and this buffer briefly held the recovered
        // platform key.
        std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
        Ok(Zeroizing::new(bytes))
    }
}

/// Seconds since the last keyboard or mouse input, system-wide.
///
/// Used for idle auto-lock. System-wide rather than window-scoped
/// deliberately: a user reading a long note in Envryn with the mouse still
/// should not be logged as idle just because they have not touched the
/// keyboard, but they also should not stay "active" forever just because the
/// window has focus -- this matches what every other Windows idle-timeout
/// feature (screen lock, screensaver) already measures.
pub fn idle_seconds() -> Result<u64> {
    // SAFETY: `info` is a plain POD struct; GetLastInputInfo only writes to
    // it. `cbSize` must be set before the call per the Win32 contract, which
    // is how the API validates struct-layout compatibility across versions.
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if !GetLastInputInfo(&mut info).as_bool() {
            return Err(Error::PlatformUnavailable);
        }
        // GetTickCount64 truncated to 32 bits matches GetTickCount's units,
        // and wrapping_sub handles the ~49.7-day rollover MSDN documents for
        // this pattern the same way GetTickCount's own callers must.
        let now = GetTickCount64() as u32;
        let idle_ms = now.wrapping_sub(info.dwTime);
        Ok(u64::from(idle_ms) / 1000)
    }
}

/// RAII guard ensuring `CloseClipboard` runs on every exit path, including an
/// early `?` return -- an unclosed clipboard blocks every other application
/// from reading or writing it until the offending process exits.
struct ClipboardGuard;

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: only ever constructed immediately after a successful
        // OpenClipboard in this module, so there is always a matching open
        // handle to close. CloseClipboard cannot itself panic or corrupt state
        // on failure; the worst case is a leaked clipboard lock, which a
        // #[allow(unsafe_code)] guard existing specifically to avoid.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

/// `OpenClipboard` can fail transiently with no real error: Windows'
/// clipboard-history service, screen readers, and antivirus products all
/// briefly hold the clipboard open when they notice new content -- which is
/// exactly the moment every one of these functions calls this. A short retry
/// loop is the standard remedy (Chromium and most other clipboard-heavy
/// desktop apps do the same). Not a workaround for a bug in our code; the
/// clipboard has always been a shared, briefly-contended resource.
///
/// # Safety
/// The caller must close the clipboard (via [`ClipboardGuard`]) after a
/// successful open.
unsafe fn open_clipboard_with_retry() -> Result<()> {
    const ATTEMPTS: u32 = 10;
    const DELAY: std::time::Duration = std::time::Duration::from_millis(20);

    for attempt in 0..ATTEMPTS {
        // SAFETY: forwarded to the caller's contract; this function performs
        // no operation beyond the FFI call itself.
        if unsafe { OpenClipboard(None) }.is_ok() {
            return Ok(());
        }
        if attempt + 1 < ATTEMPTS {
            std::thread::sleep(DELAY);
        }
    }
    Err(Error::Internal("clipboard unavailable"))
}

/// Copy `text` to the clipboard, tagged so Windows clipboard history and
/// cloud clipboard sync skip it.
///
/// The exclusion tag is best-effort: it is a convention third-party clipboard
/// managers may or may not honour, and Windows versions before the 2018
/// clipboard-history feature do not check for it at all. Documented as a
/// residual risk, not solved, in THREAT_MODEL.md V-08.
pub fn set_clipboard_text_excluded(text: &str) -> Result<()> {
    // SAFETY: OpenClipboard(None) associates the clipboard with the current
    // thread rather than a specific window, which is correct for a
    // background/IPC-triggered copy with no window of its own to pass.
    unsafe {
        open_clipboard_with_retry()?;
        let _guard = ClipboardGuard;

        EmptyClipboard().map_err(|_| Error::Internal("clipboard unavailable"))?;

        let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let byte_len = utf16.len() * std::mem::size_of::<u16>();

        let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len)
            .map_err(|_| Error::Internal("clipboard allocation failed"))?;
        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            return Err(Error::Internal("clipboard lock failed"));
        }
        // SAFETY: `ptr` was just allocated with exactly `byte_len` bytes and
        // is non-null; `utf16` also has `byte_len` bytes. Non-overlapping:
        // the two buffers were allocated independently.
        std::ptr::copy_nonoverlapping(utf16.as_ptr().cast::<u8>(), ptr.cast::<u8>(), byte_len);
        // A `false` return here can mean either "unlock failed" or "the
        // object is now fully unlocked" (Win32's own documented ambiguity for
        // this call) -- the copied bytes are valid either way, so the result
        // is deliberately ignored.
        let _ = GlobalUnlock(hmem);

        // Ownership of `hmem` transfers to the clipboard on success; it must
        // not be freed by us afterwards.
        SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hmem.0)))
            .map_err(|_| Error::Internal("clipboard write failed"))?;

        let exclude_format = RegisterClipboardFormatW(&HSTRING::from(EXCLUDE_FORMAT_NAME));
        if exclude_format != 0 {
            // The marker's content is conventionally ignored by consumers of
            // this format; only its presence matters. A failure to set it
            // does not affect the primary copy, so it is not propagated.
            if let Ok(marker) = GlobalAlloc(GMEM_MOVEABLE, 1) {
                let _ = SetClipboardData(exclude_format, Some(HANDLE(marker.0)));
            }
        }

        Ok(())
    }
}

/// Read the clipboard's current text, if any.
///
/// Used only to confirm the clipboard still holds the value Envryn put there
/// before clearing it -- never to observe arbitrary clipboard content, and
/// the result is never logged.
pub fn read_clipboard_text() -> Result<Option<String>> {
    // SAFETY: OpenClipboard(None) as above; the handle returned by
    // GetClipboardData is owned by the clipboard/system, not by us, so it is
    // never freed here -- only locked, read, and unlocked.
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_err() {
            return Ok(None);
        }
        open_clipboard_with_retry()?;
        let _guard = ClipboardGuard;

        let handle = match GetClipboardData(CF_UNICODETEXT.0 as u32) {
            Ok(h) => h,
            Err(_) => return Ok(None),
        };
        let ptr = GlobalLock(HGLOBAL(handle.0));
        if ptr.is_null() {
            return Ok(None);
        }

        // Find the NUL terminator ourselves: CF_UNICODETEXT carries no
        // explicit length, only a NUL-terminated UTF-16 buffer, per the
        // format's own definition.
        let mut len = 0usize;
        // SAFETY: CF_UNICODETEXT is documented to be NUL-terminated; `ptr`
        // was just successfully locked, so it points at a valid, live
        // allocation for at least as long as the clipboard section is open.
        while *ptr.cast::<u16>().add(len) != 0 {
            len += 1;
        }
        let text = {
            let slice = std::slice::from_raw_parts(ptr.cast::<u16>(), len);
            String::from_utf16_lossy(slice)
        };
        let _ = GlobalUnlock(HGLOBAL(handle.0));

        Ok(Some(text))
    }
}

/// Empty the clipboard.
pub fn clear_clipboard() -> Result<()> {
    // SAFETY: standard open/empty/close sequence; the guard closes on every
    // exit path.
    unsafe {
        open_clipboard_with_retry()?;
        let _guard = ClipboardGuard;
        EmptyClipboard().map_err(|_| Error::Internal("clipboard unavailable"))?;
        Ok(())
    }
}

/// Exclude a window from screen capture (`WDA_EXCLUDEFROMCAPTURE`).
///
/// `hwnd` is the raw handle value (`HWND.0 as isize`) rather than a typed
/// window, so this crate needs no dependency on Tauri or any windowing
/// library to flip one flag on a window it did not create.
///
/// This affects screenshots, screen recording, and remote-desktop mirroring
/// of the window, but not a camera pointed at the physical screen -- stated
/// plainly in docs/THREAT_MODEL.md rather than implied as complete.
pub fn exclude_window_from_capture(hwnd: isize) -> Result<()> {
    // SAFETY: `hwnd` is expected to be a live window handle supplied by the
    // caller (the Tauri shell, from its own main window). Passing a stale or
    // invalid handle fails the call rather than causing memory unsafety --
    // SetWindowDisplayAffinity validates its argument before use.
    unsafe {
        SetWindowDisplayAffinity(HWND(hwnd as *mut _), WDA_EXCLUDEFROMCAPTURE)
            .map_err(|_| Error::Internal("could not enable capture protection"))
    }
}

/// A Windows Job Object configured to terminate every process still
/// assigned to it the instant the *last* handle to the job closes --
/// including when this process exits abnormally (a crash, a forceful kill,
/// Task Manager's "End task") without running any `Drop` at all.
///
/// `crate::ai::worker_client::WorkerClient` assigns the spawned AI worker
/// process to one of these for exactly the gap an ordinary `Child::kill()`
/// called from a `Drop` impl cannot close: if this process's own `Drop`
/// never runs, a worker process with a loaded model and whatever was in its
/// inference buffers would otherwise be orphaned and keep running
/// indefinitely (docs/AI_SECURITY.md section 3's "the only way to be
/// genuinely confident... is killing the process" -- this is what makes
/// that true even when the confident-killing code itself never executes).
///
/// **The handle's lifetime *is* the policy.** Keeping it alive keeps the
/// assigned process(es) able to run normally; dropping it is what triggers
/// termination -- this is not "fire and forget" configuration.
pub struct KillOnCloseJob {
    handle: HANDLE,
}

// SAFETY: `HANDLE` here wraps a Windows kernel job-object handle. The Win32
// APIs this type calls (`AssignProcessToJobObject`, `CloseHandle`) are
// documented as safe to call from any thread; nothing about this type's
// single-field, `&self`-only API introduces a data race the Win32 layer
// does not already handle.
unsafe impl Send for KillOnCloseJob {}
unsafe impl Sync for KillOnCloseJob {}

impl KillOnCloseJob {
    pub fn new() -> Result<Self> {
        // SAFETY: `CreateJobObjectW` is called with no security attributes
        // and an unnamed (anonymous) job, both explicitly permitted by the
        // API; the returned handle is owned by this call and closed exactly
        // once, in `Drop`. `SetInformationJobObject` is passed a pointer to
        // `info` and its exact size, matching the Win32 contract for this
        // information class -- `info` outlives the call (it is a local that
        // is not dropped until after the call returns).
        unsafe {
            let handle = CreateJobObjectW(None, PCWSTR::null())
                .map_err(|_| Error::Internal("could not create job object"))?;

            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(info).cast(),
                u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
                    .unwrap_or(0),
            )
            .is_err()
            {
                let _ = CloseHandle(handle);
                return Err(Error::Internal("could not configure job object"));
            }

            Ok(Self { handle })
        }
    }

    /// Assign a process (identified by its raw Win32 handle value, e.g.
    /// `std::os::windows::io::AsRawHandle::as_raw_handle` on a
    /// `std::process::Child`) to this job. Windows forbids assigning a
    /// process that is already in another job unless that job permits
    /// nesting, so this can fail on some systems -- callers should treat it
    /// as a hardening layer, not the only thing keeping the process
    /// supervised, and keep their own explicit kill logic regardless.
    pub fn assign(&self, process_handle: isize) -> Result<()> {
        // SAFETY: `process_handle` is expected to be a live process handle
        // supplied by the caller (a just-spawned child it owns).
        // `AssignProcessToJobObject` validates both handles before use and
        // fails cleanly rather than causing memory unsafety if either is
        // stale or invalid.
        unsafe {
            AssignProcessToJobObject(self.handle, HANDLE(process_handle as *mut _))
                .map_err(|_| Error::Internal("could not assign process to job object"))
        }
    }
}

impl Drop for KillOnCloseJob {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was created by `CreateJobObjectW` in `new`
        // and this is the only place it is ever closed.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// Global state for the single main-window subclass this app installs. One
/// process, one main window, for the process's whole lifetime -- the same
/// assumption `capture_protection`'s single startup call already makes in
/// `src-tauri` -- so one global slot per piece of state is enough; there is
/// no scenario in this codebase where a second window needs a second
/// callback.
static PREV_WNDPROC: AtomicIsize = AtomicIsize::new(0);
static SESSION_LOCK_CALLBACK: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Subclass the main window to react to Windows locking *this* session
/// (`WM_WTSSESSION_CHANGE` / `WTS_SESSION_LOCK`) directly, rather than only
/// discovering it later via the idle poll -- closes the gap
/// `docs/ARCHITECTURE.md` section 7 previously recorded as open. `on_lock`
/// fires exactly when this session locks; every other session-change reason
/// (unlock, remote-connect, another session's own lock) is deliberately
/// ignored, and every other window message is forwarded unchanged to the
/// window's real procedure.
///
/// `on_lock` lives in a process-wide global to match the process-wide window
/// subclass it answers to -- Envryn has exactly one main window for the life
/// of the process, so this is called at most once in practice.
pub fn watch_session_lock(hwnd: isize, on_lock: impl Fn() + Send + Sync + 'static) -> Result<()> {
    let hwnd = HWND(hwnd as *mut _);
    // A second call would silently replace the queued callback rather than
    // stacking -- fine, since nothing in this codebase calls it more than
    // once, and `OnceLock` makes a genuine double-call a deliberate no-op
    // rather than a panic.
    let _ = SESSION_LOCK_CALLBACK.set(Box::new(on_lock));

    // SAFETY: `hwnd` is a live window handle the caller (the Tauri main
    // window) owns for the life of the process. `NOTIFY_FOR_THIS_SESSION`
    // asks only for this session's own lock/unlock events.
    unsafe {
        WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)
            .map_err(|_| Error::Internal("could not register for session notifications"))?;
    }

    // SAFETY: installing a window procedure is the documented way to observe
    // a message Tauri's own `WindowEvent` does not expose (there is no
    // variant for an arbitrary `WM_*` message). `subclass_proc` below always
    // forwards to the procedure captured here, so every message this window
    // already handled keeps being handled identically.
    let previous =
        unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, subclass_proc as *const () as isize) };
    PREV_WNDPROC.store(previous, Ordering::SeqCst);

    Ok(())
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_WTSSESSION_CHANGE && wparam.0 == WTS_SESSION_LOCK as usize {
        if let Some(callback) = SESSION_LOCK_CALLBACK.get() {
            callback();
        }
    }

    let previous = PREV_WNDPROC.load(Ordering::SeqCst);
    // SAFETY: `previous` was captured by `watch_session_lock` from the same
    // `SetWindowLongPtrW` call that installed this procedure as the current
    // one -- it is the window's real prior procedure, always safe to
    // forward to for any message this function does not itself act on.
    let prev_wndproc: WNDPROC = unsafe { std::mem::transmute(previous) };
    unsafe { CallWindowProcW(prev_wndproc, hwnd, msg, wparam, lparam) }
}

/// Undo [`watch_session_lock`]. Not currently called anywhere -- the
/// subclass and notification registration are meant to live for the whole
/// process, torn down implicitly on exit -- but provided so a future caller
/// (e.g. tests, or a window that can legitimately close before the process
/// exits) is not forced to reinvent an unregister path.
pub fn unwatch_session_lock(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut _);
    // SAFETY: unregistering a session notification for a handle this process
    // previously registered (or never registered, in which case this is a
    // harmless no-op the API itself defines).
    unsafe {
        let _ = WTSUnRegisterSessionNotification(hwnd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real native window, real `SendMessageW` delivery, and the actual
    /// installed subclass procedure -- not a unit test calling
    /// `subclass_proc` directly, which would prove the logic but not that
    /// `watch_session_lock` actually wired it into the window's message
    /// pipeline. Uses the built-in "STATIC" window class so the test needs
    /// no `RegisterClassW`/GDI setup of its own.
    #[test]
    fn subclassed_window_reports_a_session_lock_and_forwards_everything_else() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SendMessageW, HWND_MESSAGE, WINDOW_EX_STYLE, WS_POPUP,
        };

        let hinstance = unsafe { GetModuleHandleW(PCWSTR::null()) }.expect("GetModuleHandleW");
        let class_name = HSTRING::from("STATIC");
        // SAFETY: "STATIC" is a system window class registered by user32 for
        // every process; no class of our own needs registering first. This
        // window is never shown (WS_POPUP, no WS_VISIBLE) and parented to
        // HWND_MESSAGE so it never appears on screen during the test run.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_name.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(hinstance.into()),
                None,
            )
        }
        .expect("CreateWindowExW");

        let locked = Arc::new(AtomicBool::new(false));
        let locked_flag = locked.clone();
        watch_session_lock(hwnd.0 as isize, move || {
            locked_flag.store(true, Ordering::SeqCst);
        })
        .expect("watch_session_lock");

        // A message this subclass does not act on must still reach the
        // window's real procedure -- proving `subclass_proc` forwards rather
        // than swallows. WM_NULL is defined to do nothing and always return 0.
        const WM_NULL: u32 = 0;
        let forwarded = unsafe { SendMessageW(hwnd, WM_NULL, None, None) };
        assert_eq!(
            forwarded.0, 0,
            "an unrelated message must still be forwarded"
        );

        assert!(
            !locked.load(Ordering::SeqCst),
            "must not fire before the message"
        );

        // The message Windows actually sends on a real session lock.
        unsafe {
            SendMessageW(
                hwnd,
                WM_WTSSESSION_CHANGE,
                Some(WPARAM(WTS_SESSION_LOCK as usize)),
                Some(LPARAM(0)),
            );
        }
        assert!(
            locked.load(Ordering::SeqCst),
            "the callback should have fired for WTS_SESSION_LOCK"
        );

        unwatch_session_lock(hwnd.0 as isize);
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }

    /// DPAPI round-trips whatever bytes it is given, including ones that
    /// happen to look like they could confuse a length calculation.
    #[test]
    fn dpapi_round_trip() {
        for secret in [b"".as_slice(), b"short", &[0u8; 64], &[0xFFu8; 32]] {
            let protected = dpapi_protect(secret).expect("dpapi_protect");
            let recovered = dpapi_unprotect(&protected).expect("dpapi_unprotect");
            assert_eq!(recovered.as_slice(), secret);
        }
    }

    /// The whole reason to use DPAPI instead of a fixed key: a corrupted blob
    /// must not silently produce wrong-but-plausible plaintext.
    #[test]
    fn dpapi_rejects_a_tampered_blob() {
        let mut protected = dpapi_protect(b"platform key material").expect("dpapi_protect");
        let last = protected.len() - 1;
        protected[last] ^= 0x01;
        assert!(matches!(
            dpapi_unprotect(&protected),
            Err(Error::PlatformUnavailable)
        ));
    }

    #[test]
    fn idle_seconds_is_queryable() {
        // This machine just produced keyboard/mouse input to run the test
        // suite, or is a CI runner that has never had any -- either way the
        // call must succeed and return a sane, bounded value.
        let idle = idle_seconds().expect("idle_seconds");
        assert!(idle < 60 * 60 * 24 * 365, "idle time is implausibly large");
    }

    #[test]
    fn clipboard_round_trip() {
        let _guard = super::super::clipboard_test_lock();
        set_clipboard_text_excluded("envryn-platform-test-value").expect("set_clipboard");
        let read = read_clipboard_text().expect("read_clipboard");
        assert_eq!(read.as_deref(), Some("envryn-platform-test-value"));

        clear_clipboard().expect("clear_clipboard");
        let after_clear = read_clipboard_text().expect("read_clipboard after clear");
        assert!(after_clear.is_none() || after_clear.as_deref() == Some(""));
    }

    /// A real spawned child, real job-object assignment, real termination --
    /// this is the safety-net property `crate::ai::worker_client::WorkerClient`
    /// depends on: dropping the job kills whatever is still assigned to it,
    /// without that process's own cooperation. `ping` is used as the
    /// long-running child rather than the AI worker fixture because this
    /// module's tests run in the same lib unit-test binary as
    /// `ai::worker_client`'s (where `CARGO_BIN_EXE_*` is unavailable either
    /// way) and `ping.exe` is a standalone, always-present Windows binary
    /// that runs fine with redirected stdio, unlike `cmd /C timeout`.
    #[test]
    fn dropping_the_job_kills_the_assigned_process() {
        use std::os::windows::io::AsRawHandle;
        use std::process::{Command, Stdio};

        let mut child = Command::new("ping")
            .args(["-n", "60", "127.0.0.1"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn ping");
        assert!(
            child.try_wait().expect("try_wait").is_none(),
            "ping should still be running immediately after spawn"
        );

        let job = KillOnCloseJob::new().expect("create job");
        job.assign(child.as_raw_handle() as isize)
            .expect("assign process to job");

        drop(job);
        std::thread::sleep(std::time::Duration::from_millis(500));

        assert!(
            child.try_wait().expect("try_wait after job drop").is_some(),
            "ping should have been killed when the job handle closed"
        );
    }
}
