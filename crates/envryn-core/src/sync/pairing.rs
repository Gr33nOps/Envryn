//! Pairing: establishing a shared secret with a new device, and turning it
//! into a confirmed, VMK-transferring trust relationship.
//!
//! Two paths converge on the same downstream logic:
//!
//! - **Manual code** (Windows <-> Windows, no camera): SPAKE2. A short code
//!   cannot safely key a plain Diffie-Hellman exchange -- an attacker who
//!   intercepts it could brute-force a 6- or 8-character code offline at
//!   leisure. SPAKE2 is a password-authenticated key exchange built for
//!   exactly this: one online guess per attempt, nothing learned from a
//!   failed one.
//! - **QR** (Windows <-> Android): X25519 ECDH. The QR code is the
//!   out-of-band channel; because it carries a full ephemeral public key
//!   rather than a short code, plain ECDH is safe here.
//!
//! Both produce raw shared-secret bytes, which [`PairingSession`] normalises
//! (via HKDF) into a 6-digit SAS the user compares on both screens, and a
//! transfer key that AEAD-seals the VMK -- never sent until the human
//! confirms the SAS matches.
//!
//! **Deliberately not run over an authenticated transport.** Pairing
//! messages travel over a plain connection (see `sync::protocol`), and that
//! is not a gap: a passive eavesdropper who captures a SPAKE2 message or an
//! ECDH public key learns nothing exploitable -- that is the entire point of
//! those primitives -- and the one genuinely sensitive artefact, the VMK
//! transfer, is independently AEAD-sealed under the derived key regardless
//! of what the transport does. The SAS comparison, not the transport, is
//! what defeats an active machine-in-the-middle: a MITM that substitutes its
//! own keys produces a different shared secret on each side, hence a
//! different SAS, which the user is asked to notice.

use hkdf::Hkdf;
use rand_core::OsRng;
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

use crate::crypto::keys::SymmetricKey;
use crate::error::{Error, Result};
use crate::sync::identity::Fingerprint;

const SPAKE_IDENTITY: &[u8] = b"envryn/v1/pairing";
const INFO_SAS: &[u8] = b"envryn/v1/sas";
const INFO_TRANSFER: &[u8] = b"envryn/v1/pairing-transfer";

/// A canonical, order-independent transcript binding both devices' claimed
/// identities into the SAS. A man-in-the-middle who substitutes either
/// device's identity partway through produces a transcript -- and therefore
/// a SAS -- that does not match what the honest peer computes.
fn transcript(a: &str, fp_a: Fingerprint, b: &str, fp_b: Fingerprint) -> Vec<u8> {
    let mut left = a.as_bytes().to_vec();
    left.extend_from_slice(fp_a.as_bytes());
    let mut right = b.as_bytes().to_vec();
    right.extend_from_slice(fp_b.as_bytes());
    // Sorting makes the transcript identical regardless of which side is
    // "device A" in the exchange -- there is no initiator/responder
    // asymmetry once both sides know both identities.
    if left <= right {
        [left, right].concat()
    } else {
        [right, left].concat()
    }
}

/// A completed key agreement, not yet confirmed by the user.
pub struct PairingSession {
    key: SymmetricKey,
}

impl PairingSession {
    fn from_shared_secret(shared_secret: &[u8], transcript: &[u8]) -> Result<Self> {
        // HKDF folds the transcript in as context, so the SAS and transfer
        // key are bound to *which two devices* are pairing, not only to the
        // raw Diffie-Hellman/SPAKE2 output.
        let hk = Hkdf::<Sha256>::new(Some(transcript), shared_secret);
        let mut root = [0u8; 32];
        hk.expand(b"envryn/v1/pairing-root", &mut root)
            .map_err(|_| Error::Internal("pairing key derivation failed"))?;
        Ok(Self {
            key: SymmetricKey::from_bytes(root),
        })
    }

    /// The 6-digit string both devices display for the human to compare.
    /// Matching digits is what actually defeats an active MITM here -- see
    /// the module docs.
    pub fn sas(&self) -> Result<String> {
        let subkey = self.key.derive_subkey(INFO_SAS)?;
        let head = subkey
            .as_slice()
            .get(..4)
            .ok_or(Error::Internal("unexpected key length"))?;
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(head);
        let value = u32::from_be_bytes(bytes) % 1_000_000;
        Ok(format!("{value:06}"))
    }

    /// The key that AEAD-seals the VMK for transfer. Never used before the
    /// SAS has been confirmed by the human on both ends -- enforced by
    /// caller discipline in `sync::protocol`, not by this type, since the
    /// confirmation is fundamentally a UI step this crate cannot see.
    pub fn transfer_key(&self) -> Result<SymmetricKey> {
        self.key.derive_subkey(INFO_TRANSFER)
    }
}

