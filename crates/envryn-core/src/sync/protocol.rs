//! The sync wire protocol: manifest exchange, then only the records each
//! side is actually behind on.
//!
//! Deliberately generic over `Read + Write` rather than tied to a specific
//! async runtime -- the same logic runs over the blocking `rustls::Stream`
//! used in `sync::transport`'s tests and over a real TLS connection accepted
//! on a background thread in the Tauri shell. Messages are length-prefixed
//! JSON: simple to reason about, and sync payloads are small enough
//! (metadata plus already-compact sealed blobs) that JSON's overhead is not
//! worth trading away the readability during development.
//!
//! Records travel still encrypted -- this module never holds a key, matching
//! [`crate::storage`]'s own rule (`docs/CRYPTOGRAPHY.md` section 3: "records
//! travel still encrypted during sync"). Reconciliation decides *which* rows
//! to move by comparing HLCs; it never decrypts anything to make that
//! decision.

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::SecretId;
use crate::storage::{Hlc, ManifestEntry, Store, StoredRecord, VersionVector};

pub(crate) const MAX_MESSAGE_LEN: u32 = 64 * 1024 * 1024; // generous; a hostile peer sending more is refused, not allocated for

#[derive(Serialize, Deserialize)]
struct WireManifestEntry {
    id: String,
    wall_ms: i64,
    counter: u32,
    device_id: u64,
    /// JSON-encoded `VersionVector` -- carried so `ids_to_request` can detect
    /// "might be a conflict, request it" rather than the coarse (and, for a
    /// genuine fork, wrong -- see `ManifestEntry`'s doc comment)
    /// scalar-`Hlc`-only comparison this replaced.
    version_vector: String,
    deleted: bool,
}

impl From<&ManifestEntry> for WireManifestEntry {
    fn from(e: &ManifestEntry) -> Self {
        Self {
            id: e.id.to_string(),
            wall_ms: e.hlc.wall_ms,
            counter: e.hlc.counter,
            device_id: e.hlc.device_id,
            version_vector: e.version_vector.to_json(),
            deleted: e.deleted,
        }
    }
}

