//! Database schema and migrations.
//!
//! **Rows are opaque.** The `secrets` table stores an id, sync bookkeeping,
//! a keyed fingerprint, and one sealed blob. Names, projects, environments and
//! tags live *inside* the blob, not in columns.
//!
//! This departs from the common design of "plaintext metadata in an encrypted
//! database", and it is deliberate. Metadata leaks a great deal -- project
//! names and environment labels map out someone's infrastructure -- and
//! relying on whole-file encryption to protect it means the protection
//! disappears the moment the file is copied out of a running system. Since
//! search is performed against an in-memory index while unlocked
//! (docs/CRYPTOGRAPHY.md section 4), no query ever needed those columns.
//!
//! The cost is that SQL cannot filter on name or project. Nothing in Envryn
//! asks it to.

use rusqlite::Connection;

use crate::error::{Error, Result};

/// Bump only alongside a migration. A vault whose schema is newer than the
/// running build is refused rather than opened optimistically -- see
/// docs/CRYPTOGRAPHY.md section 10.
pub const SCHEMA_VERSION: i64 = 1;

/// Record format version, embedded in each record's AAD so a ciphertext
/// cannot be rolled back to an earlier format undetected.
pub const RECORD_VERSION: i64 = 1;

pub fn initialise(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = FULL;
         PRAGMA foreign_keys = ON;
         PRAGMA secure_delete = ON;",
    )?;

    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if current > SCHEMA_VERSION {
        return Err(Error::UnsupportedVersion {
            found: current as u32,
            supported: SCHEMA_VERSION as u32,
        });
    }

    if current < 1 {
        migrate_to_v1(conn)?;
    }

    Ok(())
}

fn migrate_to_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "BEGIN;

         -- Unencrypted by necessity: these are needed before any key exists.
         -- Integrity of the KDF parameters is enforced by range-checking at
         -- use (crypto::kdf), not by encryption.
         CREATE TABLE vault_meta (
             key   TEXT PRIMARY KEY NOT NULL,
             value BLOB NOT NULL
         );

         -- One sealed blob per record. Everything human-meaningful is inside
         -- `sealed`; the remaining columns exist only so that sync can order
         -- and reconcile records without decrypting them.
         CREATE TABLE secrets (
             id             TEXT PRIMARY KEY NOT NULL,
             record_version INTEGER NOT NULL,
             sealed         BLOB NOT NULL,
             fingerprint    BLOB,
             created_ms     INTEGER NOT NULL,
             updated_ms     INTEGER NOT NULL,
             hlc_counter    INTEGER NOT NULL DEFAULT 0,
             hlc_device     TEXT NOT NULL DEFAULT '',
             deleted        INTEGER NOT NULL DEFAULT 0
         );

         -- Duplicate detection scans by fingerprint; partial index keeps
         -- tombstones and unfingerprinted notes out of it.
         CREATE INDEX idx_secrets_fingerprint
             ON secrets(fingerprint)
             WHERE fingerprint IS NOT NULL AND deleted = 0;

         CREATE INDEX idx_secrets_updated ON secrets(updated_ms);

         PRAGMA user_version = 1;
         COMMIT;",
    )?;
    Ok(())
}

/// Keys used in `vault_meta`.
pub mod meta_keys {
    pub const CRYPTO_VERSION: &str = "crypto_version";
    pub const KDF_PARAMS: &str = "kdf_params";
    pub const KDF_SALT: &str = "kdf_salt";
    pub const WRAPPED_VMK: &str = "wrapped_vmk_";
    pub const DEVICE_ID: &str = "device_id";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        initialise(&c).unwrap();
        c
    }

    #[test]
    fn initialises_to_current_version() {
        let c = conn();
        let v: i64 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn initialise_is_idempotent() {
        let c = conn();
        initialise(&c).unwrap();
        initialise(&c).unwrap();
        let v: i64 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    /// A vault written by a newer build must be refused, not best-effort
    /// parsed. Guessing wrong about a ciphertext layout corrupts silently.
    #[test]
    fn future_schema_is_refused() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA user_version = 999;").unwrap();
        assert!(matches!(
            initialise(&c),
            Err(Error::UnsupportedVersion { found: 999, .. })
        ));
    }

    /// The schema must not grow a plaintext column for anything meaningful.
    /// If someone adds `name TEXT` for convenience, this fails.
    #[test]
    fn secrets_table_exposes_no_plaintext_metadata() {
        let c = conn();
        let mut stmt = c.prepare("PRAGMA table_info(secrets)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let permitted = [
            "id",
            "record_version",
            "sealed",
            "fingerprint",
            "created_ms",
            "updated_ms",
            "hlc_counter",
            "hlc_device",
            "deleted",
        ];
        for col in &cols {
            assert!(
                permitted.contains(&col.as_str()),
                "unexpected column `{col}` in secrets -- metadata belongs inside the sealed blob"
            );
        }
    }
}
