# Envryn - Threat Model

---

## 1. What Envryn protects

A developer's working credentials: API keys, environment variables, access tokens, database
and SSH credentials, OAuth and webhook secrets, and secure notes - on a Windows PC and an
Android phone that synchronise directly over the local network.

## 2. Assets

| Asset | Why it matters |
|---|---|
| Secret values | The product |
| Vault Master Key (VMK) | Decrypts everything |
| Master password | Unwraps the VMK |
| Device private identity key | Impersonating it grants sync access |
| Vault metadata | Names and projects leak your infrastructure even without values |
| Trusted device list | Adding a row here is equivalent to full vault access |

## 3. Trust boundaries

```
    UI (WebView)                  untrusted for security purposes
         |  Tauri IPC             <-- boundary: everything validated here
    Rust core                     trusted; holds keys
         |  mutual TLS 1.3        <-- boundary: pinned fingerprints only
    Paired device
```

The UI is *inside* the application but *outside* the security boundary. It renders what Rust
gives it and holds no key material. This matters because a WebView is a large attack surface
that we do not fully control.

---

## 4. In scope

- Theft of a powered-off or locked device.
- Another user account on a shared machine.
- An attacker on the same LAN (passive and active).
- A malicious or curious local process running as the user.
- Malicious content *inside* the vault (a note or value crafted to exploit a parser).
- A supply-chain compromise of a dependency.
- Accidental self-exposure: plaintext in logs, swap, crash dumps, clipboard history, screenshots.

## 5. Out of scope

Stated plainly, because a threat model that claims to cover everything is not credible:

- **Malware running as the user while the vault is unlocked.** It can read process memory. No
  user-space vault defeats this.
- **A compromised OS, kernel, or hypervisor.**
- **Hardware attacks**: cold boot, DMA, chip decapping.
- **A physically compromised display** (shoulder surfing, a camera behind you).
- **Coercion.** Envryn has no duress mode in v1.
- **Provider-side compromise.** Envryn never contacts providers and cannot know a key was
  revoked or leaked elsewhere. It must never claim otherwise (spec section 26).

---

## 6. Vault threats

| ID | Threat | Mitigation | Residual risk |
|---|---|---|---|
| **V-01** | Stolen device, vault locked | All records AEAD-encrypted under a VMK wrapped by Argon2id; SQLCipher at rest | A weak master password is brute-forceable offline. Argon2id raises the cost; it cannot fix a 6-character password. Envryn enforces an 8-character minimum and shows a real-time strength estimate (`apps/ui/src/lib/password-strength.ts`: character-class entropy, a common-password blocklist, and repeated/sequential-run penalties) on every screen that sets a master or backup password -- not a full grammar-based estimator like zxcvbn, and advisory only, since the 8-character floor is still enforced independently in Rust and is not raised by a weak score. Found during the 2026-08-26 security audit that this row had claimed "shows strength" since Phase 1 with no such UI actually built; closed rather than left overclaiming. |
| **V-02** | Stolen device, vault unlocked | Auto-lock on idle, on session lock, on backgrounding | Anything readable in the window before lock |
| **V-03** | Another local user account reads the DB file | SQLCipher + OS file ACLs | None significant |
| **V-04** | Offline password guessing | Argon2id calibrated to 500-800 ms, params raisable | Bounded by password entropy |
| **V-05** | Ciphertext moved between rows to swap a credential | AAD binds ciphertext to record id, version, type | None - authentication fails |
| **V-06** | Rollback of one record to an older ciphertext | `record_version` in AAD | Whole-database rollback still possible; sync HLCs surface it |
| **V-07** | Secret leaks to swap or hibernation | Best-effort page locking; aggressive lock policy | **Real and acknowledged.** Hibernation writes all memory. Documented in `CRYPTOGRAPHY.md`, not claimed solved. |
| **V-08** | Secret leaks via clipboard history | **Implemented on Windows and Android:** Windows tags native clipboard writes with `ExcludeClipboardContentFromMonitorProcessing`; Android labels them sensitive with `ClipDescription.EXTRA_IS_SENSITIVE`. Both use a timed clear (configurable, default 30s) that clears only if the clipboard still holds what Envryn put there. | A clipboard manager ignoring the OS sensitivity hint; quitting Envryn before the timer fires |
| **V-09** | Secret captured by screenshot or screen share | **Implemented on Windows and Android:** Windows applies `WDA_EXCLUDEFROMCAPTURE`; the Android activity applies `WindowManager.LayoutParams.FLAG_SECURE`. Both are enabled at app startup. | An external camera pointed at the physical screen |
| **V-10** | Secret written to a log or crash report | No plaintext logging; no automatic crash upload; sentinel grep test in CI | None known |
| **V-11** | Duplicate-detection hash used as a guessing oracle | Fingerprints are **keyed** HMAC under a VMK subkey | None - unusable without the VMK |
| **V-15** | Vault left unlocked and unattended | **Implemented:** Windows uses a system-wide idle poll (`GetLastInputInfo`, every 5s, configurable threshold) and a direct `WTS_SESSION_LOCK` hook. Android resets the same configurable timeout on app interaction and locks immediately when the document becomes hidden, covering app backgrounding and screen-off. | Windows' message hook is best-effort and falls back to the idle poll. Android's timeout observes activity inside Envryn rather than system-wide input. |
| **V-16** | Platform-protected unlock (DPAPI) misidentified as biometric-*bound* | An optional real Windows Hello gate (`platform::hello`) can require a biometric/PIN prompt before the DPAPI unwrap runs, but the unwrap itself stays DPAPI-strength -- UI copy must say the gate requires Windows Hello without claiming the vault key is cryptographically bound to the biometric | Mostly labelling discipline (as before), now paired with one real technical distinction to get right: `hello_gate_enabled` genuinely requires the OS gesture to succeed (`platform::hello_verify`, gated at `vault_unlock_with_platform`), but a compromised OS/kernel that can forge a `KeyCredentialStatus::Success` result would defeat the gate the same way it could defeat any other client-side check; see `CRYPTOGRAPHY.md` section 2 |
| **V-12** | Malicious webview content exfiltrates data | CSP restricts to `'self'` and `ipc:`; no remote origins loadable. ESLint fails the build on `fetch`/`WebSocket` in UI code | A WebView RCE would bypass this |
| **V-13** | Metadata leak from a stolen database file | The whole record is sealed, so names, projects, environments and tags are ciphertext, not columns (`CRYPTOGRAPHY.md` 3.1) | **Accepted:** record count and modification timestamps remain visible. SQLCipher would conceal these too; it is defence in depth, not yet implemented. |
| **V-14** | Runtime asset fetch leaks usage to a third party | Fonts are self-hosted; no remote origin is referenced by the bundle | None known; asserted by a build-output scan |

