# Envryn — Security Invariants

An invariant is a property that must hold in **every** build, on **every** platform, at **every** commit.
If a change breaks one, the change is wrong — not the invariant.

Each invariant below names how it is **enforced**. Prefer enforcement that a contributor
would have to actively fight (a type, a missing dependency, a failing test) over enforcement
that they merely have to remember (a rule in a document).

Legend for the *Enforced by* column:

| Mark | Meaning |
|---|---|
| **T** | Type system / compile error |
| **D** | Dependency graph — the code physically isn't linked in |
| **A** | Automated test in CI |
| **S** | Static analysis rule (Semgrep / clippy / cargo-deny) |
| **M** | Manual review only — *weakest; minimise these* |

---

## 1. Vault core invariants

| ID | Invariant | Enforced by |
|---|---|---|
| **INV-001** | The master password is never written to disk, never logged, and never leaves the process that received it. | T, S |
| **INV-002** | The Vault Master Key (VMK) exists on disk **only** in wrapped form. | A |
| **INV-003** | The KEK derived from the master password is never persisted in any form. | A, M |
| **INV-004** | No secret value is ever written to disk outside the encrypted database or an encrypted backup. | A, S |
| **INV-005** | Locking the vault zeroizes the VMK, all derived subkeys, and the in-memory plaintext index. | A |
| **INV-006** | A wrong master password yields an authentication failure, never a partial unlock or a distinguishable error. | A |
| **INV-007** | Platform authentication (DPAPI on Windows; Android biometric not yet implemented) is an **additional** wrapper over the same VMK, never a bypass of it. Removing the platform credential never destroys vault access. | A — `platform_protection_does_not_disturb_the_password_slot` and `disabling_platform_protection_preserves_password_unlock` in `crates/envryn-core/tests/vault_lifecycle.rs` |
| **INV-008** | Every ciphertext is authenticated. Envryn never decrypts without verifying the AEAD tag. | T |
| **INV-009** | Record ciphertext is bound to its row via AAD. A ciphertext moved between rows fails authentication. | A |
| **INV-010** | Envryn makes **no** outbound network connection except (a) explicit user-initiated model download, and (b) LAN sync with an already-paired device. | A, S |
| **INV-011** | The UI layer performs no cryptography and holds no key material. | D, M |
| **INV-012** | Duplicate fingerprints are keyed (HMAC under a VMK-derived subkey), never an unkeyed hash of a secret value. | A, M |

## 2. Sync invariants

