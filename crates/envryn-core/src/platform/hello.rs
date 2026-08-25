//! Windows Hello as an authentication *gate* in front of the existing
//! DPAPI-based platform key wrap (`KeySlot::Platform`) -- deliberately not a
//! separate, stronger key-wrapping scheme.
//!
//! Why not stronger: the WinRT surface Windows Hello for Apps exposes
//! (`KeyCredentialManager`) only supports *signing*, not decrypt or key
//! agreement, and standard ECDSA signatures are not deterministic -- so
//! there is no way to derive a stable unwrap key from "hash the signature"
//! the way DPAPI's recovered bytes work today. A cryptographically stronger
//! binding would need raw CNG against the Microsoft Passport key storage
//! provider, a materially larger and riskier undertaking than this codebase
//! has precedent for; see `docs/ARCHITECTURE.md` section 7 for the tradeoff
//! as recorded there.
//!
//! What this module *does* buy: the OS itself refuses to let
//! [`hello_verify`] return success unless the enrolled biometric or PIN
//! gesture succeeds at `RequestSignAsync` time -- a genuine, OS-enforced
//! presence check. `crate::vault`/the IPC layer call this before attempting
//! the ordinary DPAPI unlock, so "unlock with Windows Hello" really does
//! require the gesture, even though the DPAPI unwrap behind it is exactly
//! as strong as it already was. Nothing here should be read as "the vault
//! key is bound to your fingerprint" -- it is not, and the UI copy must not
//! imply it is (same rule `ARCHITECTURE.md` already applies to the plain
//! DPAPI slot's "Unlock with this Windows account" wording).
//!
//! No `unsafe` anywhere in this file: unlike `windows_impl`'s raw Win32 FFI,
//! the `windows` crate's WinRT projection wraps every COM/vtable call
//! safely internally, so this module needs no exemption from the crate's
//! `unsafe_code = "deny"` lint.

use windows::core::HSTRING;
use windows::Security::Credentials::{
    KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus,
};
use windows::Storage::Streams::DataWriter;

use crate::error::{Error, Result};

const CREDENTIAL_NAME: &str = "Envryn";

/// Fixed, non-secret bytes signed purely to force the OS consent gesture --
/// never used as key material, so a fixed value costs nothing and needs no
/// randomness.
const CHALLENGE: &[u8] = b"envryn/v1/windows-hello-gate";

/// Whether Windows Hello for Apps is available on this machine (supporting
/// hardware plus an enrolled biometric or PIN) -- checked before the
/// Settings UI offers the option at all.
pub fn hello_supported() -> bool {
    KeyCredentialManager::IsSupportedAsync()
        .and_then(|op| op.get())
        .unwrap_or(false)
}

/// Create (or replace) this installation's Windows Hello credential.
/// Triggers the OS enrollment/consent UI. Idempotent -- `ReplaceExisting`
/// means calling this again (e.g. the user re-enables the setting after
/// disabling it) does not error on an already-present credential.
pub fn hello_enroll() -> Result<()> {
    let op = KeyCredentialManager::RequestCreateAsync(
        &HSTRING::from(CREDENTIAL_NAME),
        KeyCredentialCreationOption::ReplaceExisting,
    )
    .map_err(|_| Error::PlatformUnavailable)?;
    let result = op.get().map_err(|_| Error::PlatformUnavailable)?;
    if result.Status().map_err(|_| Error::PlatformUnavailable)? != KeyCredentialStatus::Success {
        return Err(Error::PlatformUnavailable);
    }
    Ok(())
}

/// Remove this installation's Windows Hello credential (mirrors disabling
/// the platform DPAPI slot -- turning a setting off should undo what turning
/// it on created).
pub fn hello_forget() -> Result<()> {
    let op = KeyCredentialManager::DeleteAsync(&HSTRING::from(CREDENTIAL_NAME))
        .map_err(|_| Error::PlatformUnavailable)?;
    op.get().map_err(|_| Error::PlatformUnavailable)
}

/// The actual gate: prompts for the enrolled biometric/PIN and returns
/// `Ok(())` only once the OS confirms the gesture succeeded. Every failure
/// -- no credential enrolled, the user cancelled, the gesture failed --
/// surfaces as [`Error::AuthenticationFailed`] with no further distinction,
/// matching the no-detail rule INV-006 applies to every other authentication
/// path in this codebase.
pub fn hello_verify() -> Result<()> {
    let open_op = KeyCredentialManager::OpenAsync(&HSTRING::from(CREDENTIAL_NAME))
        .map_err(|_| Error::AuthenticationFailed)?;
    let open_result = open_op.get().map_err(|_| Error::AuthenticationFailed)?;
    if open_result
        .Status()
        .map_err(|_| Error::AuthenticationFailed)?
        != KeyCredentialStatus::Success
    {
        return Err(Error::AuthenticationFailed);
    }
    let credential = open_result
        .Credential()
        .map_err(|_| Error::AuthenticationFailed)?;

    let writer = DataWriter::new().map_err(|_| Error::AuthenticationFailed)?;
    writer
        .WriteBytes(CHALLENGE)
        .map_err(|_| Error::AuthenticationFailed)?;
    let challenge_buffer = writer
        .DetachBuffer()
        .map_err(|_| Error::AuthenticationFailed)?;

    // This call is the point at which Windows actually prompts for the
    // biometric or PIN gesture (`OpenAsync` above only resolves the existing
    // credential handle; it does not by itself require fresh user presence).
    let sign_op = credential
        .RequestSignAsync(&challenge_buffer)
        .map_err(|_| Error::AuthenticationFailed)?;
    let sign_result = sign_op.get().map_err(|_| Error::AuthenticationFailed)?;
    if sign_result
        .Status()
        .map_err(|_| Error::AuthenticationFailed)?
        != KeyCredentialStatus::Success
    {
        return Err(Error::AuthenticationFailed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real, always-run: `hello_supported` must never panic and must return
    /// a plain bool regardless of whether this machine actually has Windows
    /// Hello hardware/enrollment -- the one part of this module safe to
    /// exercise without popping an interactive OS prompt.
    #[test]
    fn hello_supported_does_not_error() {
        let supported = hello_supported();
        println!("hello_supported on this machine: {supported}");
    }

    /// Not run by default: `hello_enroll` and `hello_verify` both trigger a
    /// real interactive Windows Hello prompt (biometric or PIN), which would
    /// hang a headless test run and requires hardware/enrollment this
    /// environment cannot guarantee. Run manually on a machine with Windows
    /// Hello set up:
    ///
    /// ```text
    /// cargo test -p envryn-core --lib platform::hello::tests::hello_enroll_and_verify_round_trip -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "requires real Windows Hello hardware/enrollment and an interactive prompt"]
    fn hello_enroll_and_verify_round_trip() {
        assert!(
            hello_supported(),
            "Windows Hello is not available on this machine"
        );
        hello_enroll().expect("enroll should succeed with the OS enrollment UI completed");
        hello_verify().expect("verify should succeed after completing the biometric/PIN prompt");
        hello_forget().expect("forget should clean up the credential this test created");
    }
}
