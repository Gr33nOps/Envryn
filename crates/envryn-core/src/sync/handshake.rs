//! Carries a pairing key agreement (SPAKE2 or ECDH) over an actual
//! connection.
//!
//! [`crate::sync::pairing`] defines the cryptography and is deliberately
//! transport-agnostic; this module is the one place that puts its messages
//! on a wire. Both pairing paths turn out to need the exact same shape of
//! exchange -- send one outbound message carrying this device's claimed
//! identity and key-agreement payload, receive the peer's equivalent, then
//! hand both to the path-specific `finish` -- so [`run_manual_pairing`] and
//! [`run_qr_pairing`] are thin wrappers over one shared [`exchange`].
//!
//! Framing reuses [`crate::sync::protocol`]'s length-prefixed JSON rather
//! than inventing a second wire format for what is, structurally, the same
//! kind of message.
//!
//! As `sync::pairing`'s module doc explains, this exchange does not need an
//! authenticated transport: eavesdropping learns nothing exploitable, and an
//! active man-in-the-middle is caught by the SAS comparison the human
//! performs afterward, not by anything at this layer.

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::sync::identity::Fingerprint;
use crate::sync::pairing::{ManualPairing, PairingSession, QrPairing};
use crate::sync::protocol::{read_json, write_json};

/// The peer's claimed identity, as read off the wire during the handshake.
/// Not yet trusted for anything beyond "this is who to compute the SAS
/// against" -- the human confirming the SAS is what the caller relies on
/// before treating this as a real peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerHandshakeInfo {
    pub device_id: String,
    pub fingerprint: Fingerprint,
}

#[derive(Serialize, Deserialize)]
struct HandshakeMessage {
    device_id: String,
    fingerprint: String,
    payload: Vec<u8>,
}

fn exchange<S: Read + Write>(
    stream: &mut S,
    my_device_id: &str,
    my_fingerprint: Fingerprint,
    my_payload: Vec<u8>,
    finish: impl FnOnce(&[u8], &str, Fingerprint) -> Result<PairingSession>,
) -> Result<(PairingSession, PeerHandshakeInfo)> {
    write_json(
        stream,
        &HandshakeMessage {
            device_id: my_device_id.to_string(),
            fingerprint: my_fingerprint.to_hex(),
            payload: my_payload,
        },
    )?;
    let inbound: HandshakeMessage = read_json(stream)?;

    let their_fingerprint =
        Fingerprint::from_hex(&inbound.fingerprint).map_err(|_| Error::AuthenticationFailed)?;
    let session = finish(&inbound.payload, &inbound.device_id, their_fingerprint)?;
    Ok((
        session,
        PeerHandshakeInfo {
            device_id: inbound.device_id,
            fingerprint: their_fingerprint,
        },
    ))
}

/// Run manual-code (SPAKE2) pairing over an already-connected stream.
/// Symmetric -- it makes no difference which side dialled and which side
/// accepted, matching `ManualPairing`'s own symmetric SPAKE2 mode.
pub fn run_manual_pairing<S: Read + Write>(
    stream: &mut S,
    code: &str,
    my_device_id: &str,
    my_fingerprint: Fingerprint,
) -> Result<(PairingSession, PeerHandshakeInfo)> {
    let (session, outbound) = ManualPairing::start(code, my_device_id, my_fingerprint);
    exchange(
        stream,
        my_device_id,
        my_fingerprint,
        outbound,
        |msg, id, fp| session.finish(msg, id, fp),
    )
}

/// Run QR (ECDH) pairing over an already-connected stream.
///
/// The QR code itself carries only the address to connect to plus the
/// displaying device's claimed id/fingerprint (for the caller to show
/// "connecting to <name>" before a key even exists) -- the ephemeral X25519
/// public keys that actually key the exchange travel over this connection
/// exactly like a SPAKE2 message does. This is a deliberate simplification
/// versus a design that embeds the public key in the QR image itself: it
/// keeps both pairing paths structurally identical at the network layer
/// (see the module doc), and loses nothing, since a MITM substituting a key
/// in transit is still caught by the SAS comparison either way.
pub fn run_qr_pairing<S: Read + Write>(
    stream: &mut S,
    my_device_id: &str,
    my_fingerprint: Fingerprint,
) -> Result<(PairingSession, PeerHandshakeInfo)> {
    let session = QrPairing::start(my_device_id, my_fingerprint);
    let outbound = session.public_key.to_vec();
    exchange(
        stream,
        my_device_id,
        my_fingerprint,
        outbound,
        |msg, id, fp| {
            let key: [u8; 32] = msg.try_into().map_err(|_| Error::AuthenticationFailed)?;
            session.finish(key, id, fp)
        },
    )
}