| ID | Invariant | Enforced by |
|---|---|---|
| **INV-101** | The device private identity key never leaves the platform keystore. | T, A — the key never has an accessor that returns it; `sync::identity` seals it via `platform::dpapi_protect` and only ever hands out a `Fingerprint` or a signature. Tests: `creates_and_reloads_identically`, `corrupt_identity_file_is_rejected`. |
| **INV-102** | A sync session is established only over TLS 1.3 with **mutual** authentication. | A — `sync::transport::tests::mutual_tls_succeeds_between_devices_that_trust_each_other` (real loopback TCP + TLS) |
| **INV-103** | A peer certificate is accepted only if its fingerprint is present in `trusted_devices`. There is no "trust on first use" during sync. | A — `sync::transport::tests::handshake_fails_when_client_is_not_trusted` |
| **INV-104** | Revoking a device causes the **TLS handshake itself** to fail — not an application-layer check that could be skipped. | A — `sync::transport::tests::revoked_fingerprint_is_rejected_on_the_next_handshake` |
| **INV-105** | The VMK is transferred only during pairing, only after the user has confirmed a matching short authentication string (SAS) on both devices. | A, M — `sync::handshake::tests::vmk_transfers_over_the_paired_connection` proves the wire mechanics; that a *human* actually compared the SAS before the IPC layer's `pairing_confirm` is called is enforced by the UI flow (`apps/ui/src/routes/vault/devices.tsx`), not by an automated test — **M**, not A, for that half. |
| **INV-106** | Pairing sessions are single-use and expire. An expired or consumed session cannot be resumed. | T, M — `PairingState` in `src-tauri/src/sync.rs` holds an `Option<Sender>` that `pairing_confirm`/`pairing_cancel` `.take()`, making reuse a type-level impossibility once consumed; connect/confirm timeouts (120s / 90s) bound how long a session waits. Not covered by an automated test (no `src-tauri` test suite yet) — verify by review. |
| **INV-107** | Discovery grants no trust. Being discoverable and being paired are independent. | A, D — `sync::discovery`'s tests never touch `TrustedFingerprints`; `sync::transport`'s verifier has no code path that reads a `DiscoveredPeer` at all, so a discovery result cannot influence a handshake decision even accidentally. |
| **INV-108** | Record payloads remain AEAD-sealed while in transit. The sync layer never handles plaintext secret values. | T, A — `sync::protocol`'s `WireRecord` carries only an opaque `sealed: Vec<u8>`, no plaintext field exists to populate; `sync::protocol::tests::two_vaults_converge_over_real_tls` |
| **INV-109** | Sync never destroys data without preserving the losing side of a conflict. | **A** — `storage::upsert_from_sync` distinguishes a fast-forward from a genuine fork via a per-record `VersionVector` (not the scalar Hlc alone, which cannot tell the two apart -- see `CRYPTOGRAPHY.md` section 8). A real fork's losing side is inserted into `record_conflicts`, never overwritten in place. Tested end-to-end against two real vaults over real loopback TLS: `sync::protocol::tests::two_devices_editing_the_same_record_offline_produce_a_recoverable_conflict`, plus focused `storage::tests::a_genuine_concurrent_edit_preserves_the_losing_side` and `a_resolved_conflict_can_be_deleted`. |
| **INV-110** | Deletions propagate as tombstones, never as immediate row removal, and old tombstones are purged after a bounded retention window rather than kept forever. | A — `storage::tests::soft_delete_leaves_a_tombstone`, `soft_delete_clears_the_ciphertext`, `purge_removes_only_tombstones_past_the_cutoff`, `purging_a_tombstone_also_purges_its_preserved_conflicts`. `Store::purge_expired_tombstones` runs opportunistically on every unlock (`Vault::unlock`/`unlock_with_platform`) against `storage::TOMBSTONE_RETENTION_MS` (90 days) — a device that has not synced within that window could still resurrect a peer's already-purged deletion, which is the tradeoff any bounded retention window makes, not a defect specific to this one. |

## 3. AI invariants

These restate §61 of the product specification. The *Enforced by* column is the addition.
**Implemented** as of the AI subsystem's build (`crates/envryn-core/src/ai/`,
`crates/envryn-ai-worker/`, `src-tauri/src/ai.rs`); see `docs/AI_SECURITY.md` for the two
recorded deviations from the original design (candle instead of llama.cpp; no
grammar-constrained decode) neither of which weakens anything in this table.

| ID | Invariant | Enforced by |
|---|---|---|
| **AI-INV-001** | The AI never receives vault master keys. | T, D — see below |
| **AI-INV-002** | The AI never receives device private identity keys. | T, D — see below |
| **AI-INV-003** | The AI never has automatic whole-vault plaintext access. | **T** — see below |
| **AI-INV-004** | The AI cannot approve trusted devices. | D — see below |
| **AI-INV-005** | The AI cannot bypass vault authentication. | D — see below |
| **AI-INV-006** | AI prompt history containing secrets is not persisted. | M + T, as of M22 — true by construction (no code path in `ai/` or `envryn-ai-worker` writes a prompt or response to disk); the Semgrep rule this row originally implied now exists (`.semgrep/ai-no-content-logging.yml`, 0 findings against the real tree, verified to catch synthetic violations) and `worker_client::tests::a_sentinel_in_a_worker_error_message_produces_no_message_carrying_result` proves a worker-side error message cannot carry content into this process's own output. Neither is wired into CI (none exists — `ARCHITECTURE.md` section 9), so both are pre-release manual checks, not merge gates |
| **AI-INV-007** | AI inference requires no external Internet service. | M + T, as of M22 — true by construction: `envryn-ai-worker`'s entire dependency list (`candle-core`, `candle-transformers`, `tokenizers`, `serde`, `serde_json`, `rand`) contains no HTTP client; the only network-capable code in the AI subsystem is `ai::model_download`, a separate, explicitly-invoked module the worker never calls into. `deny.toml` now bans the unreviewed HTTP client crates workspace-wide and `.semgrep/network-egress.yml` bans calling one outside `model_download`, both passing. `tests/ai_real_model.rs::classification_still_works_with_the_workers_proxy_env_poisoned` additionally proves real inference against a real model succeeds with the worker's own proxy env vars poisoned. Still not covered by a live deny-all-egress firewall test (no CI to run one against, and configuring a real firewall rule on this development machine was judged too disruptive for the marginal proof it would add over the above) |
| **AI-INV-008** | AI suggestions cannot perform destructive operations without user confirmation. | T — no code path exists today, anywhere in `ai/` or its IPC commands, that writes to the vault. The one wired feature (classification) only ever sets a value in an in-memory form; saving remains the ordinary, human-clicked `secret_create`/`secret_update` path. True by the AI surface's shape, not by a dedicated test of a mutation that does not exist yet |
| **AI-INV-009** | The vault remains fully functional if the AI subsystem fails, is disabled, or was never installed. | **A** — see below |

