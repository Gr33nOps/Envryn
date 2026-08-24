//! Persistence.
//!
//! This layer moves opaque bytes. It never holds a key, never seals, and never
//! opens -- encryption happens one level up, in [`crate::vault`]. Keeping the
//! boundary there means a bug in SQL handling cannot produce a plaintext write,
//! because this module has nothing to write in plaintext.

pub mod schema;

use rusqlite::{params, Connection, OptionalExtension};

use crate::crypto::fingerprint::Fingerprint;
use crate::error::{Error, Result};
use crate::model::SecretId;

pub use schema::{meta_keys, RECORD_VERSION, SCHEMA_VERSION};

/// A stored row, still sealed.
pub struct StoredRecord {
    pub id: SecretId,
    pub record_version: i64,
    pub sealed: Vec<u8>,
    pub fingerprint: Option<Fingerprint>,
    pub created_ms: i64,
    pub updated_ms: i64,
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
                (id, record_version, sealed, fingerprint, created_ms, updated_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                record.id.to_string(),
                record.record_version,
                record.sealed,
                record.fingerprint.map(|f| f.as_bytes().to_vec()),
                record.created_ms,
                record.updated_ms,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, record: &StoredRecord) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE secrets
                SET sealed = ?2, fingerprint = ?3, updated_ms = ?4, record_version = ?5
              WHERE id = ?1 AND deleted = 0",
            params![
                record.id.to_string(),
                record.sealed,
                record.fingerprint.map(|f| f.as_bytes().to_vec()),
                record.updated_ms,
                record.record_version,
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
                "SELECT id, record_version, sealed, fingerprint, created_ms, updated_ms
                   FROM secrets WHERE id = ?1 AND deleted = 0",
                params![id.to_string()],
                row_to_record,
            )
            .optional()?
            .transpose()
    }

    pub fn list(&self) -> Result<Vec<StoredRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, record_version, sealed, fingerprint, created_ms, updated_ms
               FROM secrets WHERE deleted = 0 ORDER BY updated_ms DESC",
        )?;
        let rows = stmt.query_map([], row_to_record)?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// Soft-delete.
    ///
    /// A tombstone rather than a row removal, because a deletion racing a sync
    /// against a peer that has not seen it would otherwise resurrect the
    /// record (INV-110). The sealed blob is cleared immediately so the
    /// ciphertext does not linger for the tombstone's retention window.
    pub fn soft_delete(&self, id: SecretId, at_ms: i64) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE secrets
                SET deleted = 1, sealed = X'', fingerprint = NULL, updated_ms = ?2
              WHERE id = ?1 AND deleted = 0",
            params![id.to_string(), at_ms],
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
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<StoredRecord>> {
    let id_str: String = row.get(0)?;
    let fp_bytes: Option<Vec<u8>> = row.get(3)?;

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
            updated_ms: row.get(5).unwrap_or(0),
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
            updated_ms: 100,
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
        rec.updated_ms = 200;
        store.update(&rec).unwrap();

        let got = store.get(rec.id).unwrap().unwrap();
        assert_eq!(got.sealed, b"after");
        assert_eq!(got.updated_ms, 200);
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

        store.soft_delete(id, 300).unwrap();

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
        store.soft_delete(id, 300).unwrap();

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
        store.soft_delete(id, 1).unwrap();
        assert!(matches!(store.soft_delete(id, 2), Err(Error::NotFound)));
    }

    #[test]
    fn list_excludes_deleted_and_sorts_by_recency() {
        let store = Store::open_in_memory().unwrap();
        let mut old = record(b"old");
        old.updated_ms = 10;
        let mut new = record(b"new");
        new.updated_ms = 20;
        let gone = record(b"gone");

        store.insert(&old).unwrap();
        store.insert(&new).unwrap();
        store.insert(&gone).unwrap();
        store.soft_delete(gone.id, 30).unwrap();

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
        store.soft_delete(a.id, 1).unwrap();

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
}
