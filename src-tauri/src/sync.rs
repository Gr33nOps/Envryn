//! Sync IPC: this device's identity, LAN peer discovery, pairing, trusted
//! device management, and manual sync sessions.
//!
//! Follows `ipc.rs`'s rules -- no command hands the UI a key, and errors
//! never distinguish "wrong password" from other failures. Pairing and sync
//! additionally never send vault key material anywhere until a human has
//! looked at the two devices' SAS values and confirmed they match; see
//! `envryn_core::sync::pairing`'s module doc for why that comparison, not
//! the transport, is what defeats an active machine-in-the-middle.
//!
//! **Scope note.** Sync only ever runs while this device's own vault is
//! unlocked -- there is no background listener that starts at app launch and
//! runs against a locked vault (`envryn_core::vault::Vault::trusted_fingerprints`'s
//! own doc comment already commits to this). `sync_listen_start` begins
//! accepting connections and advertising over mDNS; the accept loop checks
//! the vault's lock state on every iteration and stops itself the moment it
//! locks, rather than relying on every caller to remember to stop it.
//!
//! **Verification note.** The pairing and sync wire protocols themselves are
//! exercised with real loopback TCP and TLS in `envryn_core::sync`'s own
//! test suite. What is *not* exercised here is the interactive, two-human,
//! two-physical-device flow this module exists to drive -- there is no
//! second machine in this development environment. The background-thread
//! and event design below has been reviewed carefully but should be treated
//! as unverified until it has run against a second real device.

use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use envryn_core::model::TrustedDevice;
use envryn_core::storage::Store;
use envryn_core::sync::discovery::Discovery;
use envryn_core::sync::handshake;
use envryn_core::sync::identity::{DeviceIdentity, Fingerprint};
use envryn_core::sync::pairing::{open_vmk, seal_vmk};
use envryn_core::sync::protocol::run_sync_session;
use envryn_core::sync::transport::{
    client_config, placeholder_server_name, server_config, TrustedFingerprints,
};
use envryn_core::vault::Vault;
use envryn_core::{crypto::kdf, Error};
use rand::Rng;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use zeroize::Zeroizing;

use crate::ipc::{internal, invalid, vault_path, IpcError, IpcResult, VaultState};

const PAIRING_CONNECT_TIMEOUT: Duration = Duration::from_secs(120);
const PAIRING_CONFIRM_TIMEOUT: Duration = Duration::from_secs(90);
const DISCOVERY_BROWSE_TIMEOUT: Duration = Duration::from_secs(4);

fn identity_path(app: &AppHandle) -> IpcResult<std::path::PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| internal("could not locate the application data directory"))?;
    std::fs::create_dir_all(&dir)
        .map_err(|_| internal("could not create the application data directory"))?;
    Ok(dir.join("device_identity.json"))
}

fn load_identity(app: &AppHandle) -> IpcResult<DeviceIdentity> {
    let path = identity_path(app)?;
    Ok(DeviceIdentity::load_or_create(&path)?)
}

/// The numeric id this installation stamps its own writes with, for HLC
/// tie-breaking (`storage::Hlc::device_id_from_fingerprint_bytes`, derived
/// from the same certificate fingerprint sync already uses to identify this
/// device to peers). Called once, right after every unlock -- see
/// `ipc::vault_create`/`vault_unlock`/`vault_unlock_with_platform` -- so a
/// vault that has never synced still gets a real, stable, non-zero device id
/// rather than defaulting to the same id every install would otherwise share.
pub(crate) fn local_device_id(app: &AppHandle) -> IpcResult<u64> {
    let identity = load_identity(app)?;
    Ok(envryn_core::storage::Hlc::device_id_from_fingerprint_bytes(
        identity.fingerprint().as_bytes(),
    ))
}

/// Best-effort local LAN address, via the classic zero-traffic trick: a UDP
/// "connect" only asks the OS to pick a route and does not send a packet, so
/// this learns the interface address the OS would use without depending on
/// any particular network being reachable.
fn local_ip() -> IpcResult<std::net::IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|_| internal("could not determine this device's LAN address"))?;
    socket
        .connect("192.168.1.1:80")
        .map_err(|_| internal("could not determine this device's LAN address"))?;
    socket
        .local_addr()
        .map(|a| a.ip())
        .map_err(|_| internal("could not determine this device's LAN address"))
}

