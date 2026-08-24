//! The key hierarchy.
//!
//! ```text
//! Master Password --Argon2id--> KEK --wraps--> VMK --HKDF--> subkeys
//! ```
//!
//! The VMK indirection buys two properties that direct password-derived
//! encryption cannot provide:
//!
//! 1. Changing the master password rewraps 32 bytes. It never re-encrypts the
//!    vault, so it is instant and cannot half-fail partway through, leaving
//!    some records readable and others not.
//! 2. Paired devices may each hold a different master password, because
//!    pairing transfers the VMK rather than the password.
//!
//! See docs/CRYPTOGRAPHY.md section 2.

use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::aead::{self, Sealed};
use crate::error::{Error, Result};

pub const KEY_LEN: usize = 32;

/// A 32-byte symmetric key.
///
/// Deliberately implements neither `Debug`, `Display`, `Clone`, nor `Serialize`.
/// A key that can be printed is a key that ends up in a log; a key that can be
/// cloned is a key whose copies outlive the zeroization of the original.
/// See INV-001 and docs/THREAT_MODEL.md V-10.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SymmetricKey([u8; KEY_LEN]);

impl SymmetricKey {
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Generate a fresh key from the OS CSPRNG.
    pub fn generate() -> Result<Self> {
        let mut bytes = [0u8; KEY_LEN];
        getrandom::getrandom(&mut bytes).map_err(|_| Error::Internal("CSPRNG unavailable"))?;
        Ok(Self(bytes))
    }

    pub(crate) fn as_array(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Derive a subkey via HKDF-SHA256 with a domain-separating `info` string.
    ///
    /// `info` strings are versioned so a future key-schedule change is
    /// explicit rather than silently producing different keys for the same
    /// stored data.
    pub fn derive_subkey(&self, info: &[u8]) -> Result<SymmetricKey> {
        let hk = Hkdf::<Sha256>::new(None, &self.0);
        let mut out = [0u8; KEY_LEN];
        hk.expand(info, &mut out)
            .map_err(|_| Error::Internal("HKDF expand failed"))?;
        Ok(SymmetricKey(out))
    }
}

// --- Domain separation strings ----------------------------------------------
// Changing any of these makes existing vaults unreadable. They are versioned
// for exactly that reason: a new schedule gets a new string, not a silent edit.

pub const INFO_RECORD: &[u8] = b"envryn/v1/record";
pub const INFO_FINGERPRINT: &[u8] = b"envryn/v1/fingerprint";
pub const INFO_SQLCIPHER: &[u8] = b"envryn/v1/sqlcipher";
pub const INFO_BACKUP: &[u8] = b"envryn/v1/backup";

/// AAD binding the wrapped VMK to the slot that holds it, so a `platform`
/// wrapper cannot be substituted for a `password` wrapper.
fn wrap_aad(slot: KeySlot) -> Vec<u8> {
    let mut aad = b"envryn/v1/wrap/".to_vec();
    aad.extend_from_slice(slot.as_str().as_bytes());
    aad
}

/// Which unwrap path a wrapped VMK belongs to.
///
/// Both slots wrap the *same* VMK. Platform authentication is therefore an
/// additional route to the vault, never a bypass of the password -- losing a
/// fingerprint sensor does not lose the vault (INV-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySlot {
    Password,
    Platform,
}

impl KeySlot {
    pub fn as_str(self) -> &'static str {
        match self {
            KeySlot::Password => "password",
            KeySlot::Platform => "platform",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "password" => Ok(KeySlot::Password),
            "platform" => Ok(KeySlot::Platform),
            _ => Err(Error::InvalidInput("unknown key slot")),
        }
    }
}

/// The Vault Master Key. Everything in the vault is reachable from this.
///
/// Never persisted except wrapped. Never leaves the Rust core. Never crosses
/// the IPC boundary to the UI, and never reaches the AI subsystem
/// (AI-INV-001).
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct VaultMasterKey(SymmetricKey);