## 7. Sync threats

| ID | Threat | Mitigation | Residual risk |
|---|---|---|---|
| **S-01** | Passive LAN eavesdropping | TLS 1.3; payloads independently AEAD-sealed | Traffic analysis reveals that sync occurred |
| **S-02** | Active MITM during sync | Mutual TLS with pinned fingerprints | None - an unpinned certificate fails the handshake |
| **S-03** | MITM during **pairing** | QR path: high-entropy out-of-band secret. Manual path: SPAKE2. Both: user-confirmed SAS over a transcript covering both identities | A user who confirms without comparing the digits. UI makes comparison the explicit action. |
| **S-04** | Brute-forcing a short pairing code | SPAKE2 gives one online guess per attempt; sessions single-use, 120 s expiry | Negligible |
| **S-05** | Revoked device continues syncing | Verifier consults `trusted_devices` **during the handshake** | None - connection fails, not an app-layer check |
| **S-06** | Replay of captured sync traffic | TLS 1.3 anti-replay; HLC ordering rejects stale writes | None significant |
| **S-07** | Malicious peer sends malformed records | Strict schema validation; AEAD verification before any parse | Denial of service against the sync session only |
| **S-08** | Rogue device discovered on LAN and trusted | Discovery grants zero trust; pairing requires physical SAS confirmation | User pairs with an attacker's device deliberately |
| **S-09** | Concurrent edits silently destroy a credential | Per-record `VersionVector` (`storage::version_vector`) detects a genuine fork -- neither side's vector dominates the other's -- distinct from a peer simply being behind; the scalar Hlc only picks the deterministic winner once a fork is already known. The losing side is inserted into `record_conflicts`, never discarded. | **Implemented**, in the sync-hardening pass after Phase 2. `Vault::list_conflicts`/`recover_conflict`/`discard_conflict` let the user review a preserved conflict, keep it as a new record, or drop it. Proven end-to-end against two real vaults over real loopback TLS in `sync::protocol::tests::two_devices_editing_the_same_record_offline_produce_a_recoverable_conflict` -- a user who edits the same secret on two devices before they next sync no longer loses the other edit without a trace of it. |
| **S-10** | Deletion race resurrects a record | Tombstones (`deleted` flag; `soft_delete` clears the sealed content but keeps the row) rather than immediate row removal. A delete's HLC (and version vector) advance like any other write, so a concurrent edit with an older HLC cannot un-delete a record. | **Implemented, bounded.** `Store::purge_expired_tombstones` reclaims a tombstone once it is older than `storage::TOMBSTONE_RETENTION_MS` (90 days), run opportunistically on every unlock. A device offline for longer than that window could still resurrect a deletion its peers have already purged when it finally reconnects -- the inherent tradeoff of any *bounded* retention window, not a defect unique to this one; the alternative (no purge at all) traded unbounded storage growth for the same risk never manifesting, which is why 90 days was chosen generously. |

