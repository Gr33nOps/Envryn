//! LAN discovery via mDNS/DNS-SD.
//!
//! Advertises this device on `_envryn._tcp.local.` with its fingerprint and
//! device id in a TXT record, and browses for other instances doing the
//! same.
//!
//! **Discovery conveys zero trust.** A browsed [`DiscoveredPeer`] is nothing
//! more than an address and a self-reported fingerprint -- anyone on the LAN
//! can advertise any fingerprint they like. The only place trust is actually
//! established is `sync::transport`'s mutual-TLS handshake, which accepts a
//! connection only if the peer proves possession of the private key behind a
//! fingerprint already present in `trusted_devices`. An unpaired device that
//! is discovered gets a TLS handshake rejection; a paired device that is
//! *not* discovered (multicast blocked, different subnet) can still be
//! reached by address. Discovery is a convenience for finding that address,
//! not a security boundary.
//!
//! **Verification note:** this module is exercised only against a single
//! `mdns-sd` daemon running in this process (self-discovery -- a service
//! this process registers is also seen by a browse issued from the same
//! process, on the same host). There is no second physical machine available
//! in this development environment, so genuine cross-device LAN discovery
//! -- multicast actually reaching another host, mDNS behaviour across real
//! Windows/Android network stacks and firewalls -- has not been exercised.
//! The wire format (service type, TXT keys) is stable and documented here so
//! it can be tested against a second device before this is relied upon.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use rand::RngCore;

use crate::error::{Error, Result};
use crate::sync::identity::{DeviceIdentity, Fingerprint};

/// The one service type Envryn advertises and browses for.
pub const SERVICE_TYPE: &str = "_envryn._tcp.local.";

/// Fixed UDP port for the direct LAN discovery fallback. The sync listener
/// itself remains on an ephemeral TCP port, which is carried in the reply.
/// This is deliberately separate from mDNS port 5353 because Android and
/// some access points suppress multicast while still permitting ordinary
/// local broadcast and unicast traffic.
const FALLBACK_DISCOVERY_PORT: u16 = 37_853;
const FALLBACK_QUERY_MAGIC: &[u8; 16] = b"envryn-disc-v1\0\0";
const FALLBACK_REPLY_MAGIC: &[u8; 16] = b"envryn-here-v1\0\0";
const FALLBACK_NONCE_LEN: usize = 16;
const FALLBACK_QUERY_LEN: usize = 64;
const FALLBACK_REPLY_LEN: usize = 82;
const FALLBACK_POLL: Duration = Duration::from_millis(100);
const FALLBACK_RETRY: Duration = Duration::from_millis(500);

const PROP_FINGERPRINT: &str = "fp";
const PROP_DEVICE_ID: &str = "id";

/// A peer seen on the LAN. `fingerprint` and `device_id` are the peer's own
/// claims, unauthenticated until `sync::transport` proves them -- see the
/// module doc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    pub device_id: String,
    pub fingerprint: Fingerprint,
    pub addresses: Vec<IpAddr>,
    pub port: u16,
}

/// Owns the mdns-sd daemon thread and (if advertising) this device's own
/// registration, so both stop together on drop rather than leaking an
/// advertisement after the caller forgets about it.
pub struct Discovery {
    daemon: ServiceDaemon,
    advertised_fullname: Option<String>,
    fallback_responder: Option<FallbackResponder>,
}

struct FallbackResponder {
    running: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    #[cfg(test)]
    local_addr: SocketAddr,
}

impl Drop for FallbackResponder {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Discovery {
    /// Starts the mDNS daemon thread. Does not advertise or browse by
    /// itself -- call [`Discovery::advertise`] and/or [`Discovery::browse`].
    pub fn new() -> Result<Self> {
        let daemon =
            ServiceDaemon::new().map_err(|_| Error::Internal("could not start mDNS daemon"))?;
        Ok(Self {
            daemon,
            advertised_fullname: None,
            fallback_responder: None,
        })
    }

    /// Advertise this device on the LAN so paired peers can find it by mDNS.
    ///
    /// `port` is the mutual-TLS listener's port; binding and owning that
    /// listener is the caller's responsibility (the IPC layer), not this
    /// module's -- discovery only announces where it is.
    pub fn advertise(&mut self, identity: &DeviceIdentity, port: u16) -> Result<()> {
        self.stop_advertising()?;

        // Start the broadcast responder before registering mDNS. If mDNS is
        // filtered by Android or the LAN, peers still learn this listener's
        // current ephemeral port from a direct unicast reply.
        self.fallback_responder = start_fallback_responder(
            SocketAddr::from((Ipv4Addr::UNSPECIFIED, FALLBACK_DISCOVERY_PORT)),
            identity,
            port,
        )
        .ok();

        let instance_name = identity.device_id.clone();
        let host_name = format!("{}.local.", identity.device_id);
        let props: [(&str, String); 2] = [
            (PROP_FINGERPRINT, identity.fingerprint().to_hex()),
            (PROP_DEVICE_ID, identity.device_id.clone()),
        ];

        // Empty IP + `enable_addr_auto()` -- the daemon fills in this host's
        // real interface addresses and keeps them updated, rather than this
        // module trying to enumerate interfaces itself.
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &host_name,
            "",
            port,
            &props[..],
        )
        .map_err(|_| Error::Internal("could not build mDNS service record"))?
        .enable_addr_auto();

