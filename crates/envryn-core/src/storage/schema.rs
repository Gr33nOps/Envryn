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
pub const SCHEMA_VERSION: i64 = 3;

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
    if current < 2 {
        migrate_to_v2(conn)?;
    }
    if current < 3 {
        migrate_to_v3(conn)?;
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

fn migrate_to_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "BEGIN;

         -- Paired devices this vault will accept a sync connection from.
         -- Opaque, like `secrets`: name and pairing history live inside
         -- `sealed`, under the same Record Key. `fingerprint` is the one
         -- exception -- it is not a secret (it is read aloud and compared on
         -- screen during pairing, the same role an SSH host key fingerprint
         -- plays) and `sync::transport`'s TLS verifier needs the whole
         -- trusted set in memory to check every incoming handshake, which is
         -- simpler and cheaper to build from a plain column than by
         -- unsealing every row up front for a value that was never secret.
         CREATE TABLE trusted_devices (
             device_id   TEXT PRIMARY KEY NOT NULL,
             fingerprint BLOB NOT NULL,
             sealed      BLOB NOT NULL,
             paired_ms   INTEGER NOT NULL
         );

         CREATE UNIQUE INDEX idx_trusted_devices_fingerprint
             ON trusted_devices(fingerprint);

         PRAGMA user_version = 2;
         COMMIT;",
    )?;
    Ok(())
}

fn migrate_to_v3(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "BEGIN;

         -- Per-record causal history (storage::version_vector::VersionVector,
         -- JSON-encoded), used only to tell a genuine concurrent edit apart
         -- from a peer that was simply behind -- see
         -- storage::Store::upsert_from_sync. Not a secret: it reveals exactly
         -- the same shape of information hlc_counter/hlc_device/updated_ms
         -- already do in plaintext (which devices touched this record, and
         -- when), just per contributing device instead of only the latest.
         ALTER TABLE secrets ADD COLUMN version_vector TEXT NOT NULL DEFAULT '{}';

         -- The losing side of a genuine concurrent edit (INV-109). Opaque
         -- like `secrets`: nothing human-meaningful outside `sealed`.
         -- Populated only when `upsert_from_sync` detects two branches that
         -- neither vector dominates; the winner (by Hlc tiebreak) becomes the
         -- live `secrets` row, and the loser lands here instead of being
         -- silently discarded.
         CREATE TABLE record_conflicts (
             id             TEXT PRIMARY KEY NOT NULL,
             secret_id      TEXT NOT NULL,
             record_version INTEGER NOT NULL,
             sealed         BLOB NOT NULL,
             fingerprint    BLOB,
             hlc_wall_ms    INTEGER NOT NULL,
             hlc_counter    INTEGER NOT NULL,
             hlc_device     TEXT NOT NULL,
             deleted        INTEGER NOT NULL DEFAULT 0,
             created_ms     INTEGER NOT NULL
         );

         CREATE INDEX idx_record_conflicts_secret ON record_conflicts(secret_id);

         PRAGMA user_version = 3;
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
    /// The platform-protected (DPAPI) blob that decrypts to the platform slot's
    /// wrapping key. Distinct from `WRAPPED_VMK + "platform"`, which is the VMK
    /// itself wrapped *under* that recovered key -- two layers, so DPAPI never
    /// touches the VMK directly and the AEAD wrap path stays the only thing
    /// that ever handles it.
    pub const PLATFORM_KEY_BLOB: &str = "platform_key_blob";
    /// Whether unlocking via the platform slot must first pass the Windows
    /// Hello gate (`platform::hello_verify`) -- a UX/authentication
    /// requirement layered in front of the DPAPI unwrap, not a different key
    /// derivation. Presence of this key (any value) means "on"; absence
    /// means "off," matching `PLATFORM_KEY_BLOB`'s own presence-as-boolean
    /// convention.
    pub const HELLO_GATE_ENABLED: &str = "hello_gate_enabled";
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
            "version_vector",
        ];
        for col in &cols {
            assert!(
                permitted.contains(&col.as_str()),
                "unexpected column `{col}` in secrets -- metadata belongs inside the sealed blob"
            );
        }
    }

    /// Same rule as `secrets`: the losing side of a conflict is opaque too.
    #[test]
    fn record_conflicts_exposes_no_plaintext_metadata() {
        let c = conn();
        let mut stmt = c.prepare("PRAGMA table_info(record_conflicts)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let permitted = [
            "id",
            "secret_id",
            "record_version",
            "sealed",
            "fingerprint",
            "hlc_wall_ms",
            "hlc_counter",
            "hlc_device",
            "deleted",
            "created_ms",
        ];
        for col in &cols {
            assert!(
                permitted.contains(&col.as_str()),
                "unexpected column `{col}` in record_conflicts -- metadata belongs inside the sealed blob"
            );
        }
    }

    /// A v2 database (pre-conflict-tracking) must migrate forward cleanly,
    /// not just a fresh-created one -- the actual upgrade path a real user's
    /// existing vault would go through.
    #[test]
    fn a_v2_database_migrates_to_v3_cleanly() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA journal_mode = WAL;").unwrap();
        migrate_to_v1(&c).unwrap();
        migrate_to_v2(&c).unwrap();
        initialise(&c).unwrap();

        let v: i64 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);

        c.execute(
            "INSERT INTO secrets (id, record_version, sealed, created_ms, updated_ms) \
             VALUES ('x', 1, X'00', 0, 0)",
            [],
        )
        .unwrap();
        let vector: String = c
            .query_row(
                "SELECT version_vector FROM secrets WHERE id = 'x'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(vector, "{}");
    }

    /// `trusted_devices` may expose `fingerprint` (not a secret -- see the
    /// migration's doc comment) but nothing else meaningful in plaintext.
    #[test]
    fn trusted_devices_exposes_only_the_fingerprint() {
        let c = conn();
        let mut stmt = c.prepare("PRAGMA table_info(trusted_devices)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let permitted = ["device_id", "fingerprint", "sealed", "paired_ms"];
        for col in &cols {
            assert!(
                permitted.contains(&col.as_str()),
                "unexpected column `{col}` in trusted_devices -- device names and history belong inside the sealed blob"
            );
        }
    }
}
