# Envryn — Cryptography

This document specifies the cryptography Envryn uses. It is normative: the implementation
follows this document, and where the two disagree, one of them is a bug.

**Design rule:** Envryn uses well-reviewed primitives from well-reviewed libraries and invents
no protocols. Where a standard construction exists (TLS 1.3, SPAKE2, HKDF, Argon2id), Envryn
uses it rather than assembling an equivalent from parts.

---

## 1. Primitives

| Purpose | Algorithm | Crate |
|---|---|---|
| Password KDF | Argon2id | `argon2` |
| Subkey derivation | HKDF-SHA256 | `hkdf`, `sha2` |
| Record and key-wrap AEAD | XChaCha20-Poly1305 | `chacha20poly1305` |
| Duplicate fingerprints | HMAC-SHA256 (truncated to 128 bits) | `hmac`, `sha2` |
| Device identity | Ed25519 | `ed25519-dalek` |
| Pairing key agreement (QR path) | X25519 ECDH | `x25519-dalek` |
| Pairing key agreement (manual code path) | SPAKE2 | `spake2` |
| Transport | TLS 1.3, mutual auth | `rustls` |
| Randomness | OS CSPRNG | `getrandom` / `rand_core::OsRng` |
| Database at rest | SQLCipher (AES-256-CBC + HMAC-SHA512) | `rusqlite` with SQLCipher |

**No other cryptographic implementation is permitted in the codebase.** `cargo-deny` blocks
alternative crypto crates from entering the dependency tree; adding one requires amending
`DEPENDENCY_POLICY.md`.

### Why XChaCha20-Poly1305 rather than AES-GCM

Because of sync. Two paired devices generate nonces independently with no shared counter.
The AES-GCM 96-bit nonce has a birthday bound that makes random generation uncomfortable at
scale; the XChaCha20-Poly1305 192-bit nonce does not. Random nonces are therefore safe without
any cross-device coordination, which removes an entire class of catastrophic failure.

Envryn also runs on hardware without AES-NI (low-end Android). ChaCha20 is fast in software
everywhere; AES in software is both slower and timing-sensitive.

---

## 2. Key hierarchy

```
Master Password
   |
   |  Argon2id(password, salt, params)          salt: 16 random bytes, per vault
   |  params stored in vault_meta so they can be raised later
   |  Windows default: m = 64 MiB, t = 3, p = 4
   |  Android default: m = 32 MiB, t = 3, p = 4
   |  calibrated at vault creation targeting 500-800 ms on the device
   v
  KEK  (32 bytes)  --- never persisted; zeroized immediately after use
   |
   |  XChaCha20-Poly1305 wrap
   v
  VMK  (32 bytes, from OS CSPRNG)  --- persisted ONLY wrapped
   |
   +- HKDF-SHA256(VMK, info="envryn/v1/record")      -> Record Key
   +- HKDF-SHA256(VMK, info="envryn/v1/fingerprint") -> Fingerprint Key
   +- HKDF-SHA256(VMK, info="envryn/v1/sqlcipher")   -> Database page key
   +- HKDF-SHA256(VMK, info="envryn/v1/backup")      -> Backup subkey
```

HKDF is used with an empty salt and a distinct `info` string per subkey. The `info` strings
are versioned (`v1`) so a future key-schedule change is unambiguous rather than silent.

### Why there is a VMK at all

Two properties depend on it, and neither is achievable if records are encrypted directly
under a password-derived key:

1. **Changing the master password rewraps 32 bytes.** It does not re-encrypt the vault.
   The operation is instant regardless of vault size, and cannot half-fail partway through,
   leaving some records readable and others not.

2. **Paired devices may each have a different master password.** Pairing transfers the VMK,
   not the password. This is what makes "my phone uses a PIN and my desktop uses a long
   passphrase" possible.

### Wrapping slots

`vault_meta` holds one row per unwrap path:

| Slot | Wrapping key | Present when |
|---|---|---|
| `password` | Argon2id(master password) | always |
| `platform` | DPAPI / Windows Hello / Android Keystore | user enabled platform auth |

Both slots wrap the **same** VMK. Adding or removing a slot never re-encrypts records, and
never invalidates the other slot — hence INV-007: losing your fingerprint sensor does not
lose your vault. **Implemented and tested** (`crates/envryn-core/src/vault.rs`,
`enable_platform_protection` / `disable_platform_protection` / `unlock_with_platform`); the
Android row below is not.

**Platform key details**

- *Windows, implemented*: DPAPI (`CryptProtectData`/`CryptUnprotectData`, user scope,
  `CRYPTPROTECT_UI_FORBIDDEN`) protects a freshly generated 32-byte random key — never the VMK
  directly. That recovered key is then used as an ordinary wrapping key through the same AEAD
  path every other slot uses (`crypto::keys::VaultMasterKey::wrap`/`unwrap_from`), so DPAPI's
  only job is protecting 32 bytes of random material, and a bug in the platform layer cannot
  produce a different code path for unwrapping the VMK itself.

  DPAPI alone is not a user-presence check — it protects against another Windows user account
  on the machine, not against malware running as this one. The UI accordingly says "Unlock
  with this Windows account," never "Windows Hello": true Windows Hello (`KeyCredentialManager`,
  a biometric-gated NCrypt/CNG key) is a different, larger API surface and remains unimplemented.
  See `docs/ARCHITECTURE.md` section 7.

- *Android, not implemented*: planned as an AES key in the Android Keystore created with
  `setUserAuthenticationRequired(true)`, and `setIsStrongBoxBacked(true)` where StrongBox
  is available. Key invalidation on biometric enrolment change would be **expected behaviour**
  under that design: the platform slot dies, the password slot survives.

---

## 3. Record encryption

**The whole record is sealed, not just the value.** Name, project, environment,
tags, notes and payload all live inside one AEAD blob. The database row holds only an opaque
UUID, the keyed fingerprint, and sync bookkeeping (see section 3.1).

```
nonce      = 24 random bytes (OS CSPRNG, fresh per write)
aad        = "envryn/v1/record/" || record_id || "/" || record_version
ciphertext = XChaCha20Poly1305(Record Key).encrypt(nonce, plaintext, aad)
```

**The AAD is load-bearing.** Without it, an attacker with database write access could move
a ciphertext from one row to another — swapping your staging database password into the row
labelled "production" — and it would decrypt cleanly into the wrong context. With the record
id bound in, that attack produces an authentication failure instead.

`record_version` in the AAD additionally prevents rollback of a single record to an earlier
ciphertext without detection.

### 3.1 Why metadata is not a plaintext column

The common design for an encrypted store is plaintext metadata columns inside a whole-file
encrypted database, so that SQL can filter on them. Envryn does not do this.

Metadata is not incidental — project and environment names map out someone's infrastructure,
and the *set* of provider names in a vault is itself sensitive. Relying on whole-file
encryption to protect it means the protection ends the moment the file is copied off a running
system, which is precisely the threat (V-01) the design exists to address.

The usual justification for plaintext columns is query performance, and it does not apply here:
search runs against an in-memory index while unlocked (section 4), so no query ever needed
those columns. Sealing everything costs nothing we were using.

What remains in SQL, and why each is unavoidable:

| Column | Why it cannot be sealed |
|---|---|
| `id` | Opaque UUIDv7. Needed to address a row. |
| `fingerprint` | Keyed HMAC; reveals nothing without the VMK (section 5). Needed to find duplicates without decrypting every row. |
| `created_ms` / `updated_ms` | Needed to order and reconcile during sync without unlocking. |
| `hlc_counter` / `hlc_device` | Conflict resolution. |
| `deleted` | Tombstone flag. |

The accepted residual leak is therefore **timing and volume**: an attacker holding the file
learns how many records exist and when they changed. That is recorded in `THREAT_MODEL.md`
rather than claimed as solved.