**Verification scope for sync/pairing (S-01 through S-10).** Every row above whose
mechanism lives in `envryn_core::sync` is exercised by a real test over real loopback
TCP/TLS - mutual TLS handshake success/rejection/revocation (`sync::transport`), SPAKE2
and ECDH pairing convergence and MITM-produces-different-SAS (`sync::pairing`,
`sync::handshake`), and full manifest-exchange sync convergence between two independent
`Store`s (`sync::protocol`). What is **not** exercised is the interactive, two-human,
two-physical-device flow the Tauri IPC layer (`src-tauri/src/sync.rs`) drives on top of
those primitives - there is no second physical machine in this development environment.
The background-thread pairing state machine there has been reviewed carefully but should
be treated as unverified in practice until it has run against a second real device.

**Pairing rendezvous is address-carried, not discovery-assisted.** The design in this
document describes discovery (mDNS) as separate from pairing; in the implementation, the
host side of a pairing session displays its LAN address and port directly (alongside the
code, for the manual path) rather than the joining device finding it via `sync::discovery`.
This keeps the two mechanisms decoupled - mDNS discovery is used only for already-trusted
peers finding each other for an ongoing sync session, never for establishing initial trust.

## 8. AI threats (retired in 0.2.0)

Earlier releases shipped an optional local-AI subsystem, and this section tracked eight threats
against it (AI-01 to AI-08): whole-vault leakage to the model, secrets in logs or cached model
context, prompt injection from vault content, a tampered model file, a compromised inference
runtime, over-broad data requests, and trusted hallucinated advice.

**0.2.0 removed the subsystem**: model, worker process, model download, and every AI command.
None of those threats has anything left to act on. The one AI-adjacent behaviour that remains,
suggesting a secret's type and name, is deterministic string matching in `envryn_core::classify`
with no model, no process boundary, and no network, and it answers "Unknown" instead of
guessing (so AI-08's "confident wrong answer" failure mode cannot arise from it). See
`SECURITY_INVARIANTS.md` section 3 for the matching retired invariants.

---

## 9. Supply chain

| Threat | Mitigation |
|---|---|
| Malicious crate update | Lockfile committed; dependency additions reviewed manually. As of M22, `cargo-deny` (via `deny.toml`) and `cargo-audit` are installed and run in this environment - `cargo deny check` exits 0 (bans, licenses, sources, and reviewed advisories all pass) and `cargo audit` shows 18 findings, all reviewed individually and confirmed to be "unmaintained"/"unsound" warnings (not exploitable vulnerabilities) in Tauri's own transitive tree, never in code this project chose directly (see `deny.toml`'s `[advisories].ignore` for the per-ID reasoning). There is still no CI to run either automatically - that gap remains real, just narrower: these are now real, working, documented pre-release checks rather than absent tooling. |
| Malicious npm package | Lockfile committed; UI dependencies cannot reach keys (they are in Rust) |
| Typosquatting | Additions require justification per `DEPENDENCY_POLICY.md` |

Special scrutiny applies to native-code libraries and archive/compression code (spec section
26) - these are large surfaces that process untrusted input. Removing local AI in 0.2.0 dropped
the largest such surface (the inference runtime, tokenizer, and model loader) from the build.

---

## 10. Privacy claims Envryn can honestly make

Each of these is testable, and each has a test:

- Secrets are encrypted on your devices. - *Crypto suite; disk inspection*
- Devices synchronise directly after explicit pairing. - *Two-vault integration test over real loopback TLS (`sync::protocol::two_vaults_converge_over_real_tls`); not yet verified against two physical devices - see section 7's verification-scope note*
- No cloud vault, no account, no telemetry, and no HTTP requests at all. - *No live deny-all-egress firewall test exists; structurally true via `deny.toml` (bans `ureq`/`reqwest`/`hyper`/`curl`/`sentry`* workspace-wide) and `.semgrep/network-egress.yml` (flags any HTTP client call), both passing with 0 findings - the only network-capable code path is `sync` (LAN-only, mutually authenticated)*
- Type and name suggestions never leave the device. - *They are deterministic string rules in `envryn_core::classify`, with unit tests for recognised values and for "Unknown"; there is no model and no network path to send anything to*

Envryn does **not** claim: protection against malware on an unlocked device, immunity to
hibernation-file exposure, or any knowledge of whether a stored credential is still valid.

---

## 11. Maintenance

Update this document when: a trust boundary moves, any model or network feature is proposed, a dependency
with native code is added, a sync protocol change lands, or a security test is added or removed.

Reviewed at every milestone completion, and in full at M28.
