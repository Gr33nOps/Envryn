//! Device identity.
//!
//! One Ed25519 keypair per installation, generated at first run. This is
//! deliberately **not** vault content: it lives in its own small file,
//! independent of the vault database, because resetting or deleting a vault
//! must not change how paired peers recognise this device. See
//! docs/CRYPTOGRAPHY.md section 6.
//!
//! Where DPAPI (or an equivalent) exists, the private key is sealed with
//! [`crate::platform::dpapi_protect`] -- reusing the exact same platform
//! primitive the vault's platform-unlock slot uses, rather than inventing a
//! second way to ask the OS to protect a secret.
//!
//! **On a platform with no such primitive (Android today), the key is
//! stored as plain bytes inside this app's own private, sandboxed data
//! directory, protected only by OS-level app-isolation -- not left
//! unprotected.** This was a real bug, not a deliberate scope decision: every
//! sync/pairing command calls [`DeviceIdentity::load_or_create`] before doing
//! anything else, and until this was fixed, that unconditionally tried to
//! seal or unseal via DPAPI and failed with [`Error::PlatformUnavailable`] on
//! every non-Windows platform -- meaning pairing was completely unusable on
//! Android, not merely less protected, despite `sync::pairing`'s own module
//! doc always having described QR pairing as "Windows <-> Android." Losing
//! this key only lets an attacker impersonate this device to peers that
//! already trust it; it is not the vault's own encryption key, which is
//! wrapped from the master password via `crypto::kdf` on every platform
//! identically and is unaffected by any of this.

use std::path::Path;

use ed25519_dalek::pkcs8::EncodePrivateKey;
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::platform;

pub const FINGERPRINT_LEN: usize = 32;

/// SHA-256 of the raw 32-byte Ed25519 public key.
///
/// Deliberately *not* SHA-256 of the full DER-encoded SubjectPublicKeyInfo --
/// for Ed25519, the SPKI's BIT STRING content is exactly the raw 32-byte key
/// with no further structure (RFC 8410), so hashing the raw key directly
/// produces the identical value with less code, and no DER encoding decision
/// (canonical form, etc.) to get subtly wrong. The verifier in
/// `sync::transport` extracts this same 32 bytes from a peer's presented
/// certificate via `x509-parser` before hashing, so both sides always agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; FINGERPRINT_LEN]);

impl Fingerprint {
    pub fn of_public_key(key: &VerifyingKey) -> Self {
        Self::of_raw_bytes(key.as_bytes())
    }

    /// Wrap an already-computed 32-byte fingerprint (e.g. a raw column read
    /// from `trusted_devices.fingerprint`) without hashing it again --
    /// distinct from [`Fingerprint::of_raw_bytes`], which hashes its input.
    pub fn from_bytes(bytes: [u8; FINGERPRINT_LEN]) -> Self {
        Self(bytes)
    }

    pub fn of_raw_bytes(raw: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(raw);
        let digest = hasher.finalize();
        let mut out = [0u8; FINGERPRINT_LEN];
        out.copy_from_slice(&digest);
        Self(out)
    }

    pub fn as_bytes(&self) -> &[u8; FINGERPRINT_LEN] {
        &self.0
    }

    /// Colon-separated uppercase hex, matching what the UI already renders
    /// for device fingerprints (`apps/ui/src/routes/vault/devices.tsx`).
    pub fn to_display_string(&self) -> String {
        self.0
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn from_hex(s: &str) -> Result<Self> {
        if s.len() != FINGERPRINT_LEN * 2 {
            return Err(Error::InvalidInput("fingerprint has the wrong length"));
        }
        let mut out = [0u8; FINGERPRINT_LEN];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            let byte_str = std::str::from_utf8(chunk)
                .map_err(|_| Error::InvalidInput("invalid fingerprint"))?;
            let byte = u8::from_str_radix(byte_str, 16)
                .map_err(|_| Error::InvalidInput("invalid fingerprint"))?;
            let Some(slot) = out.get_mut(i) else {
                return Err(Error::InvalidInput("invalid fingerprint"));
            };
            *slot = byte;
        }
        Ok(Self(out))
    }
}

/// Every identity file written before this fix had no `sealed` field at all,
/// and every one of them *was* DPAPI-sealed (Android/non-Windows support did
/// not exist yet to write anything else) -- so a missing field must default
/// to `true`, not `false`, or an upgrade would try to use already-sealed
/// bytes as a raw secret key and reject every existing installation's
/// identity as corrupt.
fn sealed_defaults_to_true() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct IdentityFile {
    device_id: String,
    /// Whether `sealed_secret_key` went through `platform::dpapi_protect`
    /// (true) or is the raw 32-byte secret key (false, non-Windows only).
    #[serde(default = "sealed_defaults_to_true")]
    sealed: bool,
    /// The 32-byte Ed25519 secret key -- DPAPI-protected when `sealed` is
    /// true, raw bytes when it is false.
    sealed_secret_key: Vec<u8>,
    /// Public; stored alongside so the fingerprint is available without
    /// unsealing the private key.
    public_key: [u8; 32],
    created_ms: i64,
}