fn trusted_fingerprint_set(state: &VaultState) -> IpcResult<TrustedFingerprints> {
    let raw = state.with(|v| v.trusted_fingerprints())?;
    let fingerprints = raw
        .into_iter()
        .filter_map(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .map(Fingerprint::from_bytes)
        .collect::<Vec<_>>();
    Ok(TrustedFingerprints::new(fingerprints))
}

// --- device identity ---------------------------------------------------------

#[derive(Serialize, ts_rs::TS)]
#[ts(export)]
pub struct OwnIdentity {
    pub device_id: String,
    pub fingerprint_display: String,
    pub fingerprint_hex: String,
}

#[tauri::command]
pub fn device_identity(app: AppHandle) -> IpcResult<OwnIdentity> {
    let identity = load_identity(&app)?;
    Ok(OwnIdentity {
        device_id: identity.device_id.clone(),
        fingerprint_display: identity.fingerprint().to_display_string(),
        fingerprint_hex: identity.fingerprint().to_hex(),
    })
}

// --- trusted devices ----------------------------------------------------------

#[tauri::command]
pub fn trusted_device_list(state: State<'_, VaultState>) -> IpcResult<Vec<TrustedDevice>> {
    state.with(|v| v.list_trusted_devices())
}

#[tauri::command]
pub fn trusted_device_rename(
    state: State<'_, VaultState>,
    device_id: String,
    name: String,
) -> IpcResult<TrustedDevice> {
    if name.trim().is_empty() {
        return Err(invalid("Give the device a name."));
    }
    state.with(|v| v.rename_trusted_device(&device_id, name.trim()))
}

#[tauri::command]
pub fn trusted_device_revoke(state: State<'_, VaultState>, device_id: String) -> IpcResult<()> {
    state.with(|v| v.revoke_trusted_device(&device_id))
}

// --- discovery -----------------------------------------------------------------

#[derive(Serialize, ts_rs::TS)]
// The Rust name carries a "Dto" suffix that has no reason to leak into the
// generated contract; the frontend already calls this DiscoveredPeer.
#[ts(export, rename = "DiscoveredPeer")]
pub struct DiscoveredPeerDto {
    pub device_id: String,
    pub fingerprint_hex: String,
    pub addresses: Vec<String>,
    pub port: u16,
}

/// Browse the LAN for other Envryn instances currently listening (i.e. that
/// have called `sync_listen_start`). Does not itself imply trust -- see the
/// module doc on `envryn_core::sync::discovery`.
#[tauri::command]
pub async fn discovery_browse(app: AppHandle) -> IpcResult<Vec<DiscoveredPeerDto>> {
    tauri::async_runtime::spawn_blocking(move || {
        let identity = load_identity(&app)?;
        let discovery = Discovery::new().map_err(IpcError::from)?;
        let peers = discovery
            .browse(DISCOVERY_BROWSE_TIMEOUT)
            .map_err(IpcError::from)?;
        Ok(peers
            .into_iter()
            .filter(|p| p.device_id != identity.device_id)
            .map(|p| DiscoveredPeerDto {
                device_id: p.device_id,
                fingerprint_hex: p.fingerprint.to_hex(),
                addresses: p
                    .addresses
                    .iter()
                    .map(std::net::IpAddr::to_string)
                    .collect(),
                port: p.port,
            })
            .collect())
    })
    .await
    .map_err(|_| internal("discovery task failed"))?
}

// --- manual sync ---------------------------------------------------------------

#[derive(Serialize, ts_rs::TS)]
#[ts(export)]
pub struct SyncSummary {
    pub records_applied: usize,
    /// Genuine concurrent edits detected during this sync (INV-109) -- the
    /// losing side of each was preserved, not discarded, but the frontend
    /// should surface a non-zero count rather than let it pass silently.
    pub conflicts: usize,
}

/// Connect out to a peer and run one sync session as the TLS client. The
/// peer must already be listening (its own `sync_listen_start`) and must
/// trust this device's fingerprint, and this device must trust the peer's --
/// the mutual-TLS handshake enforces both directions before any record
/// moves (INV-104).
#[tauri::command]
pub async fn sync_now(
    app: AppHandle,
    state: State<'_, VaultState>,
    address: String,
    port: u16,
) -> IpcResult<SyncSummary> {
    let identity = load_identity(&app)?;
    let trusted = trusted_fingerprint_set(&state)?;
    let path = vault_path(&app)?;
    let target: SocketAddr = format!("{address}:{port}")
        .parse()
        .map_err(|_| invalid("Not a valid device address."))?;

    tauri::async_runtime::spawn_blocking(move || {
        let client_conf = Arc::new(client_config(&identity, trusted).map_err(IpcError::from)?);
        let stream = TcpStream::connect_timeout(&target, Duration::from_secs(10))
            .map_err(|_| internal("Could not reach that device."))?;
        let mut conn = rustls::ClientConnection::new(client_conf, placeholder_server_name())
            .map_err(|_| internal("could not start the TLS session"))?;
        let mut stream = stream;
        let mut tls = rustls::Stream::new(&mut conn, &mut stream);

        let store = Store::open(&path).map_err(IpcError::from)?;
        let result = run_sync_session(&mut tls, &store).map_err(IpcError::from)?;
        Ok(SyncSummary {
            records_applied: result.applied,
            conflicts: result.conflicts,
        })
    })
    .await
    .map_err(|_| internal("sync task failed"))?
}

/// Handle to a running `sync_listen_start` background thread, so
/// `sync_listen_stop` can signal it and a lock event can find out it should.
#[derive(Default)]
pub struct SyncListenState(Mutex<Option<Arc<AtomicBool>>>);

#[tauri::command]
pub fn sync_listen_start(
    app: AppHandle,
    listen_state: State<'_, SyncListenState>,
    vault_state: State<'_, VaultState>,
) -> IpcResult<u16> {
    // Fails closed if locked -- there is nothing to serve, and starting
    // anyway would mean the accept loop's first real check happens per
    // connection instead of here where the caller can see the error.
    vault_state.with(|_| Ok(()))?;

    let mut guard = listen_state
        .0
        .lock()
        .map_err(|_| internal("sync listener state unavailable"))?;
    if guard.is_some() {
        return Err(invalid("Already listening for sync connections."));
    }

    let identity = load_identity(&app)?;
    let listener = TcpListener::bind("0.0.0.0:0")
        .map_err(|_| internal("could not open a port to listen on"))?;
    let port = listener
        .local_addr()
        .map_err(|_| internal("could not determine the listener's port"))?
        .port();

    let running = Arc::new(AtomicBool::new(true));
    *guard = Some(running.clone());
    drop(guard);

    let path = vault_path(&app)?;
    let app_for_thread = app.clone();
    std::thread::spawn(move || run_listen_loop(app_for_thread, listener, identity, path, running));

    Ok(port)
}

#[tauri::command]
pub fn sync_listen_stop(listen_state: State<'_, SyncListenState>) -> IpcResult<()> {
    let mut guard = listen_state
        .0
        .lock()
        .map_err(|_| internal("sync listener state unavailable"))?;
    if let Some(flag) = guard.take() {
        flag.store(false, Ordering::SeqCst);
    }
    Ok(())
}

fn run_listen_loop(
    app: AppHandle,
    listener: TcpListener,
    identity: DeviceIdentity,
    vault_db_path: std::path::PathBuf,
    running: Arc<AtomicBool>,
) {
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let Ok(mut discovery) = Discovery::new() else {
        return;
    };
    if discovery.advertise(&identity, port).is_err() {
        return;
    }

    if listener.set_nonblocking(true).is_err() {
        return;
    }

    while running.load(Ordering::SeqCst) {
        let Some(vault_state) = app.try_state::<VaultState>() else {
            break;
        };
        let still_unlocked = vault_state
            .0
            .lock()
            .map(|g| g.as_ref().is_some_and(Vault::is_unlocked))
            .unwrap_or(false);
        if !still_unlocked {
            break;
        }

        match listener.accept() {
            Ok((stream, _addr)) => {
                let identity_ref = &identity;
                let trusted = app
                    .try_state::<VaultState>()
                    .and_then(|s| trusted_fingerprint_set(&s).ok());
                let Some(trusted) = trusted else { continue };
                let Ok(server_conf) = server_config(identity_ref, trusted) else {
                    continue;
                };
                let vault_db_path = vault_db_path.clone();
                std::thread::spawn(move || {
                    let _ = handle_incoming_sync(stream, server_conf, &vault_db_path);
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(_) => break,
        }
    }

    let _ = discovery.stop_advertising();
    if let Some(state) = app.try_state::<SyncListenState>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = None;
        }
    }
}

fn handle_incoming_sync(
    stream: TcpStream,
    server_conf: rustls::ServerConfig,
    vault_db_path: &std::path::Path,
) -> Result<(), Error> {
    let mut conn = rustls::ServerConnection::new(Arc::new(server_conf))
        .map_err(|_| Error::Internal("could not start the TLS session"))?;
    let mut stream = stream;
    let mut tls = rustls::Stream::new(&mut conn, &mut stream);
    let store = Store::open(vault_db_path)?;
    run_sync_session(&mut tls, &store)?;
    Ok(())
}

// --- pairing ---------------------------------------------------------------

enum PairingCommand {
    Confirm(Zeroizing<String>),
    Cancel,
}

#[derive(Default)]
pub struct PairingState(Mutex<Option<mpsc::Sender<PairingCommand>>>);

#[derive(Serialize, ts_rs::TS)]
#[ts(export)]
pub struct PairingHostStarted {
    pub address: String,
    pub port: u16,
    pub code: Option<String>,
    pub device_id: String,
    pub fingerprint_display: String,
}

#[derive(Clone, Serialize, ts_rs::TS)]
#[ts(export, rename = "PairingSasReady")]
struct SasReadyEvent {
    sas: String,
    peer_device_id: String,
    peer_fingerprint_display: String,
}

#[derive(Clone, Serialize, ts_rs::TS)]
#[ts(export, rename = "PairingFailed")]
struct PairingFailedEvent {
    message: String,
}

#[derive(Clone, Serialize, ts_rs::TS)]
#[ts(export, rename = "PairingComplete")]
struct PairingCompleteEvent {
    peer_device_id: String,
}

fn random_pairing_code() -> String {
    format!("{:06}", rand::thread_rng().gen_range(0..1_000_000u32))
}

/// Begin hosting a pairing session: bind a listener, hand back the address
/// (and, for the manual-code path, a fresh code) for the UI to display, and
/// wait for a peer in the background.
#[tauri::command]
pub fn pairing_host_start(
    app: AppHandle,
    pairing_state: State<'_, PairingState>,
    manual: bool,
) -> IpcResult<PairingHostStarted> {
    let mut guard = pairing_state
        .0
        .lock()
        .map_err(|_| internal("pairing state unavailable"))?;
    if guard.is_some() {
        return Err(invalid("A pairing session is already in progress."));
    }

    let identity = load_identity(&app)?;
    let listener = TcpListener::bind("0.0.0.0:0")
        .map_err(|_| internal("could not open a port for pairing"))?;
    let port = listener
        .local_addr()
        .map_err(|_| internal("could not determine the pairing port"))?
        .port();
    let ip = local_ip()?;
    let code = manual.then(random_pairing_code);

    let (tx, rx) = mpsc::channel();
    *guard = Some(tx);
    drop(guard);

    let response = PairingHostStarted {
        address: ip.to_string(),
        port,
        code: code.clone(),
        device_id: identity.device_id.clone(),
        fingerprint_display: identity.fingerprint().to_display_string(),
    };

    std::thread::spawn(move || {
        run_pairing_host(app, listener, identity, code, rx);
    });

    Ok(response)
}

/// Join a pairing session hosted by another device.
#[tauri::command]
pub fn pairing_join_start(
    app: AppHandle,
    pairing_state: State<'_, PairingState>,
    address: String,
    port: u16,
    code: Option<String>,
) -> IpcResult<()> {
    let vault_db_path = vault_path(&app)?;
    if vault_db_path.exists() {
        return Err(invalid(
            "This device already has a vault. Pairing to receive a shared vault is only supported on a device that doesn't have one yet.",
        ));
    }

    let mut guard = pairing_state
        .0
        .lock()
        .map_err(|_| internal("pairing state unavailable"))?;
    if guard.is_some() {
        return Err(invalid("A pairing session is already in progress."));
    }

    let identity = load_identity(&app)?;
    let target: SocketAddr = format!("{address}:{port}")
        .parse()
        .map_err(|_| invalid("Not a valid device address."))?;
    let stream = TcpStream::connect_timeout(&target, Duration::from_secs(15))
        .map_err(|_| invalid("Could not reach that device. Check the address and try again."))?;

    let (tx, rx) = mpsc::channel();
    *guard = Some(tx);
    drop(guard);

    std::thread::spawn(move || {
        run_pairing_join(app, stream, identity, code, vault_db_path, rx);
    });

    Ok(())
}

/// The human confirmed the SAS matches on both devices. `secret` is the
/// current master password on the host, or the new master password to
/// protect the freshly created vault on the joiner -- this command doesn't
/// know which role is waiting; the background thread does.
#[tauri::command]
pub fn pairing_confirm(pairing_state: State<'_, PairingState>, secret: String) -> IpcResult<()> {
    let mut guard = pairing_state
        .0
        .lock()
        .map_err(|_| internal("pairing state unavailable"))?;
    let sender = guard
        .take()
        .ok_or_else(|| invalid("No pairing session in progress."))?;
    let _ = sender.send(PairingCommand::Confirm(Zeroizing::new(secret)));
    Ok(())
}

#[tauri::command]
pub fn pairing_cancel(pairing_state: State<'_, PairingState>) -> IpcResult<()> {
    let mut guard = pairing_state
        .0
        .lock()
        .map_err(|_| internal("pairing state unavailable"))?;
    if let Some(sender) = guard.take() {
        let _ = sender.send(PairingCommand::Cancel);
    }
    Ok(())
}

/// Accept exactly one incoming connection within [`PAIRING_CONNECT_TIMEOUT`],
/// polling non-blockingly so an early `pairing_cancel` (sent before a peer
/// even connects) can interrupt the wait -- `TcpListener::accept` has no
/// portable timeout in `std`, so this is the same poll-with-deadline shape
/// `sync::discovery`'s browse uses.
fn accept_with_deadline(
    listener: &TcpListener,
    deadline: Instant,
    rx: &mpsc::Receiver<PairingCommand>,
) -> Option<TcpStream> {
    let _ = listener.set_nonblocking(true);
    loop {
        if rx.try_recv().is_ok() {
            return None;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                return Some(stream);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(_) => return None,
        }
    }
}

fn clear_pairing_state(app: &AppHandle) {
    if let Some(state) = app.try_state::<PairingState>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = None;
        }
    }
}