SQLCipher remains planned as defence in depth — it would conceal the record count and the
schema itself — but it is no longer load-bearing for confidentiality.

---

## 4. Search

When the vault is unlocked, records are decrypted into an in-memory index and searched there.
There is no blind index and no encrypted full-text index.

**Rationale.** A personal vault holds hundreds to low thousands of records. While unlocked,
plaintext is in process memory regardless, so a blind index would protect nothing that is not
already exposed — while adding a large attack surface and a family of subtle leaks
(token frequency analysis over a persisted index). The simpler design is also the safer one here.

On lock, the in-memory index is zeroized with everything else. Revisit only if a real vault
exceeds roughly 50,000 records.

---

## 5. Duplicate fingerprints

```
fingerprint = HMAC-SHA256(Fingerprint Key, normalize(value))[0..16]
```

`normalize` trims surrounding whitespace and nothing else. It deliberately does **not**
lowercase or strip punctuation: secrets are case- and byte-sensitive, and two values differing
only in case are genuinely different secrets.

**Why keyed.** An unkeyed hash of a secret value is an offline guessing oracle. Many real
credentials are low-entropy (`admin`, `changeme`, a short database password), and an attacker
holding the database file could confirm a guess instantly. Keying under a VMK-derived subkey
means an attacker without the master password cannot compute candidate fingerprints at all.

Exact-duplicate detection is deterministic Rust and **never** involves the AI. The AI's only
role in duplicate detection is *semantic* similarity over metadata (spec section 21).

---

## 6. Device identity and transport

**Implemented and tested** (`crates/envryn-core/src/sync/identity.rs`,
`crates/envryn-core/src/sync/transport.rs`).

**Identity.** One Ed25519 keypair per installation, generated at first run and stored in its
own small file independent of any vault (so resetting a vault does not change how paired
peers recognise this device). The private key is sealed with `platform::dpapi_protect` and
never leaves it in plaintext. A self-signed X.509 certificate carries the public key, built
fresh from the identity on demand rather than cached, so the identity file remains the single
source of truth.

The device fingerprint is `SHA-256(raw 32-byte Ed25519 public key)`, rendered as
colon-separated uppercase hex. This is **deliberately not** `SHA-256(SubjectPublicKeyInfo DER)`
as earlier drafts of this document assumed: for Ed25519, RFC 8410 defines the SPKI's BIT
STRING content as exactly the raw 32-byte key with no further ASN.1 structure around it, so
hashing the raw key produces the identical value with strictly less code and no DER-encoding
decision (canonical form, etc.) to get subtly wrong. `sync::transport`'s verifier extracts the
same 32 bytes from a peer's presented certificate via `x509-parser` before hashing, so both
sides always agree regardless of which framing either side thinks about it in.

**Transport.** TLS 1.3 with mutual authentication via `rustls`, using a custom
`ServerCertVerifier` and `ClientCertVerifier` that accept **only** fingerprints present in
the `trusted_devices` table. No web PKI, no certificate authorities, no name validation —
the pinned fingerprint set *is* the trust store, held live behind an `Arc<RwLock<..>>`
(`TrustedFingerprints`) so a revocation is visible to the *next* handshake without rebuilding
any TLS config. Tested end-to-end over real loopback TCP: successful mutual handshake, a
client whose fingerprint is not trusted, and a fingerprint revoked between two handshake
attempts (`sync::transport::tests::*`).

Revocation is a row delete. Because the verifier consults the live trusted set during the
handshake, a revoked device fails to establish a connection at all (INV-104). There is no
application-layer authorisation check that a future refactor could accidentally skip.

---

## 7. Pairing

**Implemented and tested** (`crates/envryn-core/src/sync/pairing.rs`,
`crates/envryn-core/src/sync/handshake.rs`). Both paths establish a shared secret, derive a
6-digit SAS, require the human to confirm the SAS matches on both screens, and only then
transfer the VMK.

### QR path (Windows to Android)