        let fullname = service.get_fullname().to_string();
        self.daemon
            .register(service)
            .map_err(|_| Error::Internal("could not register mDNS service"))?;
        self.advertised_fullname = Some(fullname);
        Ok(())
    }

    /// Stop advertising, if currently advertising. Idempotent.
    pub fn stop_advertising(&mut self) -> Result<()> {
        self.fallback_responder.take();
        if let Some(fullname) = self.advertised_fullname.take() {
            self.daemon
                .unregister(&fullname)
                .map_err(|_| Error::Internal("could not unregister mDNS service"))?;
        }
        Ok(())
    }

    /// Browse for other Envryn instances on the LAN for up to `timeout`,
    /// returning whatever resolved within that window.
    ///
    /// One-shot rather than a persistent subscription: the sync page's "scan
    /// for devices" action wants a bounded list back, not an open stream.
    ///
    /// A given peer is typically resolved more than once during the window
    /// (once per local network interface, plus cache refreshes) -- results
    /// are deduplicated by `device_id`, keeping the most recently resolved
    /// record for each.
    pub fn browse(&self, timeout: Duration) -> Result<Vec<DiscoveredPeer>> {
        let fallback = FallbackBrowser::start(SocketAddr::from((
            Ipv4Addr::BROADCAST,
            FALLBACK_DISCOVERY_PORT,
        )))
        .ok();
        let receiver = self
            .daemon
            .browse(SERVICE_TYPE)
            .map_err(|_| Error::Internal("could not start mDNS browse"))?;

        let deadline = Instant::now() + timeout;
        let mut peers: HashMap<String, DiscoveredPeer> = HashMap::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match receiver.recv_timeout(remaining.min(FALLBACK_POLL)) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    if let Some(peer) = peer_from_service_info(&info) {
                        // A device also advertises to itself on the same
                        // daemon; the caller decides whether to filter that
                        // out (it knows its own device id, this module
                        // doesn't).
                        merge_peer(&mut peers, peer);
                    }
                }
                Ok(_) => {}
                // A short timeout is expected. It lets the same bounded loop
                // drain unicast fallback replies without a second four-second
                // wait after mDNS has finished.
                Err(_) => {}
            }
            if let Some(browser) = &fallback {
                browser.retry_if_due();
                for peer in browser.drain() {
                    merge_peer(&mut peers, peer);
                }
            }
        }
        let _ = self.daemon.stop_browse(SERVICE_TYPE);
        Ok(peers.into_values().collect())
    }
}

fn merge_peer(peers: &mut HashMap<String, DiscoveredPeer>, peer: DiscoveredPeer) {
    if let Some(existing) = peers.get_mut(&peer.device_id) {
        // A conflicting self-reported fingerprint is not evidence of trust.
        // Keep the first claim and let mutual TLS perform the real check.
        if existing.fingerprint != peer.fingerprint {
            return;
        }
        existing.port = peer.port;
        for address in peer.addresses {
            if !existing.addresses.contains(&address) {
                existing.addresses.push(address);
            }
        }
    } else {
        peers.insert(peer.device_id.clone(), peer);
    }
}

fn start_fallback_responder(
    bind: SocketAddr,
    identity: &DeviceIdentity,
    sync_port: u16,
) -> std::io::Result<FallbackResponder> {
    let socket = UdpSocket::bind(bind)?;
    #[cfg(test)]
    let local_addr = socket.local_addr()?;
    socket.set_read_timeout(Some(FALLBACK_POLL))?;
    let device_id = uuid::Uuid::parse_str(&identity.device_id)
        .map_err(|_| std::io::Error::other("device id is not a UUID"))?;
    let fingerprint = *identity.fingerprint().as_bytes();
    let running = Arc::new(AtomicBool::new(true));
    let thread_running = running.clone();
    let handle = thread::spawn(move || {
        let mut buffer = [0_u8; FALLBACK_QUERY_LEN];
        while thread_running.load(Ordering::SeqCst) {
            let Ok((length, source)) = socket.recv_from(&mut buffer) else {
                continue;
            };
            let Some(query) = buffer.get(..length) else {
                continue;
            };
            let Some(reply) =
                build_fallback_reply(query, device_id.as_bytes(), &fingerprint, sync_port)
            else {
                continue;
            };
            let _ = socket.send_to(&reply, source);
        }
    });
    Ok(FallbackResponder {
        running,
        thread: Some(handle),
        #[cfg(test)]
        local_addr,
    })
}

