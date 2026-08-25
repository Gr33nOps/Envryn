//! TLS 1.3, mutually authenticated, pinned to `trusted_devices` fingerprints.
//!
//! No certificate authority, no hostname validation, no expiry check --
//! self-signed certificates exist here only to carry a public key over the
//! standard TLS handshake machinery. The pinned fingerprint set *is* the
//! trust store: a peer certificate is accepted if and only if
//! `SHA-256(its raw Ed25519 public key)` is currently in
//! [`TrustedFingerprints`], checked fresh on every handshake. Revoking a
//! device is therefore a row delete that fails the *next* handshake attempt
//! directly, with no separate authorisation check elsewhere that a future
//! refactor could accidentally bypass (INV-104).
//!
//! Certificates are generated with rcgen's default (effectively unbounded)
//! validity window and expiry is never checked here -- deliberately.
//! Revocation-by-fingerprint-removal is the only revocation mechanism;
//! adding certificate expiry would be complexity bought for no additional
//! security this model does not already provide.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    CertificateError, DigitallySignedStruct, DistinguishedName, Error as TlsError, SignatureScheme,
};

use crate::error::{Error, Result};
use crate::sync::identity::{DeviceIdentity, Fingerprint};

/// Install the process-wide default crypto provider (`ring`), exactly once.
/// `rustls::ClientConfig::builder()`/`ServerConfig::builder()` need one
/// installed before they can be called; every entry point in this module
/// goes through this first.
fn ensure_crypto_provider_installed() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // Installing twice would only be possible from a second call site;
        // Once already prevents that, so the Err (already installed) case
        // can never actually occur here.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// A shared, mutable set of fingerprints the transport currently trusts.
/// Handed to the verifier by reference so it always reflects the live
/// `trusted_devices` table, not a snapshot taken when the TLS config was
/// built -- a device revoked mid-session-list still fails its *next*
/// connection attempt without rebuilding any config.
#[derive(Clone, Default, Debug)]
pub struct TrustedFingerprints(Arc<RwLock<HashSet<[u8; 32]>>>);

impl TrustedFingerprints {
    pub fn new(initial: impl IntoIterator<Item = Fingerprint>) -> Self {
        let set = initial.into_iter().map(|f| *f.as_bytes()).collect();
        Self(Arc::new(RwLock::new(set)))
    }

    pub fn insert(&self, fp: Fingerprint) {
        if let Ok(mut set) = self.0.write() {
            set.insert(*fp.as_bytes());
        }
    }

    pub fn remove(&self, fp: Fingerprint) {
        if let Ok(mut set) = self.0.write() {
            set.remove(fp.as_bytes());
        }
    }

    fn contains(&self, fp: &Fingerprint) -> bool {
        self.0
            .read()
            .map(|set| set.contains(fp.as_bytes()))
            .unwrap_or(false)
    }
}

/// Extract the raw Ed25519 public key from a presented certificate's SPKI and
/// check it against the trusted set. The one place both the client- and
/// server-side verifiers actually decide anything; everything else in this
/// module is signature-verification boilerplate delegated to rustls itself.
fn check_fingerprint(
    end_entity: &CertificateDer<'_>,
    trusted: &TrustedFingerprints,
) -> std::result::Result<(), TlsError> {
    let (_, parsed) = x509_parser::parse_x509_certificate(end_entity.as_ref())
        .map_err(|_| TlsError::InvalidCertificate(CertificateError::BadEncoding))?;
    let raw_key = parsed
        .tbs_certificate
        .subject_pki
        .subject_public_key
        .data
        .as_ref();
    let fingerprint = Fingerprint::of_raw_bytes(raw_key);

    if trusted.contains(&fingerprint) {
        Ok(())
    } else {
        Err(TlsError::InvalidCertificate(
            CertificateError::ApplicationVerificationFailure,
        ))
    }
}

