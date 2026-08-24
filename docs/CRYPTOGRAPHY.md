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
lose your vault.

**Platform key details**

- *Windows*: DPAPI (`CryptProtectData`, user scope, `CRYPTPROTECT_LOCAL_MACHINE` **off**)
  for the at-rest case; Windows Hello via `KeyCredentialManager` where a user gesture is
  required. DPAPI alone is not a user-presence check — it protects against another user
  account on the machine, not against malware running as this user. Documented, not overstated.
- *Android*: an AES key in the Android Keystore created with
  `setUserAuthenticationRequired(true)`, and `setIsStrongBoxBacked(true)` where StrongBox
  is available. Key invalidation on biometric enrolment change is **expected behaviour**:
  the platform slot dies, the password slot survives.

---

## 3. Record encryption

Every secret payload is sealed independently:

```
nonce      = 24 random bytes (OS CSPRNG, fresh per write)
aad        = record_id || record_version || record_type
ciphertext = XChaCha20Poly1305(Record Key).encrypt(nonce, plaintext, aad)
```

**The AAD is load-bearing.** Without it, an attacker with database write access could move
a ciphertext from one row to another — swapping your staging database password into the row
labelled "production" — and it would decrypt cleanly into the wrong context. With the record
id bound in, that attack produces an authentication failure instead.

`record_version` in the AAD additionally prevents rollback of a single record to an earlier
ciphertext without detection.

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

**Identity.** One Ed25519 keypair per installation, generated at first run. The private key
is sealed by the platform keystore and never leaves it. A self-signed X.509 certificate
carries the public key. The device fingerprint shown in the UI is
`SHA-256(SubjectPublicKeyInfo)`, rendered as colon-separated uppercase hex.

**Transport.** TLS 1.3 with mutual authentication via `rustls`, using a custom
`ServerCertVerifier` and `ClientCertVerifier` that accept **only** fingerprints present in
the `trusted_devices` table. No web PKI, no certificate authorities, no name validation —
the pinned fingerprint set *is* the trust store.

Revocation is a row delete. Because the verifier consults that table during the handshake,
a revoked device fails to establish a connection at all (INV-104). There is no application-layer
authorisation check that a future refactor could accidentally skip.

---

## 7. Pairing

Both paths establish a shared secret, derive a 6-digit SAS, require the human to confirm the
SAS matches on both screens, and only then transfer the VMK.

### QR path (Windows to Android)

The QR code carries: device id, certificate fingerprint, LAN address hints, and a 256-bit
ephemeral pairing secret. Because the channel carrying the secret is a camera — out of band
and high entropy — X25519 ECDH with fingerprint pinning is sufficient.

### Manual code path (Windows to Windows)

There is no camera, so the user types a short code. **A short code cannot safely key a plain
ECDH**: an attacker who intercepts the exchange could brute-force a 6- or 8-character code
offline at leisure. This path therefore uses **SPAKE2**, a password-authenticated key exchange
designed for exactly this situation. An attacker gets one online guess per attempt and learns
nothing from a failed one.

Using ECDH on both paths would be simpler, and would be a real vulnerability on the second one.

### SAS derivation

```
SAS = HKDF-SHA256(shared_secret, info = "envryn/v1/sas" || transcript)  -> 6 decimal digits
```

The transcript covers both device ids and both certificate fingerprints, so a man-in-the-middle
who substituted either identity produces a different SAS and the user sees a mismatch.

**Sessions are single-use and expire after 120 seconds** (INV-106).

---

## 8. Backup format

Backups are independently encrypted — a backup file must be restorable on a device that has
never been paired, using only the backup password.

```
header (plaintext, authenticated as AAD):
    magic  "ENVRYNBK"
    format_version (u16)
    kdf params (algorithm, m, t, p, salt)
body:
    XChaCha20-Poly1305( HKDF(Argon2id(backup_password, salt), "envryn/v1/backup") )
```

The header is authenticated but not encrypted, so a future version can read the KDF parameters
of an older backup without guessing. `format_version` is checked before anything else; an
unknown version is a clean refusal, never a best-effort parse.

The backup password is **independent** of the vault master password by default. Reusing the
vault password is offered but not assumed.

Backups contain vault data and AI *preferences*. They never contain model binaries (spec section 19).

---

## 9. Memory hygiene

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

## 10. Versioning

Every persisted cryptographic artefact carries an explicit version: `vault_meta.crypto_version`,
`record.format_version`, the backup header, and the `v1` in every HKDF `info` string.

Envryn refuses to open an artefact whose version it does not recognise. It does not attempt a
best-effort parse of an unknown format, because the failure mode of guessing wrong about a
ciphertext layout is silent corruption.