impl VaultMasterKey {
    pub fn generate() -> Result<Self> {
        Ok(Self(SymmetricKey::generate()?))
    }

    /// Adopt a VMK received over a completed pairing exchange, or reconstruct
    /// one in a test. Not reachable from the IPC surface: a VMK must only ever
    /// enter the process by unwrapping or by pairing.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn from_key(key: SymmetricKey) -> Self {
        Self(key)
    }

    /// Wrap the VMK under a key-encryption key for storage.
    pub fn wrap(&self, kek: &SymmetricKey, slot: KeySlot) -> Result<Sealed> {
        aead::seal(kek, self.0.as_slice(), &wrap_aad(slot))
    }

    /// Unwrap a stored VMK.
    ///
    /// Returns `AuthenticationFailed` rather than `DecryptionFailed`, because
    /// at this point the only realistic cause is a wrong password, and the
    /// caller should surface it as such (INV-006).
    pub fn unwrap_from(kek: &SymmetricKey, sealed: &Sealed, slot: KeySlot) -> Result<Self> {
        let plain =
            aead::open(kek, sealed, &wrap_aad(slot)).map_err(|_| Error::AuthenticationFailed)?;
        if plain.len() != KEY_LEN {
            return Err(Error::AuthenticationFailed);
        }
        let mut bytes = [0u8; KEY_LEN];
        bytes.copy_from_slice(&plain);
        Ok(Self(SymmetricKey::from_bytes(bytes)))
    }

    pub fn derive(&self, info: &[u8]) -> Result<SymmetricKey> {
        self.0.derive_subkey(info)
    }
}

/// The subkeys held while the vault is unlocked.
///
/// Bundled into one struct so that locking is a single drop: there is no way
/// to zeroize three of four keys and forget the fourth.
#[derive(ZeroizeOnDrop)]
pub struct VaultKeys {
    pub record: SymmetricKey,
    pub fingerprint: SymmetricKey,
    pub backup: SymmetricKey,
}