fn build_fallback_reply(
    query: &[u8],
    device_id: &[u8; 16],
    fingerprint: &[u8; 32],
    sync_port: u16,
) -> Option<[u8; FALLBACK_REPLY_LEN]> {
    if query.len() != FALLBACK_QUERY_LEN || query.get(..16)? != FALLBACK_QUERY_MAGIC {
        return None;
    }
    let mut reply = [0_u8; FALLBACK_REPLY_LEN];
    reply[..16].copy_from_slice(FALLBACK_REPLY_MAGIC);
    reply[16..32].copy_from_slice(query.get(16..32)?);
    reply[32..48].copy_from_slice(device_id);
    reply[48..80].copy_from_slice(fingerprint);
    reply[80..82].copy_from_slice(&sync_port.to_be_bytes());
    Some(reply)
}

struct FallbackBrowser {
    socket: UdpSocket,
    target: SocketAddr,
    query: [u8; FALLBACK_QUERY_LEN],
    nonce: [u8; FALLBACK_NONCE_LEN],
    last_sent: std::cell::Cell<Instant>,
}

impl FallbackBrowser {
    fn start(target: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))?;
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?;
        let mut nonce = [0_u8; FALLBACK_NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce);
        let mut query = [0_u8; FALLBACK_QUERY_LEN];
        query[..16].copy_from_slice(FALLBACK_QUERY_MAGIC);
        query[16..32].copy_from_slice(&nonce);
        socket.send_to(&query, target)?;
        Ok(Self {
            socket,
            target,
            query,
            nonce,
            last_sent: std::cell::Cell::new(Instant::now()),
        })
    }

    fn retry_if_due(&self) {
        if self.last_sent.get().elapsed() >= FALLBACK_RETRY {
            let _ = self.socket.send_to(&self.query, self.target);
            self.last_sent.set(Instant::now());
        }
    }

    fn drain(&self) -> Vec<DiscoveredPeer> {
        let mut peers = Vec::new();
        let mut buffer = [0_u8; FALLBACK_REPLY_LEN];
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((length, source)) => {
                    let Some(reply) = buffer.get(..length) else {
                        continue;
                    };
                    if let Some(peer) = parse_fallback_reply(reply, &self.nonce, source) {
                        peers.push(peer);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        peers
    }
}

fn parse_fallback_reply(
    reply: &[u8],
    expected_nonce: &[u8; FALLBACK_NONCE_LEN],
    source: SocketAddr,
) -> Option<DiscoveredPeer> {
    if reply.len() != FALLBACK_REPLY_LEN
        || reply.get(..16)? != FALLBACK_REPLY_MAGIC
        || reply.get(16..32)? != expected_nonce
    {
        return None;
    }
    let device_bytes: [u8; 16] = reply.get(32..48)?.try_into().ok()?;
    let fingerprint_bytes: [u8; 32] = reply.get(48..80)?.try_into().ok()?;
    let port = u16::from_be_bytes(reply.get(80..82)?.try_into().ok()?);
    if port == 0 {
        return None;
    }
    Some(DiscoveredPeer {
        device_id: uuid::Uuid::from_bytes(device_bytes).to_string(),
        fingerprint: Fingerprint::from_bytes(fingerprint_bytes),
        addresses: vec![source.ip()],
        port,
    })
}

impl Drop for Discovery {
    fn drop(&mut self) {
        let _ = self.stop_advertising();
        let _ = self.daemon.shutdown();
    }
}

fn peer_from_service_info(info: &ServiceInfo) -> Option<DiscoveredPeer> {
    let fingerprint = Fingerprint::from_hex(info.get_property_val_str(PROP_FINGERPRINT)?).ok()?;
    let device_id = info.get_property_val_str(PROP_DEVICE_ID)?.to_string();
    let addresses = info.get_addresses().iter().copied().collect();
    Some(DiscoveredPeer {
        device_id,
        fingerprint,
        addresses,
        port: info.get_port(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::sync::Mutex;

    fn identity() -> DeviceIdentity {
        let path = temp_dir().join(format!(
            "envryn-discovery-test-{}.json",
            uuid::Uuid::now_v7()
        ));
        DeviceIdentity::load_or_create(&path).unwrap()
    }

    // Real multicast sockets, shared with every other test in this binary --
    // unlike the in-process channels the rest of the suite uses. Serialising
    // these tests keeps one test's advertisement from leaking into another's
    // browse when `cargo test` runs them on separate threads.
    static DISCOVERY_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Self-discovery: a service this process advertises is found by a
    /// browse issued from the same process. This is the one thing verifiable
    /// without a second physical machine -- see the module doc for what
    /// remains unverified.
    #[test]
    fn advertised_service_is_found_by_a_browse() {
        let _guard = DISCOVERY_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let id = identity();
        let mut disco = Discovery::new().unwrap();
        disco.advertise(&id, 4433).unwrap();

        let peers = disco.browse(Duration::from_secs(5)).unwrap();
        let found = peers
            .iter()
            .find(|p| p.device_id == id.device_id)
            .expect("self-advertised service should be found by a browse");

        assert_eq!(found.fingerprint, id.fingerprint());
        assert_eq!(found.port, 4433);
        assert!(!found.addresses.is_empty());
    }

    /// Advertising twice on the same `Discovery` must not leave the first
    /// registration dangling under a stale name.
    #[test]
    fn re_advertising_replaces_the_previous_registration() {
        let _guard = DISCOVERY_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let id = identity();
        let mut disco = Discovery::new().unwrap();
        disco.advertise(&id, 1111).unwrap();
        disco.advertise(&id, 2222).unwrap();

        let peers = disco.browse(Duration::from_secs(5)).unwrap();
        let found = peers
            .iter()
            .find(|p| p.device_id == id.device_id)
            .expect("re-advertised service should still be found by a browse");
        assert_eq!(
            found.port, 2222,
            "browse should reflect the latest advertisement, not the one it replaced"
        );
    }

    /// A browse with nothing advertised by this process must return promptly.
    ///
    /// Other Envryn instances can legitimately be present on the developer's
    /// LAN, so this test must not assume the multicast network is empty.
    #[test]
    fn browsing_without_a_local_advertisement_returns_promptly() {
        let _guard = DISCOVERY_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let disco = Discovery::new().unwrap();
        let started = std::time::Instant::now();
        let _peers = disco.browse(Duration::from_millis(500)).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "browse should respect its timeout even when other devices are visible"
        );
    }

    #[test]
    fn fallback_reply_round_trips_only_for_the_matching_probe() {
        let id = identity();
        let device_id = uuid::Uuid::parse_str(&id.device_id).unwrap();
        let nonce = [7_u8; FALLBACK_NONCE_LEN];
        let mut query = [0_u8; FALLBACK_QUERY_LEN];
        query[..16].copy_from_slice(FALLBACK_QUERY_MAGIC);
        query[16..32].copy_from_slice(&nonce);
        let reply = build_fallback_reply(
            &query,
            device_id.as_bytes(),
            id.fingerprint().as_bytes(),
            42_424,
        )
        .unwrap();
        let source: SocketAddr = "192.0.2.10:37853".parse().unwrap();

        let peer = parse_fallback_reply(&reply, &nonce, source).unwrap();
        assert_eq!(peer.device_id, id.device_id);
        assert_eq!(peer.fingerprint, id.fingerprint());
        assert_eq!(peer.addresses, vec![source.ip()]);
        assert_eq!(peer.port, 42_424);

        let wrong_nonce = [8_u8; FALLBACK_NONCE_LEN];
        assert!(parse_fallback_reply(&reply, &wrong_nonce, source).is_none());
        assert!(build_fallback_reply(
            b"not an Envryn probe",
            device_id.as_bytes(),
            id.fingerprint().as_bytes(),
            1
        )
        .is_none());
    }

    #[test]
    fn fallback_responder_is_discovered_over_real_udp() {
        let id = identity();
        let responder =
            start_fallback_responder(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), &id, 31_337)
                .unwrap();
        let browser = FallbackBrowser::start(responder.local_addr).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let peer = loop {
            if let Some(found) = browser.drain().into_iter().next() {
                break found;
            }
            assert!(Instant::now() < deadline, "fallback reply should arrive");
            thread::sleep(Duration::from_millis(20));
        };

        assert_eq!(peer.device_id, id.device_id);
        assert_eq!(peer.fingerprint, id.fingerprint());
        assert_eq!(peer.addresses, vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]);
        assert_eq!(peer.port, 31_337);
    }

    #[test]
    fn duplicate_discovery_paths_merge_addresses() {
        let id = identity();
        let first = DiscoveredPeer {
            device_id: id.device_id.clone(),
            fingerprint: id.fingerprint(),
            addresses: vec!["192.0.2.1".parse().unwrap()],
            port: 1000,
        };
        let second = DiscoveredPeer {
            device_id: id.device_id.clone(),
            fingerprint: id.fingerprint(),
            addresses: vec!["192.0.2.2".parse().unwrap()],
            port: 2000,
        };
        let mut peers = HashMap::new();
        merge_peer(&mut peers, first);
        merge_peer(&mut peers, second);

        let merged = peers.get(&id.device_id).unwrap();
        assert_eq!(merged.addresses.len(), 2);
        assert_eq!(merged.port, 2000);
    }
}