#[derive(Debug)]
struct FingerprintVerifier {
    trusted: TrustedFingerprints,
    algs: WebPkiSupportedAlgorithms,
}

impl FingerprintVerifier {
    fn new(trusted: TrustedFingerprints) -> Self {
        Self {
            trusted,
            algs: rustls::crypto::ring::default_provider().signature_verification_algorithms,
        }
    }
}

impl ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, TlsError> {
        check_fingerprint(end_entity, &self.trusted)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

impl ClientCertVerifier for FingerprintVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> std::result::Result<ClientCertVerified, TlsError> {
        check_fingerprint(end_entity, &self.trusted)?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

fn own_cert_and_key(
    identity: &DeviceIdentity,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let (cert_der, key_der) = identity.build_certificate()?;
    let cert_chain = vec![CertificateDer::from(cert_der)];
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der));
    Ok((cert_chain, key))
}

/// Build the server side of a sync/pairing-confirmation TLS listener: accepts
/// only peers whose certificate's fingerprint is currently in `trusted`.
pub fn server_config(
    identity: &DeviceIdentity,
    trusted: TrustedFingerprints,
) -> Result<rustls::ServerConfig> {
    ensure_crypto_provider_installed();
    let (cert_chain, key) = own_cert_and_key(identity)?;
    let verifier = Arc::new(FingerprintVerifier::new(trusted));

    rustls::ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(cert_chain, key)
        .map_err(|_| Error::Internal("could not build TLS server configuration"))
}

/// Build the client side: connects only if the *peer's* certificate matches
/// one of `trusted`'s fingerprints, and presents this device's own
/// certificate for the peer's matching check.
pub fn client_config(
    identity: &DeviceIdentity,
    trusted: TrustedFingerprints,
) -> Result<rustls::ClientConfig> {
    ensure_crypto_provider_installed();
    let (cert_chain, key) = own_cert_and_key(identity)?;
    let verifier = Arc::new(FingerprintVerifier::new(trusted));

    rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_client_auth_cert(cert_chain, key)
        .map_err(|_| Error::Internal("could not build TLS client configuration"))
}