impl TryFrom<WireManifestEntry> for ManifestEntry {
    type Error = Error;
    fn try_from(w: WireManifestEntry) -> Result<Self> {
        Ok(Self {
            id: SecretId::parse(&w.id)?,
            hlc: Hlc {
                wall_ms: w.wall_ms,
                counter: w.counter,
                device_id: w.device_id,
            },
            version_vector: VersionVector::from_json(&w.version_vector),
            deleted: w.deleted,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct WireRecord {
    id: String,
    record_version: i64,
    sealed: Vec<u8>,
    fingerprint: Option<Vec<u8>>,
    created_ms: i64,
    wall_ms: i64,
    counter: u32,
    device_id: u64,
    deleted: bool,
    version_vector: String,
}

impl TryFrom<&StoredRecord> for WireRecord {
    type Error = Error;
    fn try_from(r: &StoredRecord) -> Result<Self> {
        Ok(Self {
            id: r.id.to_string(),
            record_version: r.record_version,
            sealed: r.sealed.clone(),
            fingerprint: r.fingerprint.map(|f| f.as_bytes().to_vec()),
            created_ms: r.created_ms,
            wall_ms: r.hlc.wall_ms,
            counter: r.hlc.counter,
            device_id: r.hlc.device_id,
            deleted: r.deleted,
            version_vector: r.version_vector.to_json(),
        })
    }
}

impl TryFrom<WireRecord> for StoredRecord {
    type Error = Error;
    fn try_from(w: WireRecord) -> Result<Self> {
        let fingerprint = match w.fingerprint {
            Some(bytes) => Some(crate::crypto::fingerprint::Fingerprint::from_slice(&bytes)?),
            None => None,
        };
        Ok(Self {
            id: SecretId::parse(&w.id)?,
            record_version: w.record_version,
            sealed: w.sealed,
            fingerprint,
            created_ms: w.created_ms,
            hlc: Hlc {
                wall_ms: w.wall_ms,
                counter: w.counter,
                device_id: w.device_id,
            },
            deleted: w.deleted,
            version_vector: VersionVector::from_json(&w.version_vector),
        })
    }
}

#[derive(Serialize, Deserialize)]
struct ManifestMessage {
    entries: Vec<WireManifestEntry>,
}

#[derive(Serialize, Deserialize)]
struct RequestMessage {
    ids: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct RecordsMessage {
    records: Vec<WireRecord>,
}

pub(crate) fn write_json<W: Write, T: Serialize>(stream: &mut W, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    let len = u32::try_from(bytes.len()).map_err(|_| Error::Internal("sync message too large"))?;
    stream
        .write_all(&len.to_le_bytes())
        .and_then(|()| stream.write_all(&bytes))
        .map_err(|_| Error::Internal("sync connection failed"))
}

pub(crate) fn read_json<R: Read, T: for<'de> Deserialize<'de>>(stream: &mut R) -> Result<T> {
    let mut len_bytes = [0u8; 4];
    stream
        .read_exact(&mut len_bytes)
        .map_err(|_| Error::Internal("sync connection failed"))?;
    let len = u32::from_le_bytes(len_bytes);
    if len > MAX_MESSAGE_LEN {
        return Err(Error::Internal("peer sent an oversized sync message"));
    }
    let mut buf = vec![0u8; len as usize];
    stream
        .read_exact(&mut buf)
        .map_err(|_| Error::Internal("sync connection failed"))?;
    serde_json::from_slice(&buf).map_err(|_| Error::Internal("malformed sync message"))
}

/// Given the local manifest and a peer's manifest, decide which ids the peer
/// should send *to us* -- pure function, no I/O, fully testable without a
/// network. We request an id if we do not have it at all, or if our vector
/// does not already dominate the peer's (i.e. the peer might know something
/// we don't -- either because they are ahead, or because of a genuine
/// concurrent edit). A scalar-`Hlc`-only comparison would miss the second
/// case: a concurrent edit can have an *older* wall clock than our own and
/// still carry information `upsert_from_sync` needs to see to detect the
/// conflict (INV-109) -- if we never request it, the conflict is invisible,
/// not merely unresolved.
pub fn ids_to_request(local: &[ManifestEntry], remote: &[ManifestEntry]) -> Vec<SecretId> {
    use std::collections::HashMap;
    let local_by_id: HashMap<SecretId, &crate::storage::VersionVector> =
        local.iter().map(|e| (e.id, &e.version_vector)).collect();

    remote
        .iter()
        .filter(|r| {
            local_by_id
                .get(&r.id)
                .is_none_or(|local_vector| !local_vector.dominates(&r.version_vector))
        })
        .map(|r| r.id)
        .collect()
}

/// The result of one sync exchange: how many incoming records were applied
/// (fast-forwarded or newly inserted), and how many turned out to be genuine
/// concurrent edits (INV-109) -- the caller (`src-tauri/src/sync.rs`) surfaces
/// `conflicts` to the user rather than letting it pass silently, since a
/// non-zero count means something needs a look, not just "sync succeeded."
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncSessionResult {
    pub applied: usize,
    pub conflicts: usize,
}

/// Run one sync exchange over an already-authenticated, already-open stream.
/// Symmetric: both peers run this same function, each acting as both sender
/// and receiver, so there is no separate "server routine" to keep in sync
/// with a "client routine."
pub fn run_sync_session<S: Read + Write>(
    stream: &mut S,
    store: &Store,
) -> Result<SyncSessionResult> {
    // 1. Exchange manifests.
    let local_manifest = store.list_manifest()?;
    write_json(
        stream,
        &ManifestMessage {
            entries: local_manifest.iter().map(WireManifestEntry::from).collect(),
        },
    )?;
    let remote_manifest_wire: ManifestMessage = read_json(stream)?;
    let remote_manifest: Vec<ManifestEntry> = remote_manifest_wire
        .entries
        .into_iter()
        .map(ManifestEntry::try_from)
        .collect::<Result<_>>()?;

    // 2. Figure out what each side needs, and ask for it.
    let we_need = ids_to_request(&local_manifest, &remote_manifest);
    let they_need = ids_to_request(&remote_manifest, &local_manifest);

    write_json(
        stream,
        &RequestMessage {
            ids: we_need.iter().map(ToString::to_string).collect(),
        },
    )?;
    let their_request: RequestMessage = read_json(stream)?;

    // 3. Send what they asked for.
    let mut to_send = Vec::new();
    for id_str in &their_request.ids {
        let Ok(id) = SecretId::parse(id_str) else {
            continue;
        };
        if let Some(record) = store.get_including_deleted(id)? {
            to_send.push(WireRecord::try_from(&record)?);
        }
    }
    write_json(stream, &RecordsMessage { records: to_send })?;

    // 4. Receive what we asked for, and apply it.
    let incoming: RecordsMessage = read_json(stream)?;
    let mut result = SyncSessionResult::default();
    for wire in incoming.records {
        let record = StoredRecord::try_from(wire)?;
        match store.upsert_from_sync(&record)? {
            crate::storage::SyncOutcome::New | crate::storage::SyncOutcome::FastForward => {
                result.applied += 1;
            }
            crate::storage::SyncOutcome::Conflict => {
                result.applied += 1;
                result.conflicts += 1;
            }
            crate::storage::SyncOutcome::Stale => {}
        }
    }

    let _ = they_need; // informational; the peer decides its own requests independently
    Ok(result)
}

/// Fuzzing-only entry point for every inbound JSON frame used by sync. Each
/// attempt includes the real four-byte length prefix and allocation limit.
#[cfg(feature = "fuzzing")]
pub fn fuzz_parse_wire_message(bytes: &[u8]) {
    use std::io::Cursor;

    let _ = read_json::<_, ManifestMessage>(&mut Cursor::new(bytes));
    let _ = read_json::<_, RequestMessage>(&mut Cursor::new(bytes));
    let _ = read_json::<_, RecordsMessage>(&mut Cursor::new(bytes));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Environment, NewSecret, SecretPayload};
    use crate::vault::Vault;
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use zeroize::Zeroizing;

    #[test]
    fn ids_to_request_covers_missing_and_newer_only() {
        let id_a = SecretId::new();
        let id_b = SecretId::new();
        let id_c = SecretId::new();

        let local_a_hlc = Hlc {
            wall_ms: 100,
            counter: 0,
            device_id: 1,
        };
        let local_b_hlc = Hlc {
            wall_ms: 100,
            counter: 0,
            device_id: 1,
        };
        let local = vec![
            ManifestEntry {
                id: id_a,
                hlc: local_a_hlc,
                version_vector: VersionVector::single(local_a_hlc),
                deleted: false,
            },
            ManifestEntry {
                id: id_b,
                hlc: local_b_hlc,
                version_vector: VersionVector::single(local_b_hlc),
                deleted: false,
            },
        ];
        // a: remote's vector dominates local's (it has seen local_a_hlc plus
        // its own newer tick) -> request.
        let remote_a_hlc = Hlc {
            wall_ms: 200,
            counter: 0,
            device_id: 2,
        };
        // b: remote's vector is dominated by local's (an ancestor) -> do not
        // request.
        let remote_b_hlc = Hlc {
            wall_ms: 50,
            counter: 0,
            device_id: 1,
        };
        let remote = vec![
            ManifestEntry {
                id: id_a,
                hlc: remote_a_hlc,
                version_vector: VersionVector::single(local_a_hlc).advanced_by(remote_a_hlc),
                deleted: false,
            },
            ManifestEntry {
                id: id_b,
                hlc: remote_b_hlc,
                version_vector: VersionVector::single(remote_b_hlc),
                deleted: false,
            },
            // c: we don't have it at all -> request
            ManifestEntry {
                id: id_c,
                hlc: Hlc {
                    wall_ms: 1,
                    counter: 0,
                    device_id: 2,
                },
                version_vector: VersionVector::single(Hlc {
                    wall_ms: 1,
                    counter: 0,
                    device_id: 2,
                }),
                deleted: false,
            },
        ];

        let mut requested = ids_to_request(&local, &remote);
        requested.sort_by_key(ToString::to_string);
        let mut expected = vec![id_a, id_c];
        expected.sort_by_key(ToString::to_string);
        assert_eq!(requested, expected);
    }

    fn pw(s: &str) -> Zeroizing<String> {
        Zeroizing::new(s.to_string())
    }

    /// End-to-end: two real vaults, on two real device identities, connected
    /// over real loopback mutual TLS, reconcile a genuinely divergent state
    /// -- each holds one record the other does not -- into agreement. This
    /// is the two-party stand-in for two physical devices described
    /// throughout `sync::transport`'s tests.
    #[test]
    fn two_vaults_converge_over_real_tls() {
        use crate::sync::identity::DeviceIdentity;
        use crate::sync::transport;

        let dir = tempfile::tempdir().unwrap();
        let id_a = DeviceIdentity::load_or_create(&dir.path().join("id_a.json")).unwrap();
        let id_b = DeviceIdentity::load_or_create(&dir.path().join("id_b.json")).unwrap();

        let trusted_by_a = transport::TrustedFingerprints::new([id_b.fingerprint()]);
        let trusted_by_b = transport::TrustedFingerprints::new([id_a.fingerprint()]);

        let mut vault_a = Vault::create(&dir.path().join("a.db"), &pw("pw-a"), fast_kdf()).unwrap();
        vault_a
            .create_secret(NewSecret {
                name: "ONLY_ON_A".into(),
                project: "P".into(),
                environment: Environment::Production,
                payload: SecretPayload::ApiKey {
                    value: "a-value".into(),
                },
                notes: None,
                tags: vec![],
                provider: None,
            })
            .unwrap();

        // The step that actually makes two devices "paired": B's vault is
        // created from A's VMK (as if it had just arrived over
        // `pairing::open_vmk`), not a fresh random one. Exercising
        // `export_vmk_for_pairing` -> `create_with_vmk` here proves the real
        // production path, not a shortcut standing in for it.
        let shared_vmk = vault_a.export_vmk_for_pairing(&pw("pw-a")).unwrap();
        let mut vault_b = Vault::create_with_vmk(
            &dir.path().join("b.db"),
            &pw("pw-b"),
            fast_kdf(),
            shared_vmk,
        )
        .unwrap();
        vault_b
            .create_secret(NewSecret {
                name: "ONLY_ON_B".into(),
                project: "P".into(),
                environment: Environment::Production,
                payload: SecretPayload::ApiKey {
                    value: "b-value".into(),
                },
                notes: None,
                tags: vec![],
                provider: None,
            })
            .unwrap();

        let store_a = crate::storage::Store::open(&dir.path().join("a.db")).unwrap();
        let store_b = crate::storage::Store::open(&dir.path().join("b.db")).unwrap();

        let server_conf = Arc::new(transport::server_config(&id_a, trusted_by_a).unwrap());
        let client_conf = Arc::new(transport::client_config(&id_b, trusted_by_b).unwrap());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_thread = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut conn = rustls::ServerConnection::new(server_conf).unwrap();
            let mut stream = stream;
            let mut tls = rustls::Stream::new(&mut conn, &mut stream);
            run_sync_session(&mut tls, &store_a).unwrap()
        });

        let stream = TcpStream::connect(addr).unwrap();
        let mut conn =
            rustls::ClientConnection::new(client_conf, transport::placeholder_server_name())
                .unwrap();
        let mut stream = stream;
        let mut tls = rustls::Stream::new(&mut conn, &mut stream);
        let applied_by_b = run_sync_session(&mut tls, &store_b).unwrap();
        let applied_by_a = server_thread.join().unwrap();

        assert_eq!(
            applied_by_a.applied, 1,
            "A should have received exactly B's record"
        );
        assert_eq!(
            applied_by_b.applied, 1,
            "B should have received exactly A's record"
        );
        assert_eq!(applied_by_a.conflicts, 0);
        assert_eq!(applied_by_b.conflicts, 0);

        // Reload both vaults from disk and confirm both records are present
        // on both sides.
        let mut vault_a = Vault::open(&dir.path().join("a.db")).unwrap();
        vault_a.unlock(&pw("pw-a")).unwrap();
        let names_a: Vec<String> = vault_a
            .list()
            .unwrap()
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert!(names_a.contains(&"ONLY_ON_A".to_string()));
        assert!(names_a.contains(&"ONLY_ON_B".to_string()));

        let mut vault_b = Vault::open(&dir.path().join("b.db")).unwrap();
        vault_b.unlock(&pw("pw-b")).unwrap();
        let names_b: Vec<String> = vault_b
            .list()
            .unwrap()
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert!(names_b.contains(&"ONLY_ON_A".to_string()));
        assert!(names_b.contains(&"ONLY_ON_B".to_string()));
    }

    /// INV-109, end to end: two real, previously-paired vaults each edit the
    /// *same* record while genuinely disconnected from each other, then sync
    /// over real loopback mutual TLS. Neither device's edit was built on the
    /// other's, so this must land as `SyncOutcome::Conflict` on both sides --
    /// a real deterministic winner becomes the live value, and the losing
    /// edit is recoverable, not discarded. Nothing here is simulated at the
    /// `Store` level; every write goes through the same `Vault::update_secret`
    /// and `run_sync_session` a real two-device conflict would.
    #[test]
    fn two_devices_editing_the_same_record_offline_produce_a_recoverable_conflict() {
        use crate::sync::identity::DeviceIdentity;

        let dir = tempfile::tempdir().unwrap();
        let id_a = DeviceIdentity::load_or_create(&dir.path().join("id_a.json")).unwrap();
        let id_b = DeviceIdentity::load_or_create(&dir.path().join("id_b.json")).unwrap();
        let path_a = dir.path().join("a.db");
        let path_b = dir.path().join("b.db");

        let mut vault_a = Vault::create(&path_a, &pw("pw-a"), fast_kdf()).unwrap();
        vault_a.set_local_device_id(1).unwrap();
        let created = vault_a
            .create_secret(NewSecret {
                name: "SHARED".into(),
                project: "P".into(),
                environment: Environment::Production,
                payload: SecretPayload::ApiKey {
                    value: "original".into(),
                },
                notes: None,
                tags: vec![],
                provider: None,
            })
            .unwrap();
        let shared_id = created.id;

        let shared_vmk = vault_a.export_vmk_for_pairing(&pw("pw-a")).unwrap();
        let mut vault_b =
            Vault::create_with_vmk(&path_b, &pw("pw-b"), fast_kdf(), shared_vmk).unwrap();
        vault_b.set_local_device_id(2).unwrap();
        drop(vault_a);
        drop(vault_b);

        // First sync: B learns A's record, establishing a common ancestor.
        let (first_a, first_b) = sync_once(&id_a, &id_b, &path_a, &path_b);
        assert_eq!(first_a.applied, 0);
        assert_eq!(first_b.applied, 1);
        assert_eq!(first_a.conflicts, 0);
        assert_eq!(first_b.conflicts, 0);

        // Now both devices edit the SAME record independently, with no
        // further sync between the edits -- a genuine fork.
        let mut vault_a = Vault::open(&path_a).unwrap();
        vault_a.unlock(&pw("pw-a")).unwrap();
        vault_a.set_local_device_id(1).unwrap();
        vault_a
            .update_secret(
                shared_id,
                crate::model::SecretUpdate {
                    payload: Some(SecretPayload::ApiKey {
                        value: "edited-on-a".into(),
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        drop(vault_a);

        // Ensure B's edit gets a later wall-clock tick than A's, so the
        // conflict's deterministic winner is predictable in this test.
        std::thread::sleep(std::time::Duration::from_millis(5));

        let mut vault_b = Vault::open(&path_b).unwrap();
        vault_b.unlock(&pw("pw-b")).unwrap();
        vault_b.set_local_device_id(2).unwrap();
        vault_b
            .update_secret(
                shared_id,
                crate::model::SecretUpdate {
                    payload: Some(SecretPayload::ApiKey {
                        value: "edited-on-b".into(),
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        drop(vault_b);

        // Second sync: the two divergent edits meet.
        let (second_a, second_b) = sync_once(&id_a, &id_b, &path_a, &path_b);
        assert_eq!(second_a.conflicts, 1, "A must detect the fork");
        assert_eq!(second_b.conflicts, 1, "B must detect the fork");

        // B's edit has the later wall clock, so it wins the deterministic
        // tiebreak and becomes the live value on both sides.
        let mut vault_a = Vault::open(&path_a).unwrap();
        vault_a.unlock(&pw("pw-a")).unwrap();
        vault_a.set_local_device_id(1).unwrap();
        let live = vault_a.reveal(shared_id).unwrap();
        assert_eq!(
            live.payload,
            SecretPayload::ApiKey {
                value: "edited-on-b".into()
            }
        );

        // A's own edit was not thrown away -- it is recoverable.
        let conflicts = vault_a.list_conflicts(shared_id).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(
            conflicts[0].record.payload,
            SecretPayload::ApiKey {
                value: "edited-on-a".into()
            }
        );

        let recovered = vault_a.recover_conflict(&conflicts[0].conflict_id).unwrap();
        assert_ne!(
            recovered.id, shared_id,
            "recovering a conflict must not collide with the live record's id"
        );
        assert_eq!(vault_a.list_conflicts(shared_id).unwrap().len(), 0);
    }

    /// A second sync with nothing new to exchange must apply zero records --
    /// otherwise every idle sync would re-transfer the whole vault forever.
    #[test]
    fn a_second_sync_with_nothing_new_applies_nothing() {
        use crate::sync::identity::DeviceIdentity;

        let dir = tempfile::tempdir().unwrap();
        let id_a = DeviceIdentity::load_or_create(&dir.path().join("id_a.json")).unwrap();
        let id_b = DeviceIdentity::load_or_create(&dir.path().join("id_b.json")).unwrap();
        let path_a = dir.path().join("a.db");
        let path_b = dir.path().join("b.db");

        let mut vault_a = Vault::create(&path_a, &pw("pw-a"), fast_kdf()).unwrap();
        vault_a
            .create_secret(NewSecret {
                name: "SHARED".into(),
                project: "P".into(),
                environment: Environment::Production,
                payload: SecretPayload::ApiKey { value: "v".into() },
                notes: None,
                tags: vec![],
                provider: None,
            })
            .unwrap();
        // Give B a vault of its own to sync into -- an empty one is fine.
        Vault::create(&path_b, &pw("pw-b"), fast_kdf()).unwrap();

        // First sync: B learns A's one record.
        sync_once(&id_a, &id_b, &path_a, &path_b);

        // Second sync: nothing has changed on either side.
        let (applied_a, applied_b) = sync_once(&id_a, &id_b, &path_a, &path_b);
        assert_eq!(
            (applied_a.applied, applied_b.applied),
            (0, 0),
            "an idle second sync must transfer nothing"
        );
    }

    fn fast_kdf() -> crate::crypto::kdf::KdfParams {
        crate::crypto::kdf::KdfParams {
            memory_kib: 19 * 1024,
            iterations: 2,
            parallelism: 1,
        }
    }

    /// Runs one sync session between A (server) and B (client) and returns
    /// (records A applied, records B applied).
    ///
    /// Each side opens its *own* `rusqlite::Connection` to its own vault
    /// file, inside its own thread, rather than sharing one `Store` across
    /// the thread boundary -- `Connection` is `Send` but not `Sync` (it
    /// caches prepared statements behind a `RefCell`), so a shared `&Store`
    /// cannot cross threads at all, scoped or not. This mirrors how the real
    /// Tauri integration will work too: the sync session for a given peer
    /// gets its own connection, never one shared with the main vault state.
    fn sync_once(
        id_a: &crate::sync::identity::DeviceIdentity,
        id_b: &crate::sync::identity::DeviceIdentity,
        path_a: &std::path::Path,
        path_b: &std::path::Path,
    ) -> (SyncSessionResult, SyncSessionResult) {
        use crate::sync::transport;

        let trusted_by_a = transport::TrustedFingerprints::new([id_b.fingerprint()]);
        let trusted_by_b = transport::TrustedFingerprints::new([id_a.fingerprint()]);
        let server_conf = Arc::new(transport::server_config(id_a, trusted_by_a).unwrap());
        let client_conf = Arc::new(transport::client_config(id_b, trusted_by_b).unwrap());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        std::thread::scope(|scope| {
            let client_thread = scope.spawn(move || {
                let store_b = Store::open(path_b).unwrap();
                let stream = TcpStream::connect(addr).unwrap();
                let mut conn = rustls::ClientConnection::new(
                    client_conf,
                    transport::placeholder_server_name(),
                )
                .unwrap();
                let mut stream = stream;
                let mut tls = rustls::Stream::new(&mut conn, &mut stream);
                run_sync_session(&mut tls, &store_b).unwrap()
            });

            let store_a = Store::open(path_a).unwrap();
            let (stream, _) = listener.accept().unwrap();
            let mut conn = rustls::ServerConnection::new(server_conf).unwrap();
            let mut stream = stream;
            let mut tls = rustls::Stream::new(&mut conn, &mut stream);
            let applied_a = run_sync_session(&mut tls, &store_a).unwrap();

            let applied_b = client_thread.join().unwrap();
            (applied_a, applied_b)
        })
    }
}
