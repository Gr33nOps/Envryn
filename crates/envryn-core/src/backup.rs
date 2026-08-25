//! Encrypted backup format.
//!
//! A backup is restorable using only the backup password. It does not depend
//! on the source vault's VMK, master password, or device identity in any way
//! -- see docs/CRYPTOGRAPHY.md section 8.
//!
//! **Backups are data-only by design.** Restoring one always creates a *new*
//! vault, with a master password chosen at restore time, and every record is
//! re-encrypted under that vault's own fresh VMK. A backup file therefore
//! never carries any of the source vault's key material, and restoring one
//! can never result in two live vault files silently sharing a VMK. This is
//! a deliberate simplification over "restore this exact vault byte-for-byte"
//! -- multi-device continuity is what device pairing (Phase 2) is for; a
//! backup's job is disaster recovery of data, not vault identity.
//!
//! ```text
//! header (plaintext, authenticated as AAD):
//!     magic "ENVRYNBK", format_version, KDF params, salt
//! body:
//!     XChaCha20-Poly1305( HKDF(Argon2id(backup_password, salt), "envryn/v1/backup") )
//! ```
//!
//! The header is plaintext because the KDF parameters it carries are needed
//! *before* any key exists to decrypt anything with -- exactly the same
//! constraint `vault_meta` is under, for the same reason. It is authenticated
//! as AAD so a header cannot be spliced from one backup file onto another
//! file's encrypted body (for example, pasting weaker KDF parameters in front
//! of a body that was actually sealed under stronger ones).

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::crypto::aead::{self, Sealed};
use crate::crypto::kdf::{self, KdfParams, SALT_LEN};
use crate::error::{Error, Result};
use crate::model::SecretRecord;

const MAGIC: &[u8; 8] = b"ENVRYNBK";
const LENGTH_PREFIX_LEN: usize = 4;

/// Bump only alongside a change to this format. An unrecognised version is a
/// clean refusal, never a best-effort parse -- see docs/CRYPTOGRAPHY.md
/// section 10.
pub const FORMAT_VERSION: u16 = 1;

/// The HKDF domain string for the backup body key. Scoped to this module
/// deliberately: it looks identical to a VMK-derived subkey label but is
/// applied to completely different input keying material (an
/// Argon2id-derived key, never the VMK), so it has exactly one owner.
const INFO_BACKUP_BODY: &[u8] = b"envryn/v1/backup";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Header {
    format_version: u16,
    kdf: KdfParams,
    salt: [u8; SALT_LEN],
}

fn header_aad(header: &Header) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(header)?)
}

fn derive_body_key(
    password: &Zeroizing<String>,
    salt: &[u8; SALT_LEN],
    params: KdfParams,
) -> Result<crate::crypto::keys::SymmetricKey> {
    let kek = kdf::derive_kek(password, salt, params)?;
    kek.derive_subkey(INFO_BACKUP_BODY)
}

/// Produce an encrypted backup of `records`, protected by `password`.
///
/// `records` are the full plaintext records -- this is one of exactly two
/// places in Envryn that ever holds every secret value at once (the other is
/// [`crate::vault::Vault::export_all`], which is this function's only
/// caller). Nothing here is a bulk *decryption* of anything beyond what an
/// already-unlocked vault already holds in memory; it is a bulk
/// *re-encryption* into an independently-keyed file.
pub fn create(records: &[SecretRecord], password: &Zeroizing<String>) -> Result<Vec<u8>> {
    let salt = kdf::generate_salt()?;
    let params = kdf::calibrate(700);
    let header = Header {
        format_version: FORMAT_VERSION,
        kdf: params,
        salt,
    };
    let aad = header_aad(&header)?;

    let key = derive_body_key(password, &salt, params)?;
    let plaintext = Zeroizing::new(serde_json::to_vec(records)?);
    let sealed = aead::seal(&key, &plaintext, &aad)?;

    let header_bytes = serde_json::to_vec(&header)?;
    let mut out = Vec::with_capacity(
        MAGIC.len() + LENGTH_PREFIX_LEN + header_bytes.len() + sealed.as_bytes().len(),
    );
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(sealed.as_bytes());
    Ok(out)
}

