# Envryn — Security Architecture

An audit-scoped summary of Envryn's security architecture, as independently re-verified during
the 2026-08-26 audit (`AUDIT_REPORT.md`). This is deliberately a synthesis, not a replacement:
`docs/ARCHITECTURE.md` (general shape), `docs/CRYPTOGRAPHY.md` (normative crypto spec),
`docs/THREAT_MODEL.md` (assets, boundaries, threat tables), and `docs/SECURITY_INVARIANTS.md`
(enforcement mechanism per invariant) remain the detailed, normative sources — this document
exists so a reviewer gets the security-relevant shape in one page, with citations into those four
where the detail lives, rather than four separate ~300-400 line documents to reconcile.

---

## 1. Trust boundaries

```
    UI (WebView, React)              UNTRUSTED for security purposes
         |
         |  Tauri IPC -- only "core:default" capability granted;
         |  no fs/shell/http/clipboard plugin reaches the WebView
         v
    Rust core (src-tauri + envryn-core)   TRUSTED -- holds keys, does crypto
         |
         |  loopback TCP + per-session 192-bit bearer token,
         |  regenerated every worker launch
         v
    AI worker process (envryn-ai-worker)   UNTRUSTED -- no keys, no DB,
         |                                 no dependency on envryn-core
         |
         |  mutual TLS 1.3, pinned device-certificate fingerprints,
         |  no CA, no name validation -- the pinned set IS the trust store
         v
    Paired peer device
```

**Why the UI is untrusted despite being "inside" the app.** A Tauri WebView is a large attack
surface Envryn does not fully control. The UI is treated as a hostile input source: it can request
operations but cannot construct key material, cannot name a filesystem path outside two
deliberate, documented exceptions (backup export/import), and cannot reach any Tauri plugin that
would let it touch the filesystem, shell, network, or clipboard directly. This audit independently
confirmed the capability grant (`src-tauri/capabilities/default.json`: `core:default` only) and
walked every `#[tauri::command]` in `ipc.rs` to confirm no path- or SQL-shaped parameter escapes
that model.

**Why the AI worker is a separate, less-trusted process.** It is explicitly in-scope in the threat
model as reachable by "a malicious or curious local process running as the user" — so its trust
boundary is enforced the same way a network service would be: loopback-only bind, a random
per-session token nobody outside this process's own stdout stream can read, and structural
isolation (`envryn-ai-worker` does not depend on `envryn-core` at the `Cargo.toml` level, so it
cannot name a `Vmk` or a database handle even if compromised).

## 2. Key hierarchy (normative: `CRYPTOGRAPHY.md` §2)

```
Master Password --Argon2id--> KEK (never persisted) --wrap--> VMK (persisted only wrapped)
                                                                  |
                          +---------------------+---------------+----------------+
                          |                      |                                |
                   HKDF "record"          HKDF "fingerprint"                HKDF "sqlcipher" / "backup"
                   Record Key             Fingerprint Key (keyed HMAC)      DB page key / backup subkey
```

Two independent wrapping slots (`password`, `platform`) protect the *same* VMK — removing one
never invalidates the other and never re-encrypts a single record. An optional Windows Hello gate
sits in front of the platform slot's unwrap as an authentication check, not a third key-wrapping
path; it does not change what protects the VMK bytes themselves. This audit re-verified this
distinction is honestly represented in the UI-facing code (`vault_unlock_with_platform` genuinely
calls `platform::hello_verify()` before, not instead of, the unchanged DPAPI unwrap) rather than
just trusting the doc's claim.

**Password policy, as of this audit.** An 8-character minimum is enforced both client-side and
independently in the relevant Tauri commands. As of this audit, every password-creation surface
(vault creation, pairing-join, backup creation, backup restore) also shows a real-time, local
strength estimate (`apps/ui/src/lib/password-strength.ts`) — advisory only; it does not change the
enforced minimum. This closes a gap where `THREAT_MODEL.md` V-01 had claimed this existed since
Phase 1 without the UI actually being built. See `AUDIT_REPORT.md` §4.

## 3. Record encryption model (normative: `CRYPTOGRAPHY.md` §3)

The **entire record** — name, project, environment, tags, notes, payload — is one AEAD blob
(XChaCha20-Poly1305), not just the "secret value" field. The database schema holds only what
cannot be sealed without losing sync/ordering ability: an opaque UUID, a keyed-HMAC fingerprint
for duplicate detection, and HLC/version-vector bookkeeping. There is deliberately no plaintext
`name` or `project` column — the common "plaintext metadata, encrypted value" design was
considered and rejected because project/environment names themselves map infrastructure. AAD
binds every ciphertext to its record id and version, so moving a ciphertext between rows or
rolling one back to an earlier version fails authentication rather than decrypting into the wrong
context.

