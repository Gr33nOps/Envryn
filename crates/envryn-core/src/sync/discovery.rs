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
use std::net::IpAddr;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::error::{Error, Result};
use crate::sync::identity::{DeviceIdentity, Fingerprint};

/// The one service type Envryn advertises and browses for.
pub const SERVICE_TYPE: &str = "_envryn._tcp.local.";

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
        })
    }

    /// Advertise this device on the LAN so paired peers can find it by mDNS.
    ///
    /// `port` is the mutual-TLS listener's port; binding and owning that
    /// listener is the caller's responsibility (the IPC layer), not this
    /// module's -- discovery only announces where it is.
    pub fn advertise(&mut self, identity: &DeviceIdentity, port: u16) -> Result<()> {
        self.stop_advertising()?;

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
            match receiver.recv_timeout(remaining) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    if let Some(peer) = peer_from_service_info(&info) {
                        // A device also advertises to itself on the same
                        // daemon; the caller decides whether to filter that
                        // out (it knows its own device id, this module
                        // doesn't).
                        peers.insert(peer.device_id.clone(), peer);
                    }
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        let _ = self.daemon.stop_browse(SERVICE_TYPE);
        Ok(peers.into_values().collect())
    }
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
}