/// Manual-code pairing (SPAKE2). Symmetric: neither side is fixed as
/// "initiator," since either Windows machine might be the one showing the
/// code.
pub struct ManualPairing {
    spake: Spake2<Ed25519Group>,
    device_id: String,
    fingerprint: Fingerprint,
}

impl ManualPairing {
    /// Start a session from a short code both humans typed. Returns the
    /// outbound message to send to the peer.
    pub fn start(code: &str, device_id: &str, fingerprint: Fingerprint) -> (Self, Vec<u8>) {
        let (spake, outbound) = Spake2::<Ed25519Group>::start_symmetric(
            &Password::new(code.trim().as_bytes()),
            &Identity::new(SPAKE_IDENTITY),
        );
        (
            Self {
                spake,
                device_id: device_id.to_string(),
                fingerprint,
            },
            outbound,
        )
    }

    /// Complete the exchange given the peer's outbound message and claimed
    /// identity, producing a confirmed-but-not-yet-user-verified session.
    pub fn finish(
        self,
        their_message: &[u8],
        their_device_id: &str,
        their_fingerprint: Fingerprint,
    ) -> Result<PairingSession> {
        let shared_secret = self
            .spake
            .finish(their_message)
            .map_err(|_| Error::AuthenticationFailed)?;
        let t = transcript(
            &self.device_id,
            self.fingerprint,
            their_device_id,
            their_fingerprint,
        );
        PairingSession::from_shared_secret(&shared_secret, &t)
    }
}

/// QR pairing (X25519 ECDH). The QR-displaying device generates the
/// ephemeral keypair advertised in the code; the scanning device generates
/// its own and completes the exchange.
pub struct QrPairing {
    secret: EphemeralSecret,
    pub public_key: [u8; 32],
    device_id: String,
    fingerprint: Fingerprint,
}

impl QrPairing {
    pub fn start(device_id: &str, fingerprint: Fingerprint) -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public_key = *X25519PublicKey::from(&secret).as_bytes();
        Self {
            secret,
            public_key,
            device_id: device_id.to_string(),
            fingerprint,
        }
    }

    pub fn finish(
        self,
        their_public_key: [u8; 32],
        their_device_id: &str,
        their_fingerprint: Fingerprint,
    ) -> Result<PairingSession> {
        let their_key = X25519PublicKey::from(their_public_key);
        let shared_secret = self.secret.diffie_hellman(&their_key);
        let t = transcript(
            &self.device_id,
            self.fingerprint,
            their_device_id,
            their_fingerprint,
        );
        PairingSession::from_shared_secret(shared_secret.as_bytes(), &t)
    }
}

/// Seal the VMK for transfer once the user has confirmed the SAS matches.
pub fn seal_vmk(
    session: &PairingSession,
    vmk: &crate::crypto::keys::VaultMasterKey,
) -> Result<Vec<u8>> {
    let key = session.transfer_key()?;
    let vmk_bytes = vmk.expose_bytes();
    let sealed = crate::crypto::aead::seal(&key, vmk_bytes.as_slice(), b"envryn/v1/vmk-transfer")?;
    Ok(sealed.into_bytes())
}

