//! Persistence.
//!
//! This layer moves opaque bytes. It never holds a key, never seals, and never
//! opens -- encryption happens one level up, in [`crate::vault`]. Keeping the
//! boundary there means a bug in SQL handling cannot produce a plaintext write,
//! because this module has nothing to write in plaintext.

pub mod hlc;
pub mod schema;
pub mod version_vector;

use rusqlite::{params, Connection, OptionalExtension};

use crate::crypto::fingerprint::Fingerprint;
use crate::error::{Error, Result};
use crate::model::SecretId;

pub use hlc::Hlc;
pub use schema::{meta_keys, RECORD_VERSION, SCHEMA_VERSION};
pub use version_vector::VersionVector;

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
    /// This record's causal history (see [`VersionVector`]). Always populated
    /// correctly on a read ([`Store::get`], [`Store::get_including_deleted`],
    /// [`Store::list`]). On the *local*-write paths ([`Store::insert`],
    /// [`Store::update`], [`Store::soft_delete`]) whatever value is set here
    /// is ignored -- those methods derive the correct vector themselves from
    /// `hlc.device_id` (always the local device for a local write) merged
    /// with whatever the row already had, since a local edit is by
    /// definition built on the value it just read. The one write path that
    /// actually *trusts* the caller's vector is [`Store::upsert_from_sync`]:
    /// a peer's own knowledge of a record's history is exactly the
    /// information a local edit cannot derive on its own.
    pub version_vector: VersionVector,
}

impl StoredRecord {
    pub fn updated_ms(&self) -> i64 {
        self.hlc.wall_ms
    }
}

/// A record's identity and version for sync's manifest exchange -- everything
/// needed to decide whether a peer might have something we don't, without the
/// ciphertext payload. Carries the full [`VersionVector`], not just the
/// scalar [`Hlc`]: a coarse "is the peer's Hlc newer" filter would miss a
/// genuinely concurrent edit whose wall-clock happens to be *older* than the
/// local one, silently dropping the very conflict this manifest exchange
/// needs to surface (see [`Store::upsert_from_sync`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub id: SecretId,
    pub hlc: Hlc,
    pub version_vector: VersionVector,
    pub deleted: bool,
}

/// What happened when [`Store::upsert_from_sync`] applied one incoming row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOutcome {
    /// The row did not exist locally before this write.
    New,
    /// We already knew everything the peer's write reflects; nothing changed.
    Stale,
    /// The peer's write was a causal descendant of what we had -- applied
    /// directly, no conflict.
    FastForward,
    /// A genuine concurrent edit. The `Hlc`-newer side became the live row;
    /// the other side was preserved in `record_conflicts`, not discarded.
    Conflict,
}

impl SyncOutcome {
    /// Whether the local database actually changed as a result.
    pub fn applied(self) -> bool {
        !matches!(self, SyncOutcome::Stale)
    }
}

