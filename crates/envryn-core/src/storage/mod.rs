//! Persistence.
//!
//! This layer moves opaque bytes. It never holds a key, never seals, and never
//! opens -- encryption happens one level up, in [`crate::vault`]. Keeping the
//! boundary there means a bug in SQL handling cannot produce a plaintext write,
//! because this module has nothing to write in plaintext.

pub mod hlc;
pub mod schema;

use rusqlite::{params, Connection, OptionalExtension};

use crate::crypto::fingerprint::Fingerprint;
use crate::error::{Error, Result};
use crate::model::SecretId;

pub use hlc::Hlc;
pub use schema::{meta_keys, RECORD_VERSION, SCHEMA_VERSION};

/// A stored row, still sealed.
///
/// There is deliberately no separate `updated_ms` field: `hlc.wall_ms` *is*
/// the last-modified timestamp (the `updated_ms` database column stores it
/// directly). An earlier version of this struct carried both, which meant
/// every write site had to remember to keep them equal -- exactly the kind
/// of two-sources-of-truth bug that is easy to introduce and easy to miss in
/// review. One field removes the possibility.
pub struct StoredRecord {
    pub id: SecretId,
    pub record_version: i64,
    pub sealed: Vec<u8>,
    pub fingerprint: Option<Fingerprint>,
    pub created_ms: i64,
    pub hlc: Hlc,
    pub deleted: bool,
}

impl StoredRecord {
    pub fn updated_ms(&self) -> i64 {
        self.hlc.wall_ms
    }
}

/// A record's identity and version for sync's manifest exchange -- everything
/// needed to decide whether a peer is ahead, without the ciphertext payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestEntry {
    pub id: SecretId,
    pub hlc: Hlc,
    pub deleted: bool,
}

