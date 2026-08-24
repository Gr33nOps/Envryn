//! Authenticated encryption.
//!
//! XChaCha20-Poly1305 with a fresh random 192-bit nonce per write.
//!
//! The nonce size is the reason for the choice. Envryn synchronises between
//! devices that generate nonces independently with no shared counter, so
//! nonces must be safe to pick at random. AES-GCM's 96-bit nonce has a
//! birthday bound that makes that uncomfortable at scale; 192 bits does not.
//! See docs/CRYPTOGRAPHY.md section 1.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::crypto::keys::SymmetricKey;
use crate::error::{Error, Result};

pub const NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;

/// A sealed blob: nonce followed by ciphertext-with-tag.
///
/// Stored as a single opaque column. Callers never need to split it, which
/// removes a whole family of off-by-one bugs at the storage boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed(Vec<u8>);

impl Sealed {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// Reconstruct from storage. Validates only the length; authenticity is
    /// established by `open`, never by this constructor.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < NONCE_LEN + TAG_LEN {
            return Err(Error::DecryptionFailed);
        }
        Ok(Self(bytes))
    }
}

/// Encrypt `plaintext`, binding `aad` into the authentication tag.
///
/// `aad` is not optional by accident. Every caller in Envryn passes something
/// that identifies *where* the ciphertext belongs, so that a blob moved to a
/// different row fails to open rather than decrypting into the wrong context.
/// See docs/CRYPTOGRAPHY.md section 3.
pub fn seal(key: &SymmetricKey, plaintext: &[u8], aad: &[u8]) -> Result<Sealed> {
    let cipher = XChaCha20Poly1305::new(key.as_array().into());

    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce_bytes).map_err(|_| Error::Internal("CSPRNG unavailable"))?;
    let nonce = XNonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| Error::Internal("encryption failed"))?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(Sealed(out))
}

/// Decrypt and verify. Returns plaintext that zeroizes on drop.
///
/// Any failure -- wrong key, altered ciphertext, altered AAD, truncated blob --
/// produces the same `DecryptionFailed`. The caller cannot tell them apart,
/// and neither can an attacker probing through the caller.
pub fn open(key: &SymmetricKey, sealed: &Sealed, aad: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let bytes = sealed.as_bytes();
    if bytes.len() < NONCE_LEN + TAG_LEN {
        return Err(Error::DecryptionFailed);
    }
    let (nonce_bytes, ciphertext) = bytes.split_at(NONCE_LEN);

    let cipher = XChaCha20Poly1305::new(key.as_array().into());
    let nonce = XNonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| Error::DecryptionFailed)?;

    Ok(Zeroizing::new(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> SymmetricKey {
        SymmetricKey::from_bytes([7u8; 32])
    }

    #[test]
    fn round_trip() {
        let k = key();
        let sealed = seal(&k, b"gsk_secret_value", b"record-1").unwrap();
        let opened = open(&k, &sealed, b"record-1").unwrap();
        assert_eq!(opened.as_slice(), b"gsk_secret_value");
    }

    #[test]
    fn plaintext_is_not_present_in_ciphertext() {
        let k = key();
        let secret = b"sk-proj-a-very-distinctive-value";
        let sealed = seal(&k, secret, b"aad").unwrap();
        assert!(
            !sealed
                .as_bytes()
                .windows(secret.len())
                .any(|w| w == secret.as_slice()),
            "plaintext leaked into the sealed blob"
        );
    }

    #[test]
    fn wrong_key_fails() {
        let sealed = seal(&key(), b"value", b"aad").unwrap();
        let other = SymmetricKey::from_bytes([8u8; 32]);
        assert!(matches!(
            open(&other, &sealed, b"aad"),
            Err(Error::DecryptionFailed)
        ));
    }

    /// The AAD binding is what stops a ciphertext being moved between rows --
    /// e.g. swapping a staging credential into the row labelled production.
    #[test]
    fn wrong_aad_fails() {
        let k = key();
        let sealed = seal(&k, b"value", b"record-1").unwrap();
        assert!(matches!(
            open(&k, &sealed, b"record-2"),
            Err(Error::DecryptionFailed)
        ));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let k = key();
        let sealed = seal(&k, b"value", b"aad").unwrap();
        let mut bytes = sealed.into_bytes();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        let tampered = Sealed::from_bytes(bytes).unwrap();
        assert!(matches!(
            open(&k, &tampered, b"aad"),
            Err(Error::DecryptionFailed)
        ));
    }

    #[test]
    fn tampered_nonce_fails() {
        let k = key();
        let sealed = seal(&k, b"value", b"aad").unwrap();
        let mut bytes = sealed.into_bytes();
        bytes[0] ^= 0x01;
        let tampered = Sealed::from_bytes(bytes).unwrap();
        assert!(matches!(
            open(&k, &tampered, b"aad"),
            Err(Error::DecryptionFailed)
        ));
    }

    #[test]
    fn truncated_blob_fails_cleanly() {
        assert!(matches!(
            Sealed::from_bytes(vec![0u8; NONCE_LEN + TAG_LEN - 1]),
            Err(Error::DecryptionFailed)
        ));
    }

    /// Two seals of identical plaintext must differ, or the ciphertext leaks
    /// equality -- which would reveal that two projects share a credential.
    #[test]
    fn nonces_are_unique_per_seal() {
        let k = key();
        let a = seal(&k, b"same", b"aad").unwrap();
        let b = seal(&k, b"same", b"aad").unwrap();
        assert_ne!(a, b);
        assert_ne!(&a.as_bytes()[..NONCE_LEN], &b.as_bytes()[..NONCE_LEN]);
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let k = key();
        let sealed = seal(&k, b"", b"aad").unwrap();
        assert_eq!(open(&k, &sealed, b"aad").unwrap().as_slice(), b"");
    }
}
