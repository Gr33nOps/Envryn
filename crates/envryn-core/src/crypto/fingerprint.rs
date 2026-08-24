//! Keyed fingerprints for duplicate detection.
//!
//! An *unkeyed* hash of a secret value is an offline guessing oracle. Real
//! credentials are often low-entropy -- `changeme`, a short database password,
//! a four-digit PIN -- and an attacker holding the database file could confirm
//! a guess instantly by hashing it. Keying under a VMK-derived subkey means
//! they cannot compute candidate fingerprints at all without first breaking
//! the master password.
//!
//! See docs/CRYPTOGRAPHY.md section 5 and THREAT_MODEL.md V-11.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::crypto::keys::SymmetricKey;
use crate::error::{Error, Result};

type HmacSha256 = Hmac<Sha256>;

pub const FINGERPRINT_LEN: usize = 16;

/// A 128-bit keyed fingerprint of a secret value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint([u8; FINGERPRINT_LEN]);

impl Fingerprint {
    pub fn as_bytes(&self) -> &[u8; FINGERPRINT_LEN] {
        &self.0
    }

    pub fn from_bytes(bytes: [u8; FINGERPRINT_LEN]) -> Self {
        Self(bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FINGERPRINT_LEN {
            return Err(Error::InvalidInput("fingerprint has the wrong length"));
        }
        let mut out = [0u8; FINGERPRINT_LEN];
        out.copy_from_slice(bytes);
        Ok(Self(out))
    }

    /// Constant-time equality.
    ///
    /// A fingerprint is not itself a secret, but comparison timing over a set
    /// of them leaks which entries share a prefix, and the constant-time
    /// version costs nothing here.
    pub fn ct_eq(&self, other: &Fingerprint) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

/// Normalise a value before fingerprinting.
///
/// Trims surrounding whitespace and nothing else. Deliberately does *not*
/// lowercase or strip punctuation: secrets are byte-sensitive, and two values
/// differing only in case are genuinely different secrets. Folding them
/// together would report a false duplicate on credentials that are not
/// interchangeable.
fn normalize(value: &str) -> &str {
    value.trim()
}

/// Compute the keyed fingerprint of a secret value.
pub fn fingerprint(key: &SymmetricKey, value: &str) -> Result<Fingerprint> {
    let mut mac = HmacSha256::new_from_slice(key.as_slice())
        .map_err(|_| Error::Internal("HMAC key rejected"))?;
    mac.update(normalize(value).as_bytes());
    let full = mac.finalize().into_bytes();

    // SHA-256 always yields 32 bytes and FINGERPRINT_LEN is 16, so this cannot
    // fail -- but expressed as a fallible lookup rather than a slice, so the
    // "no panics in the vault core" rule holds without an exemption.
    let head = full
        .get(..FINGERPRINT_LEN)
        .ok_or(Error::Internal("HMAC output shorter than expected"))?;

    let mut out = [0u8; FINGERPRINT_LEN];
    out.copy_from_slice(head);
    Ok(Fingerprint(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> SymmetricKey {
        SymmetricKey::from_bytes([42u8; 32])
    }

    #[test]
    fn identical_values_match() {
        let k = key();
        assert_eq!(
            fingerprint(&k, "ghp_abcdef").unwrap(),
            fingerprint(&k, "ghp_abcdef").unwrap()
        );
    }

    #[test]
    fn different_values_differ() {
        let k = key();
        assert_ne!(
            fingerprint(&k, "ghp_abcdef").unwrap(),
            fingerprint(&k, "ghp_abcdeg").unwrap()
        );
    }

    #[test]
    fn surrounding_whitespace_is_ignored() {
        let k = key();
        assert_eq!(
            fingerprint(&k, "  ghp_abcdef\n").unwrap(),
            fingerprint(&k, "ghp_abcdef").unwrap()
        );
    }

    /// Case must remain significant -- `Secret1` and `secret1` are different
    /// credentials and must not be reported as duplicates of each other.
    #[test]
    fn case_is_significant() {
        let k = key();
        assert_ne!(
            fingerprint(&k, "Secret1").unwrap(),
            fingerprint(&k, "secret1").unwrap()
        );
    }

    /// The point of keying: two vaults holding the same credential produce
    /// different fingerprints, so a stolen database reveals nothing about
    /// which well-known values it contains.
    #[test]
    fn different_keys_produce_different_fingerprints() {
        let a = SymmetricKey::from_bytes([1u8; 32]);
        let b = SymmetricKey::from_bytes([2u8; 32]);
        assert_ne!(
            fingerprint(&a, "changeme").unwrap(),
            fingerprint(&b, "changeme").unwrap()
        );
    }

    #[test]
    fn fingerprint_does_not_contain_the_value() {
        let k = key();
        let value = "averydistinctivevalue";
        let fp = fingerprint(&k, value).unwrap();
        assert!(!fp
            .as_bytes()
            .windows(value.len().min(FINGERPRINT_LEN))
            .any(|w| w == &value.as_bytes()[..w.len()]));
    }

    #[test]
    fn round_trips_through_storage_form() {
        let k = key();
        let fp = fingerprint(&k, "value").unwrap();
        let restored = Fingerprint::from_slice(fp.as_bytes()).unwrap();
        assert!(fp.ct_eq(&restored));
    }

    #[test]
    fn wrong_length_is_rejected() {
        assert!(Fingerprint::from_slice(&[0u8; 8]).is_err());
        assert!(Fingerprint::from_slice(&[0u8; 32]).is_err());
    }

    #[test]
    fn empty_value_is_handled() {
        let k = key();
        assert_ne!(fingerprint(&k, "").unwrap(), fingerprint(&k, "x").unwrap());
    }
}
