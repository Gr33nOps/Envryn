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
use crate::storage::{Hlc, ManifestEntry, Store, StoredRecord};

pub(crate) const MAX_MESSAGE_LEN: u32 = 64 * 1024 * 1024; // generous; a hostile peer sending more is refused, not allocated for

#[derive(Serialize, Deserialize)]
struct WireManifestEntry {
    id: String,
    wall_ms: i64,
    counter: u32,
    device_id: u64,
    deleted: bool,
}

impl From<&ManifestEntry> for WireManifestEntry {
    fn from(e: &ManifestEntry) -> Self {
        Self {
            id: e.id.to_string(),
            wall_ms: e.hlc.wall_ms,
            counter: e.hlc.counter,
            device_id: e.hlc.device_id,
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
/// network. We request an id if we do not have it at all, or if the peer's
/// HLC for it is strictly newer than ours; anything else (peer is behind,
/// or the two sides tie) we do not request, since `upsert_from_sync`'s own
/// rule would discard it anyway -- deciding it here just avoids the transfer.
pub fn ids_to_request(local: &[ManifestEntry], remote: &[ManifestEntry]) -> Vec<SecretId> {
    use std::collections::HashMap;
    let local_by_id: HashMap<SecretId, Hlc> = local.iter().map(|e| (e.id, e.hlc)).collect();

    remote
        .iter()
        .filter(|r| {
            local_by_id
                .get(&r.id)
                .is_none_or(|local_hlc| r.hlc > *local_hlc)
        })
        .map(|r| r.id)
        .collect()
}

/// Run one sync exchange over an already-authenticated, already-open stream.
/// Symmetric: both peers run this same function, each acting as both sender
/// and receiver, so there is no separate "server routine" to keep in sync
/// with a "client routine."
///
/// Returns how many records were received and applied locally.
pub fn run_sync_session<S: Read + Write>(stream: &mut S, store: &Store) -> Result<usize> {
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
    let mut applied = 0;
    for wire in incoming.records {
        let record = StoredRecord::try_from(wire)?;
        if store.upsert_from_sync(&record)? {
            applied += 1;
        }
    }

    let _ = they_need; // informational; the peer decides its own requests independently
    Ok(applied)
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

        let local = vec![
            ManifestEntry {
                id: id_a,
                hlc: Hlc {
                    wall_ms: 100,
                    counter: 0,
                    device_id: 1,
                },
                deleted: false,
            },
            ManifestEntry {
                id: id_b,
                hlc: Hlc {
                    wall_ms: 100,
                    counter: 0,
                    device_id: 1,
                },
                deleted: false,
            },
        ];
        let remote = vec![
            // a: remote is newer -> request
            ManifestEntry {
                id: id_a,
                hlc: Hlc {
                    wall_ms: 200,
                    counter: 0,
                    device_id: 2,
                },
                deleted: false,
            },
            // b: remote is older -> do not request
            ManifestEntry {
                id: id_b,
                hlc: Hlc {
                    wall_ms: 50,
                    counter: 0,
                    device_id: 2,
                },
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

        assert_eq!(applied_by_a, 1, "A should have received exactly B's record");
        assert_eq!(applied_by_b, 1, "B should have received exactly A's record");

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
        let applied = sync_once(&id_a, &id_b, &path_a, &path_b);
        assert_eq!(applied, (0, 0), "an idle second sync must transfer nothing");
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
    ) -> (usize, usize) {
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