The QR code carries: device id, certificate fingerprint, and a LAN address to connect to.
**This is a deliberate simplification from an earlier draft of this document**, which
described the QR as also carrying a 256-bit ephemeral pairing secret or public key directly.
In the implementation, the X25519 ephemeral public keys that actually key the ECDH exchange
travel over the established connection instead — exactly like a SPAKE2 message does on the
manual path (see `sync::handshake`, which is why one shared function drives both paths'
network exchange). This costs nothing: an active man-in-the-middle substituting a key in
transit is still caught by the SAS comparison either way, and the QR's real job — an
authenticated, out-of-band channel for the address and claimed identity — is unaffected.

### Manual code path (Windows to Windows)

There is no camera, so the user types a short code. **A short code cannot safely key a plain
ECDH**: an attacker who intercepts the exchange could brute-force a 6- or 8-character code
offline at leisure. This path therefore uses **SPAKE2** in its symmetric mode (`spake2` crate,
`Spake2::start_symmetric`) — a password-authenticated key exchange designed for exactly this
situation, and symmetric because neither device is fixed as "initiator." An attacker gets one
online guess per attempt and learns nothing from a failed one.

Using ECDH on both paths would be simpler, and would be a real vulnerability on the second one.

### SAS derivation

```
SAS = HKDF-SHA256(shared_secret, info = "envryn/v1/sas" || transcript)  -> 6 decimal digits
```

The transcript covers both device ids and both certificate fingerprints, sorted so it is
identical regardless of which side is "device A," so a man-in-the-middle who substituted
either identity produces a different SAS and the user sees a mismatch. Verified for both
paths, including an explicit MITM-produces-different-SAS test
(`sync::pairing::tests::qr_pairing_mitm_produces_a_different_sas`) and end-to-end over real
loopback TCP for both paths (`sync::handshake::tests::*`), including the VMK transfer itself.

**Sessions are single-use and bounded in time.** The connect wait (waiting for a peer to dial
in) times out after 120 seconds; once a SAS is computed, the wait for the human's confirmation
times out after a further 90 seconds. A session's confirmation channel is consumed
(`Option::take`) on first use, so replaying a confirmation is a type-level impossibility, not
just a runtime check (`src-tauri/src/sync.rs`, `PairingState`). Not exercised by an automated
test — `src-tauri` has no test suite yet — so treat this half of INV-106 as verified by review
rather than by CI.

---

## 8. Sync protocol: ordering and reconciliation

**Implemented and tested** (`crates/envryn-core/src/storage/hlc.rs`,
`crates/envryn-core/src/sync/protocol.rs`). Not itself cryptography, but it decides which
ciphertext wins when two devices disagree, which is why it lives in this document rather than
`ARCHITECTURE.md`.

Every write is stamped with a **hybrid logical clock**: `(wall_ms, counter, device_id)`, used as
the deterministic tiebreak once a conflict is already known to exist. But the actual "is this a
conflict at all" decision (as of the sync-hardening pass that closed this gap) is made by a
per-record **[`storage::VersionVector`]** — a small map of `device_id -> the newest Hlc that
device has contributed to this record` — not the scalar Hlc alone. Two peers exchange a manifest
of `(id, hlc, version_vector, deleted)` for every record, request an id whenever their own vector
does not already dominate the peer's (i.e. the peer might know something they don't — either
because they are strictly ahead, or because of a genuine fork), and apply an incoming record by
comparing vectors:

- **Peer's vector is dominated by ours:** stale, discarded (`SyncOutcome::Stale`).
- **Peer's vector dominates ours:** a clean fast-forward, applied directly (`SyncOutcome::FastForward`
  or `SyncOutcome::New`).
- **Neither dominates:** a genuine concurrent edit. The scalar Hlc picks the deterministic
  winner (this is the *only* thing the scalar clock still decides), which becomes the live row —
  but the losing side is inserted into a new `record_conflicts` table rather than discarded
  (`SyncOutcome::Conflict`). `Vault::list_conflicts`/`recover_conflict`/`discard_conflict` let the
  user review, keep as a new record, or drop it.

