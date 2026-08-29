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

## 8. AI threats

These are specification section 60. Enforcement below reflects what is actually built
(`crates/envryn-core/src/ai/`, `crates/envryn-ai-worker/`, `src-tauri/src/ai.rs`), not the
original aspiration - see `AI_SECURITY.md` for the recorded deviation (candle instead of
llama.cpp) and where grammar-constrained decode now stands (real for `ClassificationOutput`,
not yet the other schemas) that this table's citations already account for.

| ID | Threat | Mitigation | Enforced by |
|---|---|---|---|
| **AI-01** | A bug sends the whole decrypted vault to the model | Central gateway; `SanitizedPrompt` constructible only inside `ai::gateway`; operations reference records by id or carry a plain, budget-bounded value the caller already had - never a vault handle | **Compile error** + `trybuild` test (`tests/sanitized_prompt_encapsulation.rs`) |
| **AI-02** | A secret appears in a log | `SanitizedPrompt` implements neither `Display` nor `Debug` (compiles-away the obvious mistake); no code path in this codebase today logs a prompt or model output; the worker's own error messages are discarded rather than printed (`worker_client.rs`'s `WireResponse::Error { message: _ }`), so a future library bug echoing input into an error string cannot leak through this client either | Compile error, plus `.semgrep/ai-no-content-logging.yml` (M22, run manually - `semgrep --config .semgrep/`, 0 findings against the real tree) and `worker_client::tests::a_sentinel_in_a_worker_error_message_produces_no_message_carrying_result`. Still **not wired into CI** - no CI pipeline exists (`ARCHITECTURE.md` section 9) - so this is "run it yourself before every release," not "cannot merge a violation." |
| **AI-03** | A secret persists in cached model context | Sessions are temporary; **vault lock kills the worker process** rather than clearing context, called synchronously from `ipc::vault_lock` and the idle auto-lock tick. As of M22, a second, independent kill path exists on Windows: the worker is assigned to a `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` job object (`platform::windows_impl::KillOnCloseJob`) whose handle lives for the `WorkerClient`'s own lifetime, so the OS itself terminates the worker if this process exits abnormally (crash, force-kill) without ever running the `Drop`-based kill path at all - closing the one real gap the original Drop-only strategy had. Not implemented for non-Windows targets (Android has no worker sidecar yet regardless - spec section 52). | `worker_client::tests::shutdown_actually_terminates_the_child_process` (idle-time kill) and, new in M22, `killing_the_worker_mid_inference_fails_the_request_cleanly` (kills the worker while a request is genuinely in flight, using the fake-worker fixture's `FAKE_WORKER_DELAY_MS`, and asserts the caller sees a clean `Timeout`/`Unavailable` error rather than a hang). The job-object kill path itself is proven separately and platform-generically with a real spawned `ping.exe` process in `platform::windows_impl::tests::dropping_the_job_kills_the_assigned_process`. |
| **AI-04** | A stored note manipulates the assistant | Model has no tools; output schema cannot express a privileged action; all mutations confirmed | Schema + absence of capability. `#[serde(deny_unknown_fields)]` proven against a real model's actual output, including a spurious extra field, in `tests/ai_real_model.rs` |
| **AI-05** | A tampered model file is loaded | Pinned checksum and size verified before load; source is pinned by construction (no public function accepts a caller-supplied URL) | Tests with a size-mismatched and a checksum-mismatched file (`model_download::tests::verify_file_rejects_*`). "Version" is a label on the pinned `ModelSpec`, not independently verified - redundant with the checksum, since a matching checksum already implies the exact expected bytes |
| **AI-06** | The AI runtime is compromised | Separate process; no DB path, no keys; `envryn-core` (not just "the vault module of it") absent from the worker's dependency graph entirely; as of M22, `deny.toml` (`cargo-deny`) structurally bans `reqwest`/`hyper`/`curl` and the non-`rustls` TLS stacks anywhere in the workspace, and `.semgrep/network-egress.yml` bans calling any HTTP client outside `ai::model_download` | `cargo tree -p envryn-ai-worker -i envryn-core` returns no match; `cargo deny check` exits 0 (advisories/bans/licenses/sources all pass); `semgrep --config .semgrep/network-egress.yml` finds 0 violations. **Run manually**, not a CI check - no CI pipeline exists yet (`ARCHITECTURE.md` section 9) - so these are pre-release checks, not merge gates. |
| **AI-07** | The AI requests more data than needed | The application decides, not the model; per-operation level policy; budgets in the gateway | Gateway tests incl. refusals (`gateway::tests::a_value_over_budget_never_reaches_the_engine`, `env_names_over_the_count_budget_are_refused`) |
| **AI-08** | Hallucinated security advice is trusted | Security decisions stay entirely deterministic (classification/naming are the only wired-up features, and neither makes a claim about a credential's validity); the one wired suggestion surface uses hedged language ("Looks like a Stripe credential") | Copy review only - no broader "every AI-sourced string is hedged" system exists; this is a one-string implementation, not a pattern enforced anywhere |

**On AI-08.** Envryn does not contact providers, so it cannot know whether a credential is
currently valid or compromised. It must say "consider reviewing this credential - last rotated
14 months ago," never "this key is compromised" (spec section 26). Overstating confidence in a
security tool is itself a security problem: it trains users to act on guesses. No feature that
makes a validity claim has been built yet, so this risk has not materialised in what exists
today - it is a rule for whatever is built next, not a currently-tested guarantee.

**Verification scope for AI (AI-01 through AI-08).** `envryn-core`'s AI tests are real: a real
spawned worker process, a real loopback socket, real length-prefixed JSON framing
(`worker_client`'s tests), and - separately, not run by default - real candle inference against
a real downloaded model proving genuinely correct classification and naming results
(`tests/ai_real_model.rs`; see that file's doc comment for why it is `#[ignore]`d and how to run
it). As of M22, that same real-model suite includes
`classification_still_works_with_the_workers_proxy_env_poisoned`, which points the worker child
process's own `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY` at a closed local port and confirms real
inference still succeeds unaffected - the closest thing this environment can safely do to "cut
the network and confirm AI still works" without touching the real OS firewall on a machine used
for other things.

As of the WebDriver work described in `ARCHITECTURE.md` section 9, "this environment cannot
drive a native GUI window" is no longer true: `.dev-tools/webdriver-smoke.mjs` genuinely launches
the release binary, types into real form fields, clicks through real Radix/shadcn buttons via the
W3C Actions API, creates a real vault, navigates to Settings, and screenshots the real "Local AI"
section (enable toggle, model status, download button) rendering correctly - not a browser-only
preview. That script is a manual smoke test, not an automated CI suite, and does not yet drive
the AI enable → download → start → use flow specifically (it stops at confirming Settings
renders) - the interactive Settings-to-inference flow remains unexercised end-to-end, but for a
smaller reason now (nobody has extended the script that far yet) rather than the tooling gap this
row used to describe. Also not exercised: a real deny-all-egress firewall rule (see
`AI_SECURITY.md` section 10 for why, and what stands in for it instead).

---

## 9. Supply chain

| Threat | Mitigation |
|---|---|
| Malicious crate update | Lockfile committed; dependency additions reviewed manually. As of M22, `cargo-deny` (via `deny.toml`) and `cargo-audit` are installed and run in this environment - `cargo deny check` exits 0 (bans, licenses, sources, and reviewed advisories all pass) and `cargo audit` shows 18 findings, all reviewed individually and confirmed to be "unmaintained"/"unsound" warnings (not exploitable vulnerabilities) in Tauri's own transitive tree, never in code this project chose directly (see `deny.toml`'s `[advisories].ignore` for the per-ID reasoning). There is still no CI to run either automatically - that gap remains real, just narrower: these are now real, working, documented pre-release checks rather than absent tooling. |
| Malicious npm package | Lockfile committed; UI dependencies cannot reach keys (they are in Rust) |
| Compromised inference runtime | `candle`/`candle-transformers`/`tokenizers` are widely-used, actively-maintained crates reviewed at the version pinned in `Cargo.lock`, same as every other dependency; the model file itself is checksum-verified and the worker process is isolated (see AI-06) |
| Typosquatting | Additions require justification per `DEPENDENCY_POLICY.md` |

Special scrutiny applies to the inference runtime, tokenizer, model loader, native and GPU
libraries, the download mechanism, and archive/compression code (spec section 26) - these are
large native-code surfaces that process untrusted input.

---

## 10. Privacy claims Envryn can honestly make

Each of these is testable, and each has a test:

- Secrets are encrypted on your devices. - *Crypto suite; disk inspection*
- Devices synchronise directly after explicit pairing. - *Two-vault integration test over real loopback TLS (`sync::protocol::two_vaults_converge_over_real_tls`); not yet verified against two physical devices - see section 7's verification-scope note*
- No cloud vault, no account, no telemetry. - *No live deny-all-egress firewall test exists (no CI to run one against); structurally true as of M22 via `deny.toml` (bans `reqwest`/`hyper`/`curl`/`sentry`* workspace-wide) and `.semgrep/network-egress.yml` (bans calling any HTTP client outside `ai::model_download`), both passing with 0 findings - the only network-capable code paths are `sync` (LAN-only, mutually authenticated) and `ai::model_download` (one pinned HTTPS source, invoked only from an explicit Settings button)*
- AI processing happens locally. - *`tests/ai_real_model.rs` runs real inference against a real local model with no network call in the inference path itself; as of M22, `classification_still_works_with_the_workers_proxy_env_poisoned` additionally proves this under a poisoned proxy environment for the worker process. Still not backed by a live "assert zero packets left the machine" firewall test - see `AI_SECURITY.md` section 10*
- The AI has restricted vault access and is not part of encryption or authentication. - *Gateway tests (`ai::gateway::tests::*`); "AI-disabled run" is not a separate CI configuration (none exists) but is true by construction - nothing in `vault`, `storage`, `crypto`, or `sync` imports from `ai`*

Envryn does **not** claim: protection against malware on an unlocked device, immunity to
hibernation-file exposure, or any knowledge of whether a stored credential is still valid.

---

## 11. Maintenance

Update this document when: a trust boundary moves, a new AI feature is registered, a dependency
with native code is added, a sync protocol change lands, or a security test is added or removed.

Reviewed at every milestone completion, and in full at M28.