/// The losing side of a genuine concurrent edit, preserved rather than
/// discarded (INV-109). `record` decrypts exactly like any other
/// [`StoredRecord`] -- same id, same AAD -- so recovering it (re-inserting it
/// as a fresh record, or comparing it against the live value) uses the same
/// `crate::vault` machinery as anything else.
pub struct ConflictRecord {
    /// This conflict row's own id -- pass to [`Store::delete_conflict`] once
    /// resolved. Not the secret's id; see [`Store::list_conflicts`].
    pub conflict_id: String,
    pub record: StoredRecord,
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
        let vector = VersionVector::single(record.hlc);
        self.conn.execute(
            "INSERT INTO secrets
                (id, record_version, sealed, fingerprint, created_ms, updated_ms,
                 hlc_counter, hlc_device, deleted, version_vector)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                vector.to_json(),
            ],
        )?;
        Ok(())
    }

    /// Local edit of an existing row. `record.version_vector` is ignored (see
    /// [`StoredRecord`]'s doc comment) -- the new vector is the row's current
    /// one, advanced by this write's own `hlc`, since a local edit is always
    /// built on the value that was just read.
    pub fn update(&self, record: &StoredRecord) -> Result<()> {
        let existing_vector: Option<String> = self
            .conn
            .query_row(
                "SELECT version_vector FROM secrets WHERE id = ?1 AND deleted = 0",
                params![record.id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        let Some(existing_vector) = existing_vector else {
            return Err(Error::NotFound);
        };
        let vector = VersionVector::from_json(&existing_vector).advanced_by(record.hlc);

        let changed = self.conn.execute(
            "UPDATE secrets
                SET sealed = ?2, fingerprint = ?3, updated_ms = ?4, record_version = ?5,
                    hlc_counter = ?6, hlc_device = ?7, version_vector = ?8
              WHERE id = ?1 AND deleted = 0",
            params![
                record.id.to_string(),
                record.sealed,
                record.fingerprint.map(|f| f.as_bytes().to_vec()),
                record.hlc.wall_ms,
                record.record_version,
                record.hlc.counter,
                record.hlc.device_id.to_string(),
                vector.to_json(),
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
                        hlc_counter, hlc_device, deleted, version_vector
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
                        hlc_counter, hlc_device, deleted, version_vector
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
                    hlc_counter, hlc_device, deleted, version_vector
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
        let mut stmt = self.conn.prepare(
            "SELECT id, updated_ms, hlc_counter, hlc_device, deleted, version_vector FROM secrets",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let wall_ms: i64 = row.get(1)?;
            let counter: u32 = row.get(2)?;
            let device_str: String = row.get(3)?;
            let deleted: bool = row.get(4)?;
            let vector_json: String = row.get(5)?;
            Ok((id, wall_ms, counter, device_str, deleted, vector_json))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (id, wall_ms, counter, device_str, deleted, vector_json) = row?;
            out.push(ManifestEntry {
                id: SecretId::parse(&id)?,
                hlc: Hlc {
                    wall_ms,
                    counter,
                    device_id: device_str.parse().unwrap_or(0),
                },
                version_vector: VersionVector::from_json(&vector_json),
                deleted,
            });
        }
        Ok(out)
    }

    /// Apply an incoming synced row.
    ///
    /// Uses the row's [`VersionVector`], not just its scalar [`Hlc`], to tell
    /// three cases apart (INV-109):
    ///
    /// - The peer already knows everything we know (our vector dominates
    ///   theirs): the incoming write is stale, nothing changes.
    /// - We already know everything the peer knows, or less (their vector
    ///   dominates ours, or the row is new to us): a clean fast-forward,
    ///   applied directly.
    /// - Neither side's vector dominates the other's: a genuine concurrent
    ///   edit. The scalar `Hlc` picks a deterministic winner (unrelated
    ///   devices must agree on the same one), the winner becomes the live
    ///   row, and the loser is preserved in `record_conflicts` rather than
    ///   silently discarded -- the whole reason this type exists instead of
    ///   the plain scalar-only comparison this function used to make.
    pub fn upsert_from_sync(&self, record: &StoredRecord) -> Result<SyncOutcome> {
        let existing = self.get_including_deleted(record.id)?;

        let Some(existing) = existing else {
            self.raw_insert(record, &record.version_vector)?;
            return Ok(SyncOutcome::New);
        };

        if existing.version_vector.dominates(&record.version_vector) {
            return Ok(SyncOutcome::Stale);
        }

        if record.version_vector.dominates(&existing.version_vector) {
            self.raw_write(record, &record.version_vector)?;
            return Ok(SyncOutcome::FastForward);
        }

        // Neither dominates: a genuine fork. Deterministic winner by Hlc,
        // same rule on every peer so all sides converge on the same choice.
        let (winner, loser) = if record.hlc > existing.hlc {
            (record, &existing)
        } else {
            (&existing, record)
        };
        let merged_vector = existing.version_vector.merged_with(&record.version_vector);
        self.raw_write(winner, &merged_vector)?;
        self.stash_conflict(loser)?;
        Ok(SyncOutcome::Conflict)
    }

    /// Shared by the `New` and fast-forward branches of `upsert_from_sync`:
    /// write a full row (insert-or-replace) with an explicit vector, rather
    /// than the local-write `insert`/`update` methods, which always compute
    /// their own vector from `hlc.device_id` -- wrong here, since the vector
    /// that matters is the peer's, not "local device, advanced."
    fn raw_write(&self, record: &StoredRecord, vector: &VersionVector) -> Result<()> {
        self.conn.execute(
            "UPDATE secrets
                SET sealed = ?2, fingerprint = ?3, updated_ms = ?4, record_version = ?5,
                    hlc_counter = ?6, hlc_device = ?7, deleted = ?8, version_vector = ?9
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
                vector.to_json(),
            ],
        )?;
        Ok(())
    }

    fn raw_insert(&self, record: &StoredRecord, vector: &VersionVector) -> Result<()> {
        self.conn.execute(
            "INSERT INTO secrets
                (id, record_version, sealed, fingerprint, created_ms, updated_ms,
                 hlc_counter, hlc_device, deleted, version_vector)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                vector.to_json(),
            ],
        )?;
        Ok(())
    }

    /// Preserve the losing side of a conflict rather than discard it.
    fn stash_conflict(&self, loser: &StoredRecord) -> Result<()> {
        let conflict_id = format!(
            "{}-{}-{}-{}",
            loser.id, loser.hlc.wall_ms, loser.hlc.counter, loser.hlc.device_id
        );
        self.conn.execute(
            "INSERT INTO record_conflicts
                (id, secret_id, record_version, sealed, fingerprint,
                 hlc_wall_ms, hlc_counter, hlc_device, deleted, created_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO NOTHING",
            params![
                conflict_id,
                loser.id.to_string(),
                loser.record_version,
                loser.sealed,
                loser.fingerprint.map(|f| f.as_bytes().to_vec()),
                loser.hlc.wall_ms,
                loser.hlc.counter,
                loser.hlc.device_id.to_string(),
                loser.deleted,
                loser.hlc.wall_ms,
            ],
        )?;
        Ok(())
    }

    /// Every preserved conflict for one record, most recent first -- the
    /// vault layer decrypts `record` for the user to review and decide
    /// whether to recover, discard, or keep as a separate record.
    pub fn list_conflicts(&self, secret_id: SecretId) -> Result<Vec<ConflictRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, secret_id, record_version, sealed, fingerprint,
                    hlc_wall_ms, hlc_counter, hlc_device, deleted, created_ms
               FROM record_conflicts WHERE secret_id = ?1 ORDER BY created_ms DESC",
        )?;
        let rows = stmt.query_map(params![secret_id.to_string()], row_to_conflict)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// One preserved conflict by its own row id -- the lookup
    /// `Vault::recover_conflict`/`discard_conflict` need before they can act
    /// on a specific conflict without the caller also supplying the secret id.
    pub fn get_conflict(&self, conflict_row_id: &str) -> Result<Option<ConflictRecord>> {
        self.conn
            .query_row(
                "SELECT id, secret_id, record_version, sealed, fingerprint,
                        hlc_wall_ms, hlc_counter, hlc_device, deleted, created_ms
                   FROM record_conflicts WHERE id = ?1",
                params![conflict_row_id],
                row_to_conflict,
            )
            .optional()?
            .transpose()
    }

    /// All preserved conflicts across the whole vault, for a summary badge
    /// ("N unresolved conflicts") without a per-record query.
    pub fn count_conflicts(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM record_conflicts", [], |r| r.get(0))?)
    }

    /// Discard a preserved conflict once the user has reviewed it (recovered
    /// it as a new record, or explicitly chosen to drop it). `conflict_row_id`
    /// is [`ConflictRecord::conflict_id`], the conflict row's own id -- not
    /// the underlying secret's id.
    pub fn delete_conflict(&self, conflict_row_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM record_conflicts WHERE id = ?1",
            params![conflict_row_id],
        )?;
        Ok(())
    }

    /// Soft-delete.
    ///
    /// A tombstone rather than a row removal, because a deletion racing a sync
    /// against a peer that has not seen it would otherwise resurrect the
    /// record (INV-110). The sealed blob is cleared immediately so the
    /// ciphertext does not linger for the tombstone's retention window.
    pub fn soft_delete(&self, id: SecretId, hlc: Hlc) -> Result<()> {
        let existing_vector: Option<String> = self
            .conn
            .query_row(
                "SELECT version_vector FROM secrets WHERE id = ?1 AND deleted = 0",
                params![id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        let Some(existing_vector) = existing_vector else {
            return Err(Error::NotFound);
        };
        // A delete is a causal event on this record like any other local
        // write -- it must advance the vector too, or a later concurrent
        // edit from another device could fail to be recognised as a genuine
        // fork against it.
        let vector = VersionVector::from_json(&existing_vector).advanced_by(hlc);

        let changed = self.conn.execute(
            "UPDATE secrets
                SET deleted = 1, sealed = X'', fingerprint = NULL, updated_ms = ?2,
                    hlc_counter = ?3, hlc_device = ?4, version_vector = ?5
              WHERE id = ?1 AND deleted = 0",
            params![
                id.to_string(),
                hlc.wall_ms,
                hlc.counter,
                hlc.device_id.to_string(),
                vector.to_json(),
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
    let vector_json: String = row.get(9).unwrap_or_default();

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
            version_vector: VersionVector::from_json(&vector_json),
        })
    })())
}