fn emit_failed(app: &AppHandle, message: &str) {
    let _ = app.emit(
        "pairing://failed",
        PairingFailedEvent {
            message: message.to_string(),
        },
    );
}

fn run_pairing_host(
    app: AppHandle,
    listener: TcpListener,
    identity: DeviceIdentity,
    code: Option<String>,
    rx: mpsc::Receiver<PairingCommand>,
) {
    let deadline = Instant::now() + PAIRING_CONNECT_TIMEOUT;
    let Some(mut stream) = accept_with_deadline(&listener, deadline, &rx) else {
        emit_failed(&app, "Pairing timed out or was cancelled.");
        clear_pairing_state(&app);
        return;
    };

    let handshake_result = match &code {
        Some(code) => handshake::run_manual_pairing(
            &mut stream,
            code,
            &identity.device_id,
            identity.fingerprint(),
        ),
        None => handshake::run_qr_pairing(&mut stream, &identity.device_id, identity.fingerprint()),
    };
    let Ok((session, peer)) = handshake_result else {
        emit_failed(
            &app,
            "Could not complete the key exchange with that device.",
        );
        clear_pairing_state(&app);
        return;
    };
    let Ok(sas) = session.sas() else {
        emit_failed(&app, "Could not compute the verification code.");
        clear_pairing_state(&app);
        return;
    };

    let _ = app.emit(
        "pairing://sas-ready",
        SasReadyEvent {
            sas,
            peer_device_id: peer.device_id.clone(),
            peer_fingerprint_display: peer.fingerprint.to_display_string(),
        },
    );

    let command = rx.recv_timeout(PAIRING_CONFIRM_TIMEOUT);
    clear_pairing_state(&app);
    let Ok(PairingCommand::Confirm(password)) = command else {
        emit_failed(&app, "Pairing was cancelled.");
        return;
    };

    let Some(vault_state) = app.try_state::<VaultState>() else {
        emit_failed(&app, "The vault is unavailable.");
        return;
    };
    let vmk = match vault_state.with(|v| v.export_vmk_for_pairing(&password)) {
        Ok(vmk) => vmk,
        Err(_) => {
            emit_failed(&app, "That password was not correct.");
            return;
        }
    };

    let Ok(sealed) = seal_vmk(&session, &vmk) else {
        emit_failed(&app, "Could not prepare the vault key for transfer.");
        return;
    };
    if handshake::send_sealed_vmk(&mut stream, &sealed).is_err() {
        emit_failed(&app, "The connection to the other device was lost.");
        return;
    }

    let device_id = peer.device_id.clone();
    let added = vault_state.with(|v| {
        v.add_trusted_device(
            &peer.device_id,
            peer.fingerprint.as_bytes(),
            &peer.device_id,
        )
    });
    if added.is_err() {
        emit_failed(&app, "Paired, but could not record the trusted device.");
        return;
    }

    let _ = app.emit(
        "pairing://complete",
        PairingCompleteEvent {
            peer_device_id: device_id,
        },
    );
}

