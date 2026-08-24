# Envryn — Threat Model

---

## 1. What Envryn protects

A developer's working credentials: API keys, environment variables, access tokens, database
and SSH credentials, OAuth and webhook secrets, and secure notes — on a Windows PC and an
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
         |  loopback + token      <-- boundary: sanitised data only, one direction
    AI worker process             untrusted; no keys, no DB
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
- Malicious content *inside* the vault (a note carrying a prompt injection).
- A tampered or malicious model file.
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
| **V-01** | Stolen device, vault locked | All records AEAD-encrypted under a VMK wrapped by Argon2id; SQLCipher at rest | A weak master password is brute-forceable offline. Argon2id raises the cost; it cannot fix a 6-character password. Envryn enforces a minimum and shows strength. |
| **V-02** | Stolen device, vault unlocked | Auto-lock on idle, on session lock, on backgrounding | Anything readable in the window before lock |
| **V-03** | Another local user account reads the DB file | SQLCipher + OS file ACLs | None significant |
| **V-04** | Offline password guessing | Argon2id calibrated to 500-800 ms, params raisable | Bounded by password entropy |
| **V-05** | Ciphertext moved between rows to swap a credential | AAD binds ciphertext to record id, version, type | None — authentication fails |
| **V-06** | Rollback of one record to an older ciphertext | `record_version` in AAD | Whole-database rollback still possible; sync HLCs surface it |
| **V-07** | Secret leaks to swap or hibernation | Best-effort page locking; aggressive lock policy | **Real and acknowledged.** Hibernation writes all memory. Documented in `CRYPTOGRAPHY.md`, not claimed solved. |
| **V-08** | Secret leaks via clipboard history | Timed clear; `ExcludeClipboardContentFromMonitorProcessing` (Windows); `EXTRA_IS_SENSITIVE` (Android) | A clipboard manager ignoring the hint |
| **V-09** | Secret captured by screenshot or screen share | `WDA_EXCLUDEFROMCAPTURE` (Windows); `FLAG_SECURE` (Android) | An external camera |
| **V-10** | Secret written to a log or crash report | No plaintext logging; no automatic crash upload; sentinel grep test in CI | None known |
| **V-11** | Duplicate-detection hash used as a guessing oracle | Fingerprints are **keyed** HMAC under a VMK subkey | None — unusable without the VMK |
| **V-12** | Malicious webview content exfiltrates data | CSP restricts to `'self'` and `ipc:`; no remote origins loadable | A WebView RCE would bypass this |

## 7. Sync threats

| ID | Threat | Mitigation | Residual risk |
|---|---|---|---|
| **S-01** | Passive LAN eavesdropping | TLS 1.3; payloads independently AEAD-sealed | Traffic analysis reveals that sync occurred |
| **S-02** | Active MITM during sync | Mutual TLS with pinned fingerprints | None — an unpinned certificate fails the handshake |
| **S-03** | MITM during **pairing** | QR path: high-entropy out-of-band secret. Manual path: SPAKE2. Both: user-confirmed SAS over a transcript covering both identities | A user who confirms without comparing the digits. UI makes comparison the explicit action. |
| **S-04** | Brute-forcing a short pairing code | SPAKE2 gives one online guess per attempt; sessions single-use, 120 s expiry | Negligible |
| **S-05** | Revoked device continues syncing | Verifier consults `trusted_devices` **during the handshake** | None — connection fails, not an app-layer check |
| **S-06** | Replay of captured sync traffic | TLS 1.3 anti-replay; HLC ordering rejects stale writes | None significant |
| **S-07** | Malicious peer sends malformed records | Strict schema validation; AEAD verification before any parse | Denial of service against the sync session only |
| **S-08** | Rogue device discovered on LAN and trusted | Discovery grants zero trust; pairing requires physical SAS confirmation | User pairs with an attacker's device deliberately |
| **S-09** | Concurrent edits silently destroy a credential | LWW by HLC, **losing side preserved** and surfaced | User must review conflicts |
| **S-10** | Deletion race resurrects a record | Tombstones with retention window | Bounded by the window |

## 8. AI threats

These are specification section 60, with enforcement made explicit.

| ID | Threat | Mitigation | Enforced by |
|---|---|---|---|
| **AI-01** | A bug sends the whole decrypted vault to the model | Central gateway; `SanitizedPrompt` constructible only inside `ai::gateway`; operations reference records by id, never by value | **Compile error** + `trybuild` test |
| **AI-02** | A secret appears in a log | No prompt logging; `SanitizedPrompt` implements neither `Display` nor `Debug`; Semgrep rule | Compile error + static analysis + sentinel grep test |
| **AI-03** | A secret persists in cached model context | Sessions are temporary; **vault lock kills the worker process** rather than clearing context | Test: lock during inference, assert process death |
| **AI-04** | A stored note manipulates the assistant | Model has no tools; output schema cannot express a privileged action; all mutations confirmed | Schema + absence of capability |
| **AI-05** | A tampered model file is loaded | Pinned checksum, size, source, version verified before load | Test with a corrupted file |
| **AI-06** | The AI runtime is compromised | Separate process; no DB path, no keys; vault crate absent from its dependency graph | **`cargo metadata` CI check** |
| **AI-07** | The AI requests more data than needed | The application decides, not the model; per-operation level policy; budgets in the gateway | Gateway tests incl. refusals |
| **AI-08** | Hallucinated security advice is trusted | Output labelled as suggestion; hedged language ("looks like"); security decisions stay deterministic | Review + copy standards |

**On AI-08.** Envryn does not contact providers, so it cannot know whether a credential is
currently valid or compromised. It must say "consider reviewing this credential — last rotated
14 months ago," never "this key is compromised" (spec section 26). Overstating confidence in a
security tool is itself a security problem: it trains users to act on guesses.

---

## 9. Supply chain

| Threat | Mitigation |
|---|---|
| Malicious crate update | Lockfile committed; `cargo-deny` and `cargo-audit` in CI; dependency additions reviewed |
| Malicious npm package | Lockfile committed; UI dependencies cannot reach keys (they are in Rust) |
| Compromised inference runtime | Pinned version, checksum-verified, isolated process |
| Typosquatting | Additions require justification per `DEPENDENCY_POLICY.md` |

Special scrutiny applies to the inference runtime, tokenizer, model loader, native and GPU
libraries, the download mechanism, and archive/compression code (spec section 26) — these are
large native-code surfaces that process untrusted input.

---

## 10. Privacy claims Envryn can honestly make

Each of these is testable, and each has a test:

- Secrets are encrypted on your devices. — *Crypto suite; disk inspection*
- Devices synchronise directly after explicit pairing. — *Two-device integration test*
- No cloud vault, no account, no telemetry. — *Egress test; dependency audit*
- AI processing happens locally. — *Offline AI test*
- The AI has restricted vault access and is not part of encryption or authentication. — *Gateway tests; AI-disabled run*

Envryn does **not** claim: protection against malware on an unlocked device, immunity to
hibernation-file exposure, or any knowledge of whether a stored credential is still valid.

---

## 11. Maintenance

Update this document when: a trust boundary moves, a new AI feature is registered, a dependency
with native code is added, a sync protocol change lands, or a security test is added or removed.

Reviewed at every milestone completion, and in full at M28.