/// A raw `trusted_devices` row, still sealed. `crate::vault` unseals `sealed`
/// into a `model::TrustedDevice`; this layer never does.
pub struct TrustedDeviceRow {
    pub device_id: String,
    pub fingerprint: Vec<u8>,
    pub sealed: Vec<u8>,
    pub paired_ms: i64,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        schema::initialise(&conn)?;
        Ok(Self { conn })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::initialise(&conn)?;
        Ok(Self { conn })
    }

    // --- vault_meta ---------------------------------------------------------

    pub fn get_meta(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM vault_meta WHERE key = ?1",
                params![key],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?)
    }

    pub fn set_meta(&self, key: &str, value: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT INTO vault_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_meta(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM vault_meta WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn is_initialised(&self) -> Result<bool> {
        Ok(self.get_meta(meta_keys::CRYPTO_VERSION)?.is_some())
    }

    // --- secrets ------------------------------------------------------------

    pub fn insert(&self, record: &StoredRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO secrets
                (id, record_version, sealed, fingerprint, created_ms, updated_ms,
                 hlc_counter, hlc_device, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id.to_string(),
                record.record_version,
                record.sealed,
                record.fingerprint.map(|f| f.as_bytes().to_vec()),
                record.created_ms,
                record.hlc.wall_ms,
                record.hlc.counter,
                record.hlc.device_id.to_string(),
                record.deleted,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, record: &StoredRecord) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE secrets
                SET sealed = ?2, fingerprint = ?3, updated_ms = ?4, record_version = ?5,
                    hlc_counter = ?6, hlc_device = ?7
              WHERE id = ?1 AND deleted = 0",
            params![
                record.id.to_string(),
                record.sealed,
                record.fingerprint.map(|f| f.as_bytes().to_vec()),
                record.hlc.wall_ms,
                record.record_version,
                record.hlc.counter,
                record.hlc.device_id.to_string(),
            ],
        )?;
        if changed == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn get(&self, id: SecretId) -> Result<Option<StoredRecord>> {
        self.conn
            .query_row(
                "SELECT id, record_version, sealed, fingerprint, created_ms, updated_ms,
                        hlc_counter, hlc_device, deleted
                   FROM secrets WHERE id = ?1 AND deleted = 0",
                params![id.to_string()],
                row_to_record,
            )
            .optional()?
            .transpose()
    }

    /// As [`Store::get`], but also returns a tombstoned row. Used only by
    /// `sync::protocol` when a peer explicitly requests a record by id --
    /// sync must be able to answer "this was deleted" (an empty `sealed`
    /// blob, `deleted: true`) so the deletion itself propagates, not only
    /// live content.
    pub fn get_including_deleted(&self, id: SecretId) -> Result<Option<StoredRecord>> {
        self.conn
            .query_row(
                "SELECT id, record_version, sealed, fingerprint, created_ms, updated_ms,
                        hlc_counter, hlc_device, deleted
                   FROM secrets WHERE id = ?1",
                params![id.to_string()],
                row_to_record,
            )
            .optional()?
            .transpose()
    }

    pub fn list(&self) -> Result<Vec<StoredRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, record_version, sealed, fingerprint, created_ms, updated_ms,
                    hlc_counter, hlc_device, deleted
               FROM secrets WHERE deleted = 0 ORDER BY updated_ms DESC",
        )?;
        let rows = stmt.query_map([], row_to_record)?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// Every record's id, HLC, and tombstone state -- deleted rows included,
    /// since sync must propagate deletions, not only live records
    /// (INV-110). Never the ciphertext: a manifest is what the two sides
    /// exchange *before* deciding what to actually transfer.
    pub fn list_manifest(&self) -> Result<Vec<ManifestEntry>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, updated_ms, hlc_counter, hlc_device, deleted FROM secrets")?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let wall_ms: i64 = row.get(1)?;
            let counter: u32 = row.get(2)?;
            let device_str: String = row.get(3)?;
            let deleted: bool = row.get(4)?;
            Ok((id, wall_ms, counter, device_str, deleted))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (id, wall_ms, counter, device_str, deleted) = row?;
            out.push(ManifestEntry {
                id: SecretId::parse(&id)?,
                hlc: Hlc {
                    wall_ms,
                    counter,
                    device_id: device_str.parse().unwrap_or(0),
                },
                deleted,
            });
        }
        Ok(out)
    }

    /// Apply an incoming synced row. Only writes if the incoming HLC is
    /// strictly newer than what is currently stored (or the row does not
    /// exist locally yet) -- last-writer-wins, decided once, here, so every
    /// caller in `sync::protocol` gets the same rule applied the same way.
    /// Returns whether the row was actually written.
    pub fn upsert_from_sync(&self, record: &StoredRecord) -> Result<bool> {
        let existing: Option<(i64, u32, String)> = self
            .conn
            .query_row(
                "SELECT updated_ms, hlc_counter, hlc_device FROM secrets WHERE id = ?1",
                params![record.id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;

        if let Some((wall_ms, counter, device_str)) = existing {
            let existing_hlc = Hlc {
                wall_ms,
                counter,
                device_id: device_str.parse().unwrap_or(0),
            };
            if record.hlc <= existing_hlc {
                return Ok(false);
            }
            self.conn.execute(
                "UPDATE secrets
                    SET sealed = ?2, fingerprint = ?3, updated_ms = ?4, record_version = ?5,
                        hlc_counter = ?6, hlc_device = ?7, deleted = ?8
                  WHERE id = ?1",
                params![
                    record.id.to_string(),
                    record.sealed,
                    record.fingerprint.map(|f| f.as_bytes().to_vec()),
                    record.hlc.wall_ms,
                    record.record_version,
                    record.hlc.counter,
                    record.hlc.device_id.to_string(),
                    record.deleted,
                ],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO secrets
                    (id, record_version, sealed, fingerprint, created_ms, updated_ms,
                     hlc_counter, hlc_device, deleted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    record.id.to_string(),
                    record.record_version,
                    record.sealed,
                    record.fingerprint.map(|f| f.as_bytes().to_vec()),
                    record.created_ms,
                    record.hlc.wall_ms,
                    record.hlc.counter,
                    record.hlc.device_id.to_string(),
                    record.deleted,
                ],
            )?;
        }
        Ok(true)
    }

    /// Soft-delete.
    ///
    /// A tombstone rather than a row removal, because a deletion racing a sync
    /// against a peer that has not seen it would otherwise resurrect the
    /// record (INV-110). The sealed blob is cleared immediately so the
    /// ciphertext does not linger for the tombstone's retention window.
    pub fn soft_delete(&self, id: SecretId, hlc: Hlc) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE secrets
                SET deleted = 1, sealed = X'', fingerprint = NULL, updated_ms = ?2,
                    hlc_counter = ?3, hlc_device = ?4
              WHERE id = ?1 AND deleted = 0",
            params![
                id.to_string(),
                hlc.wall_ms,
                hlc.counter,
                hlc.device_id.to_string()
            ],
        )?;
        if changed == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Ids of live records sharing a fingerprint. Exact duplicate detection is
    /// deterministic and never involves the AI (docs/CRYPTOGRAPHY.md section 5).
    pub fn find_by_fingerprint(&self, fp: &Fingerprint) -> Result<Vec<SecretId>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM secrets WHERE fingerprint = ?1 AND deleted = 0")?;
        let rows = stmt.query_map(params![fp.as_bytes().to_vec()], |row| {
            row.get::<_, String>(0)
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(SecretId::parse(&row?)?);
        }
        Ok(out)
    }

    pub fn count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM secrets WHERE deleted = 0", [], |r| {
                r.get(0)
            })?)
    }

    /// Force WAL contents into the main database.
    ///
    /// Called on lock. Without it, plaintext-adjacent ciphertext and the
    /// database's own structure can sit in a `-wal` sidecar file that a naive
    /// backup or disk inspection treats as separate from the vault.
    pub fn checkpoint(&self) -> Result<()> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    // --- trusted_devices -----------------------------------------------------

    pub fn insert_trusted_device(
        &self,
        device_id: &str,
        fingerprint: &[u8],
        sealed: &[u8],
        paired_ms: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO trusted_devices (device_id, fingerprint, sealed, paired_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![device_id, fingerprint, sealed, paired_ms],
        )?;
        Ok(())
    }

    pub fn update_trusted_device_sealed(&self, device_id: &str, sealed: &[u8]) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE trusted_devices SET sealed = ?2 WHERE device_id = ?1",
            params![device_id, sealed],
        )?;
        if changed == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn list_trusted_devices(&self) -> Result<Vec<TrustedDeviceRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT device_id, fingerprint, sealed, paired_ms FROM trusted_devices")?;
        let rows = stmt.query_map([], |r| {
            Ok(TrustedDeviceRow {
                device_id: r.get(0)?,
                fingerprint: r.get(1)?,
                sealed: r.get(2)?,
                paired_ms: r.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// All trusted fingerprints, for building `sync::transport`'s
    /// `TrustedFingerprints` set. Kept separate from `list_trusted_devices`
    /// so the transport layer never needs to unseal anything just to build
    /// its verifier.
    pub fn list_trusted_fingerprints(&self) -> Result<Vec<Vec<u8>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT fingerprint FROM trusted_devices")?;
        let rows = stmt.query_map([], |r| r.get::<_, Vec<u8>>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Revoke a device. A row delete, not a soft delete -- unlike vault
    /// records, there is no sync reconciliation for `trusted_devices` itself
    /// (it is not something peers exchange), so there is no tombstone to
    /// preserve. The very next handshake attempt from this fingerprint fails
    /// at the TLS layer once the caller rebuilds `TrustedFingerprints` from
    /// this table (INV-104).
    pub fn revoke_trusted_device(&self, device_id: &str) -> Result<()> {
        let changed = self.conn.execute(
            "DELETE FROM trusted_devices WHERE device_id = ?1",
            params![device_id],
        )?;
        if changed == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<StoredRecord>> {
    let id_str: String = row.get(0)?;
    let fp_bytes: Option<Vec<u8>> = row.get(3)?;
    let updated_ms: i64 = row.get(5).unwrap_or(0);
    let hlc_counter: u32 = row.get(6).unwrap_or(0);
    let hlc_device: String = row.get(7).unwrap_or_default();
    let deleted: bool = row.get(8).unwrap_or(false);

    Ok((|| {
        let fingerprint = match fp_bytes {
            Some(bytes) => Some(Fingerprint::from_slice(&bytes)?),
            None => None,
        };
        Ok(StoredRecord {
            id: SecretId::parse(&id_str)?,
            record_version: row.get(1).unwrap_or(RECORD_VERSION),
            sealed: row.get(2).unwrap_or_default(),
            fingerprint,
            created_ms: row.get(4).unwrap_or(0),
            hlc: Hlc {
                wall_ms: updated_ms,
                counter: hlc_counter,
                device_id: hlc_device.parse().unwrap_or(0),
            },
            deleted,
        })
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::fingerprint::Fingerprint;

    fn record(sealed: &[u8]) -> StoredRecord {
        StoredRecord {
            id: SecretId::new(),
            record_version: RECORD_VERSION,
            sealed: sealed.to_vec(),
            fingerprint: Some(Fingerprint::from_bytes([3u8; 16])),
            created_ms: 100,
            hlc: Hlc {
                wall_ms: 100,
                counter: 0,
                device_id: 1,
            },
            deleted: false,
        }
    }

    #[test]
    fn insert_and_get_round_trip() {
        let store = Store::open_in_memory().unwrap();
        let rec = record(b"sealed-bytes");
        let id = rec.id;
        store.insert(&rec).unwrap();

        let got = store.get(id).unwrap().unwrap();
        assert_eq!(got.id, id);
        assert_eq!(got.sealed, b"sealed-bytes");
    }

    #[test]
    fn missing_record_returns_none() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.get(SecretId::new()).unwrap().is_none());
    }

    #[test]
    fn update_replaces_sealed_blob() {
        let store = Store::open_in_memory().unwrap();
        let mut rec = record(b"before");
        store.insert(&rec).unwrap();

        rec.sealed = b"after".to_vec();
        rec.hlc.wall_ms = 200;
        store.update(&rec).unwrap();

        let got = store.get(rec.id).unwrap().unwrap();
        assert_eq!(got.sealed, b"after");
        assert_eq!(got.updated_ms(), 200);
    }

    #[test]
    fn update_of_missing_record_errors() {
        let store = Store::open_in_memory().unwrap();
        assert!(matches!(store.update(&record(b"x")), Err(Error::NotFound)));
    }

    /// Deleting must clear the ciphertext immediately, not merely flag the
    /// row -- otherwise the sealed blob outlives the user's decision to
    /// destroy it for as long as the tombstone is retained.
    #[test]
    fn soft_delete_clears_the_ciphertext() {
        let store = Store::open_in_memory().unwrap();
        let rec = record(b"sensitive-ciphertext");
        let id = rec.id;
        store.insert(&rec).unwrap();

        store
            .soft_delete(
                id,
                Hlc {
                    wall_ms: 300,
                    counter: 0,
                    device_id: 0,
                },
            )
            .unwrap();

        assert!(store.get(id).unwrap().is_none());
        assert_eq!(store.count().unwrap(), 0);

        let sealed: Vec<u8> = store
            .conn
            .query_row(
                "SELECT sealed FROM secrets WHERE id = ?1",
                params![id.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(sealed.is_empty(), "ciphertext survived deletion");
    }

    /// The tombstone itself must remain, or a delete that races a sync will be
    /// undone by the peer re-sending the record.
    #[test]
    fn soft_delete_leaves_a_tombstone() {
        let store = Store::open_in_memory().unwrap();
        let rec = record(b"x");
        let id = rec.id;
        store.insert(&rec).unwrap();
        store
            .soft_delete(
                id,
                Hlc {
                    wall_ms: 300,
                    counter: 0,
                    device_id: 0,
                },
            )
            .unwrap();

        let rows: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM secrets", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn deleting_twice_errors() {
        let store = Store::open_in_memory().unwrap();
        let rec = record(b"x");
        let id = rec.id;
        store.insert(&rec).unwrap();
        store
            .soft_delete(
                id,
                Hlc {
                    wall_ms: 1,
                    counter: 0,
                    device_id: 0,
                },
            )
            .unwrap();
        assert!(matches!(
            store.soft_delete(
                id,
                Hlc {
                    wall_ms: 2,
                    counter: 0,
                    device_id: 0
                }
            ),
            Err(Error::NotFound)
        ));
    }

    #[test]
    fn list_excludes_deleted_and_sorts_by_recency() {
        let store = Store::open_in_memory().unwrap();
        let mut old = record(b"old");
        old.hlc.wall_ms = 10;
        let mut new = record(b"new");
        new.hlc.wall_ms = 20;
        let gone = record(b"gone");

        store.insert(&old).unwrap();
        store.insert(&new).unwrap();
        store.insert(&gone).unwrap();
        store
            .soft_delete(
                gone.id,
                Hlc {
                    wall_ms: 30,
                    counter: 0,
                    device_id: 0,
                },
            )
            .unwrap();

        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(
            listed[0].id, new.id,
            "most recently updated should sort first"
        );
        assert_eq!(listed[1].id, old.id);
    }

    #[test]
    fn finds_matching_fingerprints() {
        let store = Store::open_in_memory().unwrap();
        let fp = Fingerprint::from_bytes([9u8; 16]);

        let mut a = record(b"a");
        a.fingerprint = Some(fp);
        let mut b = record(b"b");
        b.fingerprint = Some(fp);
        let mut c = record(b"c");
        c.fingerprint = Some(Fingerprint::from_bytes([1u8; 16]));

        store.insert(&a).unwrap();
        store.insert(&b).unwrap();
        store.insert(&c).unwrap();

        let found = store.find_by_fingerprint(&fp).unwrap();
        assert_eq!(found.len(), 2);
        assert!(found.contains(&a.id) && found.contains(&b.id));
    }

    #[test]
    fn deleted_records_are_not_matched_by_fingerprint() {
        let store = Store::open_in_memory().unwrap();
        let fp = Fingerprint::from_bytes([9u8; 16]);
        let mut a = record(b"a");
        a.fingerprint = Some(fp);
        store.insert(&a).unwrap();
        store
            .soft_delete(
                a.id,
                Hlc {
                    wall_ms: 1,
                    counter: 0,
                    device_id: 0,
                },
            )
            .unwrap();

        assert!(store.find_by_fingerprint(&fp).unwrap().is_empty());
    }

    #[test]
    fn meta_round_trips_and_overwrites() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.get_meta("k").unwrap().is_none());
        assert!(!store.is_initialised().unwrap());

        store.set_meta("k", b"v1").unwrap();
        assert_eq!(store.get_meta("k").unwrap().unwrap(), b"v1");

        store.set_meta("k", b"v2").unwrap();
        assert_eq!(store.get_meta("k").unwrap().unwrap(), b"v2");

        store.delete_meta("k").unwrap();
        assert!(store.get_meta("k").unwrap().is_none());
    }

    #[test]
    fn records_with_no_fingerprint_are_permitted() {
        let store = Store::open_in_memory().unwrap();
        let mut rec = record(b"note");
        rec.fingerprint = None;
        store.insert(&rec).unwrap();

        let got = store.get(rec.id).unwrap().unwrap();
        assert!(got.fingerprint.is_none());
    }

    // --- sync: manifest and last-writer-wins upsert ------------------------

    #[test]
    fn manifest_includes_tombstones() {
        let store = Store::open_in_memory().unwrap();
        let live = record(b"live");
        let gone = record(b"gone");
        store.insert(&live).unwrap();
        store.insert(&gone).unwrap();
        store
            .soft_delete(
                gone.id,
                Hlc {
                    wall_ms: 500,
                    counter: 0,
                    device_id: 1,
                },
            )
            .unwrap();

        let manifest = store.list_manifest().unwrap();
        assert_eq!(
            manifest.len(),
            2,
            "a manifest must include tombstones, not only live rows"
        );
        let gone_entry = manifest.iter().find(|e| e.id == gone.id).unwrap();
        assert!(gone_entry.deleted);
    }

    #[test]
    fn upsert_from_sync_inserts_an_unknown_record() {
        let store = Store::open_in_memory().unwrap();
        let rec = record(b"from-peer");
        assert!(store.upsert_from_sync(&rec).unwrap());
        assert_eq!(store.get(rec.id).unwrap().unwrap().sealed, b"from-peer");
    }

    /// The core sync guarantee: an incoming write with an older HLC than what
    /// is already stored must be silently ignored, not overwrite newer local
    /// data.
    #[test]
    fn upsert_from_sync_rejects_an_older_write() {
        let store = Store::open_in_memory().unwrap();
        let mut newer = record(b"newer-local");
        newer.hlc = Hlc {
            wall_ms: 200,
            counter: 0,
            device_id: 1,
        };
        store.insert(&newer).unwrap();

        let mut older_incoming = record(b"older-from-peer");
        older_incoming.id = newer.id;
        older_incoming.hlc = Hlc {
            wall_ms: 100,
            counter: 0,
            device_id: 2,
        };

        assert!(!store.upsert_from_sync(&older_incoming).unwrap());
        assert_eq!(store.get(newer.id).unwrap().unwrap().sealed, b"newer-local");
    }

    #[test]
    fn upsert_from_sync_applies_a_newer_write() {
        let store = Store::open_in_memory().unwrap();
        let mut older = record(b"older-local");
        older.hlc = Hlc {
            wall_ms: 100,
            counter: 0,
            device_id: 1,
        };
        store.insert(&older).unwrap();

        let mut newer_incoming = record(b"newer-from-peer");
        newer_incoming.id = older.id;
        newer_incoming.hlc = Hlc {
            wall_ms: 200,
            counter: 0,
            device_id: 2,
        };

        assert!(store.upsert_from_sync(&newer_incoming).unwrap());
        assert_eq!(
            store.get(older.id).unwrap().unwrap().sealed,
            b"newer-from-peer"
        );
    }

    /// A tie on `(wall_ms, counter)` must resolve identically everywhere --
    /// tested here by confirming the device-id tiebreak actually participates
    /// in the "is this newer" decision, not only wall clock and counter.
    #[test]
    fn upsert_from_sync_tiebreaks_on_device_id() {
        let store = Store::open_in_memory().unwrap();
        let mut low_device = record(b"low-device");
        low_device.hlc = Hlc {
            wall_ms: 100,
            counter: 0,
            device_id: 1,
        };
        store.insert(&low_device).unwrap();

        let mut high_device_incoming = record(b"high-device");
        high_device_incoming.id = low_device.id;
        high_device_incoming.hlc = Hlc {
            wall_ms: 100,
            counter: 0,
            device_id: 2,
        };

        assert!(store.upsert_from_sync(&high_device_incoming).unwrap());
        assert_eq!(
            store.get(low_device.id).unwrap().unwrap().sealed,
            b"high-device"
        );
    }
}