/// Recover the VMK from a transfer message. Any failure -- wrong session
/// (SAS mismatch), tampered message -- is `AuthenticationFailed`, matching
/// the same no-detail rule the vault's own unlock uses (INV-006): a peer
/// probing for which guess was closer should learn nothing either way.
pub fn open_vmk(
    session: &PairingSession,
    sealed: &[u8],
) -> Result<crate::crypto::keys::VaultMasterKey> {
    let key = session.transfer_key()?;
    let sealed = crate::crypto::aead::Sealed::from_bytes(sealed.to_vec())
        .map_err(|_| Error::AuthenticationFailed)?;
    let plaintext = crate::crypto::aead::open(&key, &sealed, b"envryn/v1/vmk-transfer")
        .map_err(|_| Error::AuthenticationFailed)?;
    if plaintext.len() != 32 {
        return Err(Error::AuthenticationFailed);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&plaintext);
    Ok(crate::crypto::keys::VaultMasterKey::from_key(
        SymmetricKey::from_bytes(out),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(byte: u8) -> Fingerprint {
        Fingerprint::of_raw_bytes(&[byte; 32])
    }

    #[test]
    fn manual_pairing_agrees_on_the_same_sas() {
        let (a, msg_a) = ManualPairing::start("482913", "device-a", fp(1));
        let (b, msg_b) = ManualPairing::start("482913", "device-b", fp(2));

        let session_a = a.finish(&msg_b, "device-b", fp(2)).unwrap();
        let session_b = b.finish(&msg_a, "device-a", fp(1)).unwrap();

        assert_eq!(session_a.sas().unwrap(), session_b.sas().unwrap());
    }

    /// The entire security property of SPAKE2: two sessions started with
    /// *different* codes must not agree on anything, including the SAS a
    /// human might otherwise be tricked into confirming.
    #[test]
    fn manual_pairing_with_different_codes_disagrees() {
        let (a, msg_a) = ManualPairing::start("111111", "device-a", fp(1));
        let (b, msg_b) = ManualPairing::start("222222", "device-b", fp(2));

        let session_a = a.finish(&msg_b, "device-b", fp(2));
        let session_b = b.finish(&msg_a, "device-a", fp(1));

        // SPAKE2 with mismatched passwords still "finishes" (it cannot know
        // the password was wrong), but the derived secrets differ, so the
        // SAS values must not match.
        if let (Ok(sa), Ok(sb)) = (session_a, session_b) {
            assert_ne!(sa.sas().unwrap(), sb.sas().unwrap());
        }
    }

    #[test]
    fn qr_pairing_agrees_on_the_same_sas() {
        let a = QrPairing::start("device-a", fp(1));
        let b = QrPairing::start("device-b", fp(2));
        let a_pub = a.public_key;
        let b_pub = b.public_key;

        let session_a = a.finish(b_pub, "device-b", fp(2)).unwrap();
        let session_b = b.finish(a_pub, "device-a", fp(1)).unwrap();

        assert_eq!(session_a.sas().unwrap(), session_b.sas().unwrap());
    }

    /// A machine-in-the-middle substituting its own ephemeral key on one leg
    /// produces a shared secret -- and SAS -- that does not match what the
    /// honest peer computes, which is the mechanism that makes the human
    /// comparison meaningful.
    #[test]
    fn qr_pairing_mitm_produces_a_different_sas() {
        let a = QrPairing::start("device-a", fp(1));
        let b = QrPairing::start("device-b", fp(2));
        let mitm = QrPairing::start("device-a", fp(1)); // impersonating A's identity claim

        let session_a = a.finish(mitm.public_key, "device-b", fp(2)).unwrap();
        let session_b = b.finish(mitm.public_key, "device-a", fp(1)).unwrap();

        assert_ne!(session_a.sas().unwrap(), session_b.sas().unwrap());
    }

    #[test]
    fn sas_is_six_digits() {
        let a = QrPairing::start("device-a", fp(1));
        let b = QrPairing::start("device-b", fp(2));
        let session = a.finish(b.public_key, "device-b", fp(2)).unwrap();
        let sas = session.sas().unwrap();
        assert_eq!(sas.len(), 6);
        assert!(sas.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn vmk_transfer_round_trips_after_confirmation() {
        let a = QrPairing::start("device-a", fp(1));
        let b = QrPairing::start("device-b", fp(2));
        let a_pub = a.public_key;
        let session_a = a.finish(b.public_key, "device-b", fp(2)).unwrap();
        let session_b = b.finish(a_pub, "device-a", fp(1)).unwrap();

        let vmk =
            crate::crypto::keys::VaultMasterKey::from_key(SymmetricKey::from_bytes([7u8; 32]));
        let sealed = seal_vmk(&session_a, &vmk).unwrap();
        let recovered = open_vmk(&session_b, &sealed).unwrap();
        assert_eq!(*recovered.expose_bytes(), *vmk.expose_bytes());
    }

    #[test]
    fn vmk_transfer_fails_across_mismatched_sessions() {
        let a = QrPairing::start("device-a", fp(1));
        let b = QrPairing::start("device-b", fp(2));
        let session_a = a.finish(b.public_key, "device-b", fp(2)).unwrap();

        // A different, unrelated session -- stands in for "the SAS did not
        // match and this transfer should never have been attempted."
        let c = QrPairing::start("device-c", fp(3));
        let d = QrPairing::start("device-d", fp(4));
        let unrelated = c.finish(d.public_key, "device-d", fp(4)).unwrap();

        let vmk =
            crate::crypto::keys::VaultMasterKey::from_key(SymmetricKey::from_bytes([9u8; 32]));
        let sealed = seal_vmk(&session_a, &vmk).unwrap();
        assert!(matches!(
            open_vmk(&unrelated, &sealed),
            Err(Error::AuthenticationFailed)
        ));
    }
}