This closes the gap `THREAT_MODEL.md` S-09 and `SECURITY_INVARIANTS.md` INV-109 previously
tracked as **not implemented** -- both are now marked implemented, with a real end-to-end test
(`sync::protocol::tests::two_devices_editing_the_same_record_offline_produce_a_recoverable_conflict`)
that edits the same record on two real, previously-paired vaults while genuinely disconnected,
syncs them over real loopback mutual TLS, and confirms the fork is detected and the losing edit
survives. Records still travel encrypted throughout: `sync::protocol`'s wire types carry only the
opaque `sealed` blob (plus the small, non-secret vector-clock metadata), never plaintext.

Deletions are tombstones (a `deleted` flag; the sealed content is cleared but the row stays)
rather than row removal, and a delete's HLC is compared like any other write — so a concurrent
edit with an older HLC cannot resurrect a deleted record. No retention window (scheduled purge
of old tombstones) is implemented; see `THREAT_MODEL.md` S-10.

---

## 9. Backup format

**Implemented** (`crates/envryn-core/src/backup.rs`). Backups are independently encrypted — a
backup file is restorable using only the backup password, and does not depend on the source
vault's VMK, master password, or device identity in any way.

```
header (plaintext, authenticated as AAD):
    magic  "ENVRYNBK"
    format_version (u16)
    kdf params (memory_kib, iterations, parallelism)
    salt (16 bytes)
body:
    XChaCha20-Poly1305( HKDF(Argon2id(backup_password, salt), "envryn/v1/backup") )
```

The header is authenticated but not encrypted, so a future version can read the KDF parameters
of an older backup without guessing. `format_version` is checked before anything else; an
unknown version is a clean refusal, never a best-effort parse.

The backup password is **independent** of the vault master password; Envryn does not currently
offer to reuse it, since the two are asked for in different flows (vault creation vs. backup
creation) and conflating them would blur what "changing your master password" is supposed to mean.

**Restoring is data-only, by design.** The body decrypts to a plain list of full records. Restoring
always creates a *new* vault, with a master password chosen at restore time, and re-encrypts
every record under that vault's own fresh VMK — a backup file never carries any of the source
vault's key material, and restoring one can never result in two live vault files silently
sharing a VMK. This is a deliberate simplification over "restore this exact vault byte-for-byte":
multi-device continuity is what device pairing (Phase 2) is for; a backup's job is disaster
recovery of data, not vault identity. The desktop app currently supports exactly one vault, so
in practice "restore" replaces it — the existing vault file (and its WAL/SHM sidecars) is renamed
aside with a timestamp first, never deleted, so a mistaken restore stays recoverable.

Backups contain vault data only. AI preferences do not exist yet (Phase 3); model binaries are
never included regardless (spec section 19).

---

## 10. Memory hygiene

- All key material and decrypted plaintext is held in `zeroize::Zeroizing` or
  `secrecy::SecretBox`, so it is wiped on drop and cannot be printed by a derived `Debug`.
- Key pages are `VirtualLock`ed (Windows) / `mlock`ed (Android) on a best-effort basis.

**Stated limitation, not a guarantee.** Neither OS promises that locked pages are never
written to disk, and hibernation writes all of physical memory regardless. A crash dump or
a hibernation file may contain vault plaintext while the vault is unlocked. Envryn reduces
the window by locking aggressively on idle, on session lock, and on backgrounding — it does
not claim to eliminate it. An attacker with the ability to read the memory of an unlocked
vault process has already won, and no amount of zeroization changes that.

---

## 11. Versioning

Every persisted cryptographic artefact carries an explicit version: `vault_meta.crypto_version`,
`record.format_version`, the backup header, and the `v1` in every HKDF `info` string.

Envryn refuses to open an artefact whose version it does not recognise. It does not attempt a
best-effort parse of an unknown format, because the failure mode of guessing wrong about a
ciphertext layout is silent corruption.