fn run_pairing_join(
    app: AppHandle,
    mut stream: TcpStream,
    identity: DeviceIdentity,
    code: Option<String>,
    vault_db_path: std::path::PathBuf,
    rx: mpsc::Receiver<PairingCommand>,
) {
    let handshake_result = match &code {
        Some(code) => handshake::run_manual_pairing(
            &mut stream,
            code,
            &identity.device_id,
            identity.fingerprint(),
        ),
        None => handshake::run_qr_pairing(&mut stream, &identity.device_id, identity.fingerprint()),
    };
    let Ok((session, peer)) = handshake_result else {
        emit_failed(
            &app,
            "Could not complete the key exchange with that device.",
        );
        clear_pairing_state(&app);
        return;
    };
    let Ok(sas) = session.sas() else {
        emit_failed(&app, "Could not compute the verification code.");
        clear_pairing_state(&app);
        return;
    };

    let _ = app.emit(
        "pairing://sas-ready",
        SasReadyEvent {
            sas,
            peer_device_id: peer.device_id.clone(),
            peer_fingerprint_display: peer.fingerprint.to_display_string(),
        },
    );

    let command = rx.recv_timeout(PAIRING_CONFIRM_TIMEOUT);
    clear_pairing_state(&app);
    let Ok(PairingCommand::Confirm(new_master_password)) = command else {
        emit_failed(&app, "Pairing was cancelled.");
        return;
    };
    if new_master_password.len() < 8 {
        emit_failed(
            &app,
            "Your new master password must be at least 8 characters.",
        );
        return;
    }

    let Ok(sealed) = handshake::receive_sealed_vmk(&mut stream) else {
        emit_failed(&app, "The connection to the other device was lost.");
        return;
    };
    let Ok(vmk) = open_vmk(&session, &sealed) else {
        emit_failed(&app, "The vault key could not be verified.");
        return;
    };

    let params = kdf::calibrate(700);
    let mut vault = match Vault::create_with_vmk(&vault_db_path, &new_master_password, params, vmk)
    {
        Ok(v) => v,
        Err(_) => {
            emit_failed(&app, "Could not create the vault on this device.");
            return;
        }
    };

    if vault
        .add_trusted_device(
            &peer.device_id,
            peer.fingerprint.as_bytes(),
            &peer.device_id,
        )
        .is_err()
    {
        emit_failed(
            &app,
            "Vault created, but could not record the trusted device.",
        );
        return;
    }

    let device_id = peer.device_id.clone();
    let Some(vault_state) = app.try_state::<VaultState>() else {
        emit_failed(&app, "The vault could not be installed.");
        return;
    };
    if vault_state.install(vault).is_err() {
        emit_failed(&app, "The vault could not be installed.");
        return;
    }

    let _ = app.emit(
        "pairing://complete",
        PairingCompleteEvent {
            peer_device_id: device_id,
        },
    );
}