/// A placeholder server name for the client handshake. Never validated
/// against anything -- `FingerprintVerifier` ignores it entirely and checks
/// only the certificate's public key fingerprint -- but the TLS 1.3 API
/// requires one to be supplied.
pub fn placeholder_server_name() -> ServerName<'static> {
    ServerName::try_from("envryn-device").unwrap_or(ServerName::IpAddress(
        rustls::pki_types::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST.into()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    fn identity() -> (tempfile::TempDir, DeviceIdentity) {
        let dir = tempfile::tempdir().unwrap();
        let identity = DeviceIdentity::load_or_create(&dir.path().join("id.json")).unwrap();
        (dir, identity)
    }

    /// End-to-end mutual TLS over a real loopback TCP socket between two
    /// independent device identities, each trusting only the other's
    /// fingerprint -- the two-party test stands in for two physical devices,
    /// which is unavailable in this environment, without mocking anything
    /// about the handshake itself.
    #[test]
    fn mutual_tls_succeeds_between_devices_that_trust_each_other() {
        let (_dir_a, id_a) = identity();
        let (_dir_b, id_b) = identity();

        let trusted_by_a = TrustedFingerprints::new([id_b.fingerprint()]);
        let trusted_by_b = TrustedFingerprints::new([id_a.fingerprint()]);

        let server_conf = Arc::new(server_config(&id_a, trusted_by_a).unwrap());
        let client_conf = Arc::new(client_config(&id_b, trusted_by_b).unwrap());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_thread = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut conn = rustls::ServerConnection::new(server_conf).unwrap();
            let mut stream = stream;
            let mut tls = rustls::Stream::new(&mut conn, &mut stream);
            let mut buf = [0u8; 5];
            tls.read_exact(&mut buf).unwrap();
            tls.write_all(b"world").unwrap();
            buf
        });

        let stream = TcpStream::connect(addr).unwrap();
        let mut conn =
            rustls::ClientConnection::new(client_conf, placeholder_server_name()).unwrap();
        let mut stream = stream;
        let mut tls = rustls::Stream::new(&mut conn, &mut stream);
        tls.write_all(b"hello").unwrap();
        let mut buf = [0u8; 5];
        tls.read_exact(&mut buf).unwrap();

        assert_eq!(&buf, b"world");
        assert_eq!(&server_thread.join().unwrap(), b"hello");
    }

    /// The entire point of this transport: a device whose fingerprint is not
    /// in the trusted set must have its handshake rejected, not merely
    /// warned about.
    #[test]
    fn handshake_fails_when_client_is_not_trusted() {
        let (_dir_a, id_a) = identity();
        let (_dir_b, id_b) = identity();
        let (_dir_stranger, id_stranger) = identity();

        // A trusts B, but the connecting party will actually be "stranger."
        let trusted_by_a = TrustedFingerprints::new([id_b.fingerprint()]);
        let trusted_by_stranger = TrustedFingerprints::new([id_a.fingerprint()]);

        let server_conf = Arc::new(server_config(&id_a, trusted_by_a).unwrap());
        let client_conf = Arc::new(client_config(&id_stranger, trusted_by_stranger).unwrap());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_thread = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut conn = rustls::ServerConnection::new(server_conf).unwrap();
            let mut stream = stream;
            let mut tls = rustls::Stream::new(&mut conn, &mut stream);
            // The handshake itself happens lazily on first read/write; a
            // rejected client certificate surfaces as an I/O error here.
            let mut buf = [0u8; 1];
            tls.read_exact(&mut buf).is_err()
        });

        let stream = TcpStream::connect(addr).unwrap();
        let mut conn =
            rustls::ClientConnection::new(client_conf, placeholder_server_name()).unwrap();
        let mut stream = stream;
        let mut tls = rustls::Stream::new(&mut conn, &mut stream);
        // The client's own write may succeed at the TCP layer even though
        // the server has already rejected the certificate -- what matters is
        // that the server-side read never completes normally.
        let _ = tls.write_all(b"x");

        assert!(
            server_thread.join().unwrap(),
            "server accepted an untrusted client certificate"
        );
    }

    /// A device revoked from the trusted set (row deleted from
    /// trusted_devices, modelled here as removing it from the live set)
    /// must fail its next handshake -- this is what makes revocation a
    /// property of the handshake itself rather than a check a caller could
    /// forget to make (INV-104).
    #[test]
    fn revoked_fingerprint_is_rejected_on_the_next_handshake() {
        let (_dir_a, id_a) = identity();
        let (_dir_b, id_b) = identity();

        let trusted_by_a = TrustedFingerprints::new([id_b.fingerprint()]);
        let trusted_by_b = TrustedFingerprints::new([id_a.fingerprint()]);

        // Revoke B before any connection is attempted.
        trusted_by_a.remove(id_b.fingerprint());

        let server_conf = Arc::new(server_config(&id_a, trusted_by_a).unwrap());
        let client_conf = Arc::new(client_config(&id_b, trusted_by_b).unwrap());

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_thread = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut conn = rustls::ServerConnection::new(server_conf).unwrap();
            let mut stream = stream;
            let mut tls = rustls::Stream::new(&mut conn, &mut stream);
            let mut buf = [0u8; 1];
            tls.read_exact(&mut buf).is_err()
        });

        let stream = TcpStream::connect(addr).unwrap();
        let mut conn =
            rustls::ClientConnection::new(client_conf, placeholder_server_name()).unwrap();
        let mut stream = stream;
        let mut tls = rustls::Stream::new(&mut conn, &mut stream);
        let _ = tls.write_all(b"x");

        assert!(
            server_thread.join().unwrap(),
            "server accepted a revoked device"
        );
    }
}