/// Columns: id, secret_id, record_version, sealed, fingerprint, hlc_wall_ms,
/// hlc_counter, hlc_device, deleted, created_ms -- see
/// `Store::list_conflicts`'s query.
fn row_to_conflict(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<ConflictRecord>> {
    let conflict_id: String = row.get(0)?;
    let secret_id_str: String = row.get(1)?;
    let fp_bytes: Option<Vec<u8>> = row.get(4)?;
    let wall_ms: i64 = row.get(5).unwrap_or(0);
    let counter: u32 = row.get(6).unwrap_or(0);
    let device_str: String = row.get(7).unwrap_or_default();
    let deleted: bool = row.get(8).unwrap_or(false);
    let created_ms: i64 = row.get(9).unwrap_or(0);

    Ok((|| {
        let fingerprint = match fp_bytes {
            Some(bytes) => Some(Fingerprint::from_slice(&bytes)?),
            None => None,
        };
        let hlc = Hlc {
            wall_ms,
            counter,
            device_id: device_str.parse().unwrap_or(0),
        };
        Ok(ConflictRecord {
            conflict_id,
            record: StoredRecord {
                id: SecretId::parse(&secret_id_str)?,
                record_version: row.get(2).unwrap_or(RECORD_VERSION),
                sealed: row.get(3).unwrap_or_default(),
                fingerprint,
                created_ms,
                hlc,
                deleted,
                version_vector: VersionVector::single(hlc),
            },
        })
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::fingerprint::Fingerprint;

    fn record(sealed: &[u8]) -> StoredRecord {
        let hlc = Hlc {
            wall_ms: 100,
            counter: 0,
            device_id: 1,
        };
        StoredRecord {
            id: SecretId::new(),
            record_version: RECORD_VERSION,
            sealed: sealed.to_vec(),
            fingerprint: Some(Fingerprint::from_bytes([3u8; 16])),
            created_ms: 100,
            hlc,
            deleted: false,
            version_vector: VersionVector::single(hlc),
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
        assert_eq!(store.upsert_from_sync(&rec).unwrap(), SyncOutcome::New);
        assert_eq!(store.get(rec.id).unwrap().unwrap().sealed, b"from-peer");
    }

    /// The peer redelivering a state we have already moved past (its vector
    /// is an ancestor of ours -- we have since advanced further) must be
    /// silently ignored, not overwrite newer local data. This is the "peer
    /// was simply behind" case INV-109 distinguishes from a genuine conflict.
    #[test]
    fn upsert_from_sync_rejects_a_write_we_have_already_moved_past() {
        let store = Store::open_in_memory().unwrap();
        let base_hlc = Hlc {
            wall_ms: 50,
            counter: 0,
            device_id: 2,
        };

        // Seed a row as if it had already synced from device 2 once...
        let mut base = record(b"base-from-peer");
        base.hlc = base_hlc;
        base.version_vector = VersionVector::single(base_hlc);
        store.upsert_from_sync(&base).unwrap();

        // ...then the local device (1) advanced it further.
        let mut newer = record(b"newer-local");
        newer.id = base.id;
        newer.hlc = Hlc {
            wall_ms: 200,
            counter: 0,
            device_id: 1,
        };
        store.update(&newer).unwrap();

        // The peer now redelivers the exact state we already incorporated
        // and moved past -- its vector is dominated by ours.
        let mut stale_incoming = record(b"stale-redelivery");
        stale_incoming.id = base.id;
        stale_incoming.hlc = base_hlc;
        stale_incoming.version_vector = VersionVector::single(base_hlc);

        assert_eq!(
            store.upsert_from_sync(&stale_incoming).unwrap(),
            SyncOutcome::Stale
        );
        assert_eq!(store.get(base.id).unwrap().unwrap().sealed, b"newer-local");
    }

    /// The peer's write builds on (dominates) what we have -- a clean
    /// fast-forward, no conflict.
    #[test]
    fn upsert_from_sync_fast_forwards_a_dominating_write() {
        let store = Store::open_in_memory().unwrap();
        let mut older = record(b"older-local");
        older.hlc = Hlc {
            wall_ms: 100,
            counter: 0,
            device_id: 1,
        };
        store.insert(&older).unwrap();

        // The peer's incoming write is causally aware of our current state
        // (its vector includes our entry) plus its own new tick -- exactly
        // what "the peer had already synced from us, then edited" looks
        // like.
        let newer_hlc = Hlc {
            wall_ms: 200,
            counter: 0,
            device_id: 2,
        };
        let mut newer_incoming = record(b"newer-from-peer");
        newer_incoming.id = older.id;
        newer_incoming.hlc = newer_hlc;
        newer_incoming.version_vector = older.version_vector.advanced_by(newer_hlc);

        assert_eq!(
            store.upsert_from_sync(&newer_incoming).unwrap(),
            SyncOutcome::FastForward
        );
        assert_eq!(
            store.get(older.id).unwrap().unwrap().sealed,
            b"newer-from-peer"
        );
    }

    /// The actual point of INV-109: two devices edit the same record without
    /// having synced with each other first (neither vector dominates the
    /// other). The `Hlc`-newer side wins and becomes the live value -- the
    /// vault stays usable -- but the losing side is preserved in
    /// `record_conflicts`, not silently destroyed.
    #[test]
    fn a_genuine_concurrent_edit_preserves_the_losing_side() {
        let store = Store::open_in_memory().unwrap();

        let mut from_device_1 = record(b"edited-on-device-1");
        from_device_1.hlc = Hlc {
            wall_ms: 100,
            counter: 0,
            device_id: 1,
        };
        store.insert(&from_device_1).unwrap();

        // Device 2 never saw device 1's edit -- its vector only reflects its
        // own contribution, with a higher wall clock so it should win the
        // tiebreak.
        let device_2_hlc = Hlc {
            wall_ms: 200,
            counter: 0,
            device_id: 2,
        };
        let mut from_device_2 = record(b"edited-on-device-2");
        from_device_2.id = from_device_1.id;
        from_device_2.hlc = device_2_hlc;
        from_device_2.version_vector = VersionVector::single(device_2_hlc);

        let outcome = store.upsert_from_sync(&from_device_2).unwrap();
        assert_eq!(outcome, SyncOutcome::Conflict);

        // The higher-Hlc side is live...
        assert_eq!(
            store.get(from_device_1.id).unwrap().unwrap().sealed,
            b"edited-on-device-2"
        );
        // ...but device 1's edit was not thrown away.
        let conflicts = store.list_conflicts(from_device_1.id).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].record.sealed, b"edited-on-device-1");
        assert_eq!(store.count_conflicts().unwrap(), 1);
    }

    /// A preserved conflict is not permanent clutter -- once the user has
    /// reviewed it, it can be discarded.
    #[test]
    fn a_resolved_conflict_can_be_deleted() {
        let store = Store::open_in_memory().unwrap();
        let mut from_device_1 = record(b"edited-on-device-1");
        from_device_1.hlc = Hlc {
            wall_ms: 100,
            counter: 0,
            device_id: 1,
        };
        store.insert(&from_device_1).unwrap();

        let device_2_hlc = Hlc {
            wall_ms: 200,
            counter: 0,
            device_id: 2,
        };
        let mut from_device_2 = record(b"edited-on-device-2");
        from_device_2.id = from_device_1.id;
        from_device_2.hlc = device_2_hlc;
        from_device_2.version_vector = VersionVector::single(device_2_hlc);
        store.upsert_from_sync(&from_device_2).unwrap();

        let conflicts = store.list_conflicts(from_device_1.id).unwrap();
        assert_eq!(conflicts.len(), 1);
        store.delete_conflict(&conflicts[0].conflict_id).unwrap();
        assert_eq!(store.list_conflicts(from_device_1.id).unwrap().len(), 0);
        assert_eq!(store.count_conflicts().unwrap(), 0);
    }
}