## 4. IPC surface shape

Every Tauri command falls into one of three shapes, confirmed by reading `src-tauri/src/ipc.rs`
in full:

1. **No path parameter at all** (the overwhelming majority) — operates on `State<VaultState>` or
   takes only opaque ids/values the caller already had.
2. **Path derived from the OS, never from the caller** — `vault_path()`, `identity_path()`: always
   `AppHandle::path().app_data_dir()` joined with a hardcoded literal filename.
3. **Path supplied by the caller, deliberately** — `backup_create`/`backup_restore` only. This is
   the one place the architecture's "no command takes a path outside the vault directory" rule
   has an exception, and it is the correct one: a backup is a user-initiated export/import to a
   location the user picks via a native save/open dialog, functionally no different from "Save
   As" in any other application. It is independently encrypted regardless of where it lands, so a
   wrong or hostile destination path is a usability problem, not a confidentiality one.

A Semgrep taint rule (borrowed from a web-framework ruleset, not written for this shape of app)
flagged shape 2 as a potential path-traversal during this audit; tracing every call site by hand
confirmed no caller-influenced component exists — documented as a false positive in
`AUDIT_REPORT.md` §3 rather than suppressed silently.

## 5. Sync trust model (normative: `THREAT_MODEL.md` §7, `CRYPTOGRAPHY.md` §§6-8)

No web PKI. The pinned fingerprint set in `trusted_devices` **is** the trust store — a custom
`rustls` `ServerCertVerifier`/`ClientCertVerifier` accepts only certificates whose SHA-256 hash of
the raw Ed25519 public key is already in that table, checked live (behind an `Arc<RwLock<..>>`)
on every handshake, so revocation takes effect on the *next* connection attempt with no separate
application-layer check to skip. Pairing (the only path that adds a device to that trust store)
requires a human to compare a 6-digit SAS derived from a transcript covering both device
identities — an active MITM substituting either identity produces a different SAS.

Conflict handling is version-vector-based, not scalar-clock-only: a genuine concurrent edit is
detected (neither side's vector dominates the other's) and the losing side is preserved in a
`record_conflicts` table rather than silently overwritten — closing what `THREAT_MODEL.md` used to
track as open item S-09.

## 6. What is, and is not, cryptographically bound

Worth stating plainly because it is easy to misread from the feature list alone:

- The Windows Hello gate is a **presence check in front of** the DPAPI unwrap, not a
  cryptographic binding of the VMK to the biometric. `KeyCredentialManager` only exposes signing,
  and ECDSA signatures are not deterministic, so there is no stable unwrap key derivable from one.
- DPAPI protects a random 32-byte platform key, never the VMK directly, so a bug in the platform
  layer cannot produce a different unwrap code path for the VMK itself.
- The AI worker's per-session token is 192 bits from a CSPRNG, checked on every request — not a
  one-time handshake token trusted for the connection's lifetime.
- The 8-character master-password minimum plus a strength estimate (§2) is **not** a guarantee of
  a strong password — it is a floor plus a nudge, and this document does not claim otherwise.

## 7. Platform coverage

| Concern | Windows | Android |
|---|---|---|
| Key storage | DPAPI, implemented | Keystore + StrongBox, planned, not built |
| Screen capture exclusion | `WDA_EXCLUDEFROMCAPTURE`, implemented | `FLAG_SECURE`, planned, not built |
| Clipboard exclusion | Implemented (native tag + timed clear) | `EXTRA_IS_SENSITIVE`, planned, not built |
| Auto-lock trigger | Idle poll + `WTS_SESSION_LOCK` hook, both implemented | Lifecycle trigger, planned, not built |
| Local AI | Implemented (candle sidecar, job-object isolation) | Not in v1 |

Android's gaps are scoped-out, documented absences, not defects discovered by this audit — see
`ARCHITECTURE.md` §7 for the full table this one summarizes.

## 8. Out of scope (stated in `THREAT_MODEL.md` §5, reaffirmed here)

Malware running as the user while the vault is unlocked; a compromised OS/kernel/hypervisor;
hardware attacks (cold boot, DMA); a physically compromised display; coercion; provider-side
compromise. A security architecture document that implies coverage of these would not be
credible, so this one does not.

---

For enforcement-mechanism detail (which invariant is enforced by the type system vs. a test vs.
manual review only), see `docs/SECURITY_INVARIANTS.md`. For the full per-area verification this
audit performed, see `AUDIT_REPORT.md` and `SECURITY_CHECKLIST.md`.