/// Send the sealed VMK over an already-paired connection, once the human has
/// confirmed the SAS on both ends.
///
/// Deliberately separate from [`run_manual_pairing`]/[`run_qr_pairing`]: the
/// key-agreement handshake is something this module owns end-to-end, but
/// *whether and when* to actually hand over vault key material is a
/// vault-layer decision (has the caller re-confirmed the current password?
/// did the human actually click "confirm"?) that belongs to the caller, not
/// to this module. This function only does the framing.
pub fn send_sealed_vmk<S: Write>(stream: &mut S, sealed: &[u8]) -> Result<()> {
    write_json(stream, &sealed.to_vec())
}

/// Receive the sealed VMK sent by [`send_sealed_vmk`]. Pass the result to
/// [`crate::sync::pairing::open_vmk`].
pub fn receive_sealed_vmk<S: Read>(stream: &mut S) -> Result<Vec<u8>> {
    read_json(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn fp(byte: u8) -> Fingerprint {
        Fingerprint::of_raw_bytes(&[byte; 32])
    }

    /// Real loopback TCP, real SPAKE2, real framing -- both sides run this
    /// module's public function exactly as the IPC layer will, and end up
    /// agreeing on the same SAS and peer identity.
    #[test]
    fn manual_pairing_converges_over_real_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            run_manual_pairing(&mut stream, "778899", "device-server", fp(1)).unwrap()
        });

        let mut client_stream = std::net::TcpStream::connect(addr).unwrap();
        let (client_session, client_peer) =
            run_manual_pairing(&mut client_stream, "778899", "device-client", fp(2)).unwrap();

        let (server_session, server_peer) = server.join().unwrap();

        assert_eq!(client_session.sas().unwrap(), server_session.sas().unwrap());
        assert_eq!(client_peer.device_id, "device-server");
        assert_eq!(client_peer.fingerprint, fp(1));
        assert_eq!(server_peer.device_id, "device-client");
        assert_eq!(server_peer.fingerprint, fp(2));
    }

    /// Mismatched codes must not silently converge just because the network
    /// plumbing worked -- the SAS values must differ, same as the in-process
    /// test in `sync::pairing`.
    #[test]
    fn manual_pairing_with_different_codes_disagrees_over_real_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            run_manual_pairing(&mut stream, "111111", "device-server", fp(1))
        });

        let mut client_stream = std::net::TcpStream::connect(addr).unwrap();
        let client_result =
            run_manual_pairing(&mut client_stream, "222222", "device-client", fp(2));

        let server_result = server.join().unwrap();
        if let (Ok((cs, _)), Ok((ss, _))) = (client_result, server_result) {
            assert_ne!(cs.sas().unwrap(), ss.sas().unwrap());
        }
    }

    #[test]
    fn qr_pairing_converges_over_real_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            run_qr_pairing(&mut stream, "device-server", fp(3)).unwrap()
        });

        let mut client_stream = std::net::TcpStream::connect(addr).unwrap();
        let (client_session, client_peer) =
            run_qr_pairing(&mut client_stream, "device-client", fp(4)).unwrap();

        let (server_session, server_peer) = server.join().unwrap();

        assert_eq!(client_session.sas().unwrap(), server_session.sas().unwrap());
        assert_eq!(client_peer.device_id, "device-server");
        assert_eq!(server_peer.device_id, "device-client");
    }

    /// End-to-end: after the network handshake and (simulated) human
    /// confirmation, the VMK itself round-trips over the same connection.
    #[test]
    fn vmk_transfers_over_the_paired_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let vmk = crate::crypto::keys::VaultMasterKey::from_key(
            crate::crypto::keys::SymmetricKey::from_bytes([42u8; 32]),
        );
        let vmk_bytes = *vmk.expose_bytes();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let (session, _peer) =
                run_manual_pairing(&mut stream, "555555", "host", fp(5)).unwrap();
            let sealed = crate::sync::pairing::seal_vmk(&session, &vmk).unwrap();
            send_sealed_vmk(&mut stream, &sealed).unwrap();
        });

        let mut client_stream = std::net::TcpStream::connect(addr).unwrap();
        let (client_session, _peer) =
            run_manual_pairing(&mut client_stream, "555555", "joiner", fp(6)).unwrap();
        let sealed = receive_sealed_vmk(&mut client_stream).unwrap();
        let recovered = crate::sync::pairing::open_vmk(&client_session, &sealed).unwrap();

        server.join().unwrap();
        assert_eq!(*recovered.expose_bytes(), vmk_bytes);
    }
}