### How AI-INV-003 is made structural

**Implemented.** `ai::gateway::SanitizedPrompt` is a newtype whose inner field is private to the
`ai::gateway` module. The `LocalAiEngine` trait accepts no other input type.
Therefore no code outside the gateway can construct a value that the engine will accept,
and "hand the model raw vault data" is a **compile error** rather than a review miss.

`AiOperation` variants reference records by `SecretId` where one exists, or carry the plain,
budget-bounded value the caller already had (for the two Level 2 operations that classify a
value before it has been saved as a record — see `AI_SECURITY.md` section 2 for why those two
necessarily differ from a pure `RecordId` reference). No variant, and no other AI-facing type,
has a field that could hold an arbitrary vault query or a whole record.

A `trybuild` compile-fail test (`crates/envryn-core/tests/sanitized_prompt_encapsulation.rs`)
asserts that constructing a `SanitizedPrompt` from outside the gateway module does not compile
— run and confirmed to fail with `E0423: cannot initialize a tuple struct which contains
private fields`. If someone makes the field `pub`, that fixture starts compiling and the test
fails.

### How AI-INV-009 is tested

**Implemented, differently from the original design.** The originally-planned mechanism — the
full vault integration suite run twice in CI, once with AI compiled in and once with a feature
flag compiling it out — does not exist: there is no Cargo feature gating `ai/`, and no CI to run
two configurations even if there were. What holds instead, and is real: `ai/` is purely
additive. Nothing in `crate::vault`, `crate::storage`, `crate::crypto`, or `crate::sync` imports
from `crate::ai` — every one of those modules' 131 unit and 26 integration tests already passes
with `ai/` never invoked, because none of them invoke it. Deleting `src/ai/` and its one `pub
mod ai;` line in `lib.rs` would leave a fully functional, fully tested vault; this has not been
mechanically re-verified by actually deleting the module and re-running the suite, but the
dependency direction that guarantees it is the same one `crypto`→`vault`→`sync` layering already
relies on throughout this codebase.

### Enforcing AI-INV-001/002/004/005 by dependency graph

**Implemented and checked.** `crates/envryn-ai-worker` does **not** depend on `envryn-core` at
all (not merely "not the vault module of it") — `cargo tree -p envryn-ai-worker -i envryn-core`
returns no match. It cannot name a `Vmk`, a `DeviceKey`, a `TrustedDevice`, or a database
handle, because none of those types are reachable from its dependency graph. This has been run
and confirmed manually; there is no CI step that runs it automatically (no CI pipeline exists —
`ARCHITECTURE.md` section 9), so a future change that adds `envryn-core` as a worker dependency
would not be caught until the next manual check.

---

## 4. Change process

Adding, weakening, or removing an invariant requires:

1. a written justification in the pull request describing what threat the change accepts;
2. a corresponding update to `THREAT_MODEL.md`;
3. explicit sign-off — not an incidental part of a feature commit.

Adding a **new** AI capability additionally requires answering the §69 decision checklist
in the pull request description. If the answers are not good, the feature waits.
