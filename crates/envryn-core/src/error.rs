use std::fmt;

/// Envryn's error type.
///
/// Deliberately coarse. An error that reaches the user must not distinguish
/// "no vault at this path" from "wrong password" from "corrupt header",
/// because each distinction is an oracle: it tells an attacker holding the
/// file which of their guesses was closer. Detail that is safe to surface
/// locally goes to the log; detail that is not goes nowhere.
///
/// See docs/THREAT_MODEL.md V-01 and INV-006.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Authentication failed. Covers a wrong password, a wrong platform key,
    /// a tampered wrapped key, and a truncated header -- deliberately
    /// indistinguishable from one another.
    #[error("authentication failed")]
    AuthenticationFailed,

    /// A ciphertext failed its authentication tag. The data was altered, or
    /// the wrong key was used. Never reported with any detail about which.
    #[error("data could not be verified")]
    DecryptionFailed,

    /// The vault is locked and the requested operation needs it unlocked.
    #[error("the vault is locked")]
    Locked,

    /// A vault already exists where one was about to be created. Refusing is
    /// the only safe answer: the alternative is overwriting someone's keys.
    #[error("a vault already exists at this location")]
    VaultExists,

    #[error("no vault found")]
    VaultNotFound,

    /// A stored artefact declares a format version this build does not know.
    /// Never best-effort parsed -- guessing wrong about a ciphertext layout
    /// corrupts silently. See docs/CRYPTOGRAPHY.md section 10.
    #[error("unsupported format version {found} (this build supports up to {supported})")]
    UnsupportedVersion { found: u32, supported: u32 },

    #[error("record not found")]
    NotFound,

    #[error("invalid input: {0}")]
    InvalidInput(&'static str),

    #[error("storage error")]
    Storage(#[from] rusqlite::Error),

    #[error("serialisation error")]
    Serialisation(#[from] serde_json::Error),

    #[error("internal error: {0}")]
    Internal(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// A message safe to display to the user.
    ///
    /// The `Display` impl is already redaction-safe, but this makes the intent
    /// explicit at call sites that cross the IPC boundary into the UI.
    pub fn user_message(&self) -> String {
        self.to_string()
    }
}

/// Marker for errors that must never be logged with their payload.
pub struct Redacted<T>(pub T);

impl<T> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl<T> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}