/// This installation's identity: a keypair, held unsealed only while this
/// struct is alive.
pub struct DeviceIdentity {
    pub device_id: String,
    signing_key: SigningKey,
    created_ms: i64,
}

impl DeviceIdentity {
    /// Load the identity at `path`, generating and sealing a new one on first
    /// run. `path` is a small JSON file, independent of any vault.
    ///
    /// Only a genuinely missing file triggers creation. Any other read
    /// failure (permissions, a transient lock from antivirus scanning the
    /// file, a full disk mid-read) is a hard error rather than falling
    /// through to "create a new identity" -- silently minting a fresh
    /// identity here would orphan every device that already trusts the real
    /// one, with no warning to the user that anything happened.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        Self::load_or_create_with_sealing(path, platform::dpapi_available())
    }

    /// `seal` is injected rather than read from [`platform::dpapi_available`]
    /// directly so both branches -- sealed (Windows) and unsealed
    /// (Android/non-Windows) -- can be exercised by tests on any host, not
    /// only by cross-compiling. [`Self::load_or_create`] is the only real
    /// caller and always supplies the true platform value.
    fn load_or_create_with_sealing(path: &Path, seal: bool) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => Self::from_file(&bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self::create(path, seal),
            Err(_) => Err(Error::Internal("could not read the device identity file")),
        }
    }

    fn from_file(bytes: &[u8]) -> Result<Self> {
        let file: IdentityFile = serde_json::from_slice(bytes)
            .map_err(|_| Error::InvalidInput("corrupt device identity"))?;
        let secret_bytes = if file.sealed {
            platform::dpapi_unprotect(&file.sealed_secret_key)
                .map_err(|_| Error::PlatformUnavailable)?
        } else {
            // Not sealed by design on this platform (see the module doc) --
            // the bytes on disk already are the raw secret key.
            zeroize::Zeroizing::new(file.sealed_secret_key.clone())
        };
        let mut secret = [0u8; 32];
        if secret_bytes.len() != 32 {
            return Err(Error::InvalidInput("corrupt device identity"));
        }
        secret.copy_from_slice(&secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret);

        if signing_key.verifying_key().to_bytes() != file.public_key {
            // The sealed secret no longer matches the recorded public key --
            // treat as corruption rather than silently using a mismatched
            // pair, which would make this device unrecognisable to peers
            // that already trust the recorded public key.
            return Err(Error::InvalidInput("device identity is inconsistent"));
        }

        Ok(Self {
            device_id: file.device_id,
            signing_key,
            created_ms: file.created_ms,
        })
    }

    fn create(path: &Path, seal: bool) -> Result<Self> {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let public_key = signing_key.verifying_key().to_bytes();
        let raw_secret = signing_key.to_bytes();
        let sealed_secret_key = if seal {
            platform::dpapi_protect(&raw_secret)?
        } else {
            raw_secret.to_vec()
        };
        let created_ms = now_ms();
        let device_id = uuid::Uuid::now_v7().to_string();

        let file = IdentityFile {
            device_id: device_id.clone(),
            sealed: seal,
            sealed_secret_key,
            public_key,
            created_ms,
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| Error::Internal("could not create the identity directory"))?;
        }
        std::fs::write(path, bytes)
            .map_err(|_| Error::Internal("could not write device identity"))?;

        Ok(Self {
            device_id,
            signing_key,
            created_ms,
        })
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of_public_key(&self.verifying_key())
    }

    pub fn created_ms(&self) -> i64 {
        self.created_ms
    }

    /// Sign `message` with this device's identity key. Used by the pairing
    /// protocol to bind a pairing transcript to a specific device, so a
    /// man-in-the-middle relaying messages between two honest devices can't
    /// substitute a different identity partway through.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        self.signing_key.sign(message).to_bytes()
    }

    /// Build a self-signed X.509 certificate carrying this device's public
    /// key, for use as the TLS certificate in `sync::transport`. Rebuilt on
    /// demand rather than cached -- it is cheap, and this keeps the identity
    /// file itself the single source of truth for the keypair.
    pub fn build_certificate(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        use rustls_pki_types::PrivatePkcs8KeyDer;

        let pkcs8 = self
            .signing_key
            .to_pkcs8_der()
            .map_err(|_| Error::Internal("could not encode device key"))?;
        let pkcs8_der = PrivatePkcs8KeyDer::from(pkcs8.as_bytes());

        let key_pair =
            rcgen::KeyPair::from_pkcs8_der_and_sign_algo(&pkcs8_der, &rcgen::PKCS_ED25519)
                .map_err(|_| Error::Internal("could not build device certificate"))?;
        let params = rcgen::CertificateParams::new(vec!["envryn-device".to_string()])
            .map_err(|_| Error::Internal("could not build device certificate"))?;
        let cert = params
            .self_signed(&key_pair)
            .map_err(|_| Error::Internal("could not build device certificate"))?;

        Ok((cert.der().to_vec(), pkcs8.as_bytes().to_vec()))
    }
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reloads_identically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("identity.json");

        let first = DeviceIdentity::load_or_create(&path).unwrap();
        let fp1 = first.fingerprint();
        let id1 = first.device_id.clone();
        drop(first);

        let second = DeviceIdentity::load_or_create(&path).unwrap();
        assert_eq!(second.device_id, id1);
        assert_eq!(second.fingerprint(), fp1);
    }

    #[test]
    fn two_installations_get_different_identities() {
        let dir = tempfile::tempdir().unwrap();
        let a = DeviceIdentity::load_or_create(&dir.path().join("a.json")).unwrap();
        let b = DeviceIdentity::load_or_create(&dir.path().join("b.json")).unwrap();
        assert_ne!(a.device_id, b.device_id);
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn certificate_carries_the_same_public_key() {
        let dir = tempfile::tempdir().unwrap();
        let identity = DeviceIdentity::load_or_create(&dir.path().join("id.json")).unwrap();
        let (cert_der, _key_der) = identity.build_certificate().unwrap();

        let (_, parsed) = x509_parser::parse_x509_certificate(&cert_der).unwrap();
        let spki_bytes = parsed
            .tbs_certificate
            .subject_pki
            .subject_public_key
            .data
            .as_ref();
        assert_eq!(spki_bytes, identity.verifying_key().as_bytes());

        // And hashing that extracted key must match our own fingerprint --
        // this is the property sync::transport's verifier depends on.
        assert_eq!(
            Fingerprint::of_raw_bytes(spki_bytes),
            identity.fingerprint()
        );
    }

    #[test]
    fn fingerprint_hex_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let identity = DeviceIdentity::load_or_create(&dir.path().join("id.json")).unwrap();
        let fp = identity.fingerprint();
        assert_eq!(Fingerprint::from_hex(&fp.to_hex()).unwrap(), fp);
    }

    #[test]
    fn corrupt_identity_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("id.json");
        std::fs::write(&path, b"not json").unwrap();
        assert!(DeviceIdentity::load_or_create(&path).is_err());
    }

    /// **The actual regression.** Every sync/pairing command loads the
    /// device identity before doing anything else, and until this was fixed,
    /// that path unconditionally sealed/unsealed via DPAPI and failed with
    /// `Error::PlatformUnavailable` ("platform feature unavailable") on any
    /// non-Windows platform -- so pairing was entirely unusable on Android,
    /// confirmed from a real screenshot of the "Join an existing vault"
    /// screen showing exactly that message under the pairing-code field.
    /// `seal: false` is what `load_or_create` now supplies whenever
    /// `platform::dpapi_available()` is false, which is every build on
    /// Android since no non-Windows DPAPI equivalent is implemented yet.
    #[test]
    fn an_unsealed_identity_creates_and_reloads_identically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("id.json");

        let first = DeviceIdentity::load_or_create_with_sealing(&path, false).unwrap();
        let fp1 = first.fingerprint();
        let id1 = first.device_id.clone();
        drop(first);

        let second = DeviceIdentity::load_or_create_with_sealing(&path, false).unwrap();
        assert_eq!(second.device_id, id1);
        assert_eq!(second.fingerprint(), fp1);
    }

    /// The exact failure this regression test replaces: creating unsealed
    /// but then trying to reload as if the platform *did* have DPAPI (or
    /// vice versa) must not silently succeed with the wrong key material --
    /// `dpapi_unprotect` on plain, unsealed bytes must fail cleanly rather
    /// than fabricate a plausible-looking but wrong signing key.
    #[cfg(windows)]
    #[test]
    fn unsealed_bytes_are_not_mistaken_for_sealed_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("id.json");
        DeviceIdentity::load_or_create_with_sealing(&path, false).unwrap();

        // Force the `sealed` flag on without actually sealing the bytes --
        // simulating a corrupted or hand-edited file, not a real code path.
        let raw = std::fs::read(&path).unwrap();
        let mut file: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        file["sealed"] = serde_json::Value::Bool(true);
        std::fs::write(&path, serde_json::to_vec(&file).unwrap()).unwrap();

        assert!(DeviceIdentity::load_or_create(&path).is_err());
    }

    /// An identity file written before this fix has no `sealed` field at
    /// all. It must still load -- and, critically, must be treated as
    /// sealed (matching what every pre-fix file actually is), not silently
    /// reinterpreted as raw key bytes.
    #[test]
    fn a_pre_fix_identity_file_with_no_sealed_field_defaults_to_sealed() {
        let value = serde_json::json!({
            "device_id": "pre-fix-device",
            "sealed_secret_key": vec![1u8; 32],
            "public_key": vec![2u8; 32],
            "created_ms": 0,
        });
        let file: IdentityFile = serde_json::from_value(value).unwrap();
        assert!(file.sealed, "a missing `sealed` field must default to true");
    }
}