/// Recover the records from a backup produced by [`create`].
///
/// A wrong password, a corrupt file, and a file that is not an Envryn backup
/// at all all fail here -- the first as [`Error::AuthenticationFailed`], the
/// rest as [`Error::InvalidInput`], since "this is not a backup file" is not
/// a secret worth protecting the way a password guess is.
pub fn restore(file: &[u8], password: &Zeroizing<String>) -> Result<Vec<SecretRecord>> {
    if file.get(..MAGIC.len()) != Some(MAGIC.as_slice())
        || file.len() < MAGIC.len() + LENGTH_PREFIX_LEN
    {
        return Err(Error::InvalidInput("this file is not an Envryn backup"));
    }

    let mut offset = MAGIC.len();
    let header_len = u32::from_le_bytes(
        file.get(offset..offset + LENGTH_PREFIX_LEN)
            .and_then(|b| b.try_into().ok())
            .ok_or(Error::InvalidInput("the backup file is truncated"))?,
    ) as usize;
    offset += LENGTH_PREFIX_LEN;

    let header_bytes = file
        .get(offset..offset + header_len)
        .ok_or(Error::InvalidInput("the backup file is truncated"))?;
    let header: Header = serde_json::from_slice(header_bytes)
        .map_err(|_| Error::InvalidInput("the backup header is corrupt"))?;
    offset += header_len;

    if header.format_version > FORMAT_VERSION {
        return Err(Error::UnsupportedVersion {
            found: u32::from(header.format_version),
            supported: u32::from(FORMAT_VERSION),
        });
    }

    let body = file
        .get(offset..)
        .ok_or(Error::InvalidInput("the backup file is truncated"))?;
    let sealed = Sealed::from_bytes(body.to_vec())?;

    let key = derive_body_key(password, &header.salt, header.kdf)?;
    let aad = header_aad(&header)?;
    let plaintext = aead::open(&key, &sealed, &aad).map_err(|_| Error::AuthenticationFailed)?;

    serde_json::from_slice(&plaintext)
        .map_err(|_| Error::InvalidInput("the backup contents are corrupt"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Environment, SecretId, SecretPayload};

    fn pw(s: &str) -> Zeroizing<String> {
        Zeroizing::new(s.to_string())
    }

    fn sample() -> Vec<SecretRecord> {
        vec![SecretRecord {
            id: SecretId::new(),
            name: "GROQ_API_KEY".into(),
            project: "Rescripto".into(),
            environment: Environment::Development,
            payload: SecretPayload::ApiKey {
                value: "gsk_backup_test_value".into(),
            },
            notes: None,
            tags: vec![],
            provider: None,
            created_ms: 1,
            updated_ms: 2,
            rotated_ms: None,
        }]
    }

    #[test]
    fn round_trip() {
        let records = sample();
        let file = create(&records, &pw("backup-password")).unwrap();
        let recovered = restore(&file, &pw("backup-password")).unwrap();
        assert_eq!(recovered, records);
    }

    #[test]
    fn wrong_password_fails_as_authentication_failure() {
        let file = create(&sample(), &pw("right")).unwrap();
        assert!(matches!(
            restore(&file, &pw("wrong")),
            Err(Error::AuthenticationFailed)
        ));
    }

    #[test]
    fn plaintext_does_not_appear_in_the_file() {
        let file = create(&sample(), &pw("backup-password")).unwrap();
        let needle = b"gsk_backup_test_value";
        assert!(!file.windows(needle.len()).any(|w| w == needle));
    }

    #[test]
    fn garbage_input_is_rejected_cleanly() {
        assert!(matches!(
            restore(b"not a backup file at all", &pw("x")),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            restore(b"", &pw("x")),
            Err(Error::InvalidInput(_))
        ));
    }

    #[test]
    fn truncated_file_is_rejected_cleanly() {
        let mut file = create(&sample(), &pw("p")).unwrap();
        file.truncate(file.len() - 5);
        assert!(restore(&file, &pw("p")).is_err());
    }

    #[test]
    fn tampered_header_is_detected() {
        let file = create(&sample(), &pw("p")).unwrap();
        let mut tampered = file.clone();
        // Flip a byte inside the header (past magic + length prefix).
        let idx = MAGIC.len() + LENGTH_PREFIX_LEN + 2;
        tampered[idx] ^= 0xFF;
        // Either the header no longer parses, or -- if it still happens to
        // parse -- the AAD binding must make decryption fail. Either is an
        // acceptable rejection; silently succeeding is not.
        assert!(restore(&tampered, &pw("p")).is_err());
    }

    #[test]
    fn future_format_version_is_refused_not_guessed() {
        let mut file = create(&sample(), &pw("p")).unwrap();
        // The version field is the first two bytes of the (JSON) header.
        // Rather than hand-craft JSON, corrupt-and-reparse would be fragile;
        // instead construct a header directly and splice it in.
        let header = Header {
            format_version: FORMAT_VERSION + 1,
            kdf: KdfParams::MINIMUM,
            salt: [0u8; SALT_LEN],
        };
        let header_bytes = serde_json::to_vec(&header).unwrap();
        let mut spliced = Vec::new();
        spliced.extend_from_slice(MAGIC);
        spliced.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        spliced.extend_from_slice(&header_bytes);
        // Body content is irrelevant -- the version check must reject before
        // any attempt to decrypt it.
        spliced.extend_from_slice(&file.split_off(file.len().saturating_sub(16)));

        assert!(matches!(
            restore(&spliced, &pw("p")),
            Err(Error::UnsupportedVersion { .. })
        ));
    }
}