impl VaultKeys {
    pub fn derive_from(vmk: &VaultMasterKey) -> Result<Self> {
        Ok(Self {
            record: vmk.derive(INFO_RECORD)?,
            fingerprint: vmk.derive(INFO_FINGERPRINT)?,
            backup: vmk.derive(INFO_BACKUP)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_unwrap_round_trip() {
        let vmk = VaultMasterKey::generate().unwrap();
        let kek = SymmetricKey::from_bytes([1u8; 32]);

        let wrapped = vmk.wrap(&kek, KeySlot::Password).unwrap();
        let recovered = VaultMasterKey::unwrap_from(&kek, &wrapped, KeySlot::Password).unwrap();

        // Same VMK must derive identical subkeys.
        assert_eq!(
            vmk.derive(INFO_RECORD).unwrap().as_slice(),
            recovered.derive(INFO_RECORD).unwrap().as_slice()
        );
    }

    #[test]
    fn wrong_kek_fails_as_authentication_failure() {
        let vmk = VaultMasterKey::generate().unwrap();
        let wrapped = vmk
            .wrap(&SymmetricKey::from_bytes([1u8; 32]), KeySlot::Password)
            .unwrap();
        let wrong = SymmetricKey::from_bytes([2u8; 32]);
        assert!(matches!(
            VaultMasterKey::unwrap_from(&wrong, &wrapped, KeySlot::Password),
            Err(Error::AuthenticationFailed)
        ));
    }

    /// A wrapped VMK from the platform slot must not open as a password slot.
    /// Without the AAD binding, an attacker who could write the database could
    /// move a platform wrapper into the password row.
    #[test]
    fn slot_binding_is_enforced() {
        let vmk = VaultMasterKey::generate().unwrap();
        let kek = SymmetricKey::from_bytes([3u8; 32]);
        let wrapped = vmk.wrap(&kek, KeySlot::Platform).unwrap();
        assert!(matches!(
            VaultMasterKey::unwrap_from(&kek, &wrapped, KeySlot::Password),
            Err(Error::AuthenticationFailed)
        ));
    }

    /// Both slots wrap the same VMK, so either unlocks the same vault.
    /// This is INV-007: platform auth is an additional route, not a bypass.
    #[test]
    fn both_slots_recover_the_same_vmk() {
        let vmk = VaultMasterKey::generate().unwrap();
        let pw_kek = SymmetricKey::from_bytes([4u8; 32]);
        let pl_kek = SymmetricKey::from_bytes([5u8; 32]);

        let from_pw = VaultMasterKey::unwrap_from(
            &pw_kek,
            &vmk.wrap(&pw_kek, KeySlot::Password).unwrap(),
            KeySlot::Password,
        )
        .unwrap();
        let from_pl = VaultMasterKey::unwrap_from(
            &pl_kek,
            &vmk.wrap(&pl_kek, KeySlot::Platform).unwrap(),
            KeySlot::Platform,
        )
        .unwrap();

        assert_eq!(
            from_pw.derive(INFO_RECORD).unwrap().as_slice(),
            from_pl.derive(INFO_RECORD).unwrap().as_slice()
        );
    }

    #[test]
    fn subkeys_are_domain_separated() {
        let vmk = VaultMasterKey::generate().unwrap();
        let record = vmk.derive(INFO_RECORD).unwrap();
        let fingerprint = vmk.derive(INFO_FINGERPRINT).unwrap();
        let backup = vmk.derive(INFO_BACKUP).unwrap();

        assert_ne!(record.as_slice(), fingerprint.as_slice());
        assert_ne!(record.as_slice(), backup.as_slice());
        assert_ne!(fingerprint.as_slice(), backup.as_slice());
    }

    #[test]
    fn derivation_is_deterministic() {
        let vmk = VaultMasterKey::from_key(SymmetricKey::from_bytes([9u8; 32]));
        assert_eq!(
            vmk.derive(INFO_RECORD).unwrap().as_slice(),
            vmk.derive(INFO_RECORD).unwrap().as_slice()
        );
    }

    /// Known-answer test for the subkey schedule.
    ///
    /// These vectors were cross-checked against an independent HKDF-SHA256
    /// implementation (Node's `crypto.hkdfSync`) rather than captured from
    /// this code, so they verify correctness and not merely self-consistency.
    ///
    /// If this test fails, every existing vault has become unreadable.
    /// Updating the constants is only ever correct alongside a
    /// `crypto_version` bump and a migration.
    #[test]
    fn subkey_schedule_matches_known_answers() {
        let vmk = VaultMasterKey::from_key(SymmetricKey::from_bytes([0u8; 32]));

        let hex = |k: &SymmetricKey| -> String {
            k.as_slice().iter().map(|b| format!("{b:02x}")).collect()
        };

        assert_eq!(
            hex(&vmk.derive(INFO_RECORD).unwrap()),
            "9904812f036766c01f1228c40b7c5b2fa75232ff580a7579f9ea8bc671344750"
        );
        assert_eq!(
            hex(&vmk.derive(INFO_FINGERPRINT).unwrap()),
            "5c11815e1ae0a158946d167e96febe877ca4711c9d5c7ec21dd311bc17eb63cb"
        );
    }

    #[test]
    fn generated_keys_differ() {
        let a = SymmetricKey::generate().unwrap();
        let b = SymmetricKey::generate().unwrap();
        assert_ne!(a.as_slice(), b.as_slice());
    }

    #[test]
    fn key_slot_round_trips() {
        assert_eq!(KeySlot::parse("password").unwrap(), KeySlot::Password);
        assert_eq!(KeySlot::parse("platform").unwrap(), KeySlot::Platform);
        assert!(KeySlot::parse("nonsense").is_err());
    }
}
