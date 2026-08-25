//! Direct device-to-device LAN sync.
//!
//! This is the one part of `envryn-core` permitted to open network
//! connections -- the deliberate exception INV-010 carves out for "LAN sync
//! with an already-paired device." Nothing outside this module may depend on
//! the networking crates it pulls in; see docs/DEPENDENCY_POLICY.md.
//!
//! ```text
//! identity    one Ed25519 keypair per installation, sealed via platform::dpapi_protect
//! pairing     SPAKE2 (manual code) or ECDH (QR), converging on a 6-digit SAS
//!             the user confirms before the VMK is transferred
//! transport   TLS 1.3, mutual auth, custom verifier pinned to trusted_devices
//! protocol    HLC-ordered manifest exchange; last-writer-wins, losing side kept
//! discovery   mDNS advertise/browse -- discovery grants no trust
//! ```
//!
//! AI has no involvement anywhere in this module (specification section 40).

pub mod discovery;
pub mod handshake;
pub mod identity;
pub mod pairing;
pub mod protocol;
pub mod transport;

pub use crate::storage::Hlc;
pub use identity::{DeviceIdentity, Fingerprint};
