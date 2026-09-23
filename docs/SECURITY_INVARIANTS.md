# Envryn - Security Invariants

An invariant is a property that must hold in **every** build, on **every** platform, at **every** commit.
If a change breaks one, the change is wrong - not the invariant.

Each invariant below names how it is **enforced**. Prefer enforcement that a contributor
would have to actively fight (a type, a missing dependency, a failing test) over enforcement
that they merely have to remember (a rule in a document).

Legend for the *Enforced by* column:

| Mark | Meaning |
|---|---|
| **T** | Type system / compile error |
| **D** | Dependency graph - the code physically isn't linked in |
| **A** | Automated test in CI |
| **S** | Static analysis rule (Semgrep / clippy / cargo-deny) |
| **M** | Manual review only - *weakest; minimise these* |

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
| **INV-007** | Platform authentication (DPAPI on Windows; Android biometric not yet implemented) is an **additional** wrapper over the same VMK, never a bypass of it. Removing the platform credential never destroys vault access. | A - `platform_protection_does_not_disturb_the_password_slot` and `disabling_platform_protection_preserves_password_unlock` in `crates/envryn-core/tests/vault_lifecycle.rs` |
| **INV-008** | Every ciphertext is authenticated. Envryn never decrypts without verifying the AEAD tag. | T |
| **INV-009** | Record ciphertext is bound to its row via AAD. A ciphertext moved between rows fails authentication. | A |
| **INV-010** | Envryn makes **no** outbound network connection except LAN sync with an already-paired device. There is no HTTP client in the build at all (`cargo deny` bans `ureq`, `reqwest`, `hyper`, and `curl`). | A, S |
| **INV-011** | The UI layer performs no cryptography and holds no key material. | D, M |
| **INV-012** | Duplicate fingerprints are keyed (HMAC under a VMK-derived subkey), never an unkeyed hash of a secret value. | A, M |

## 2. Sync invariants

| ID | Invariant | Enforced by |
|---|---|---|
| **INV-101** | The device private identity key never leaves the platform keystore. | T, A - the key never has an accessor that returns it; `sync::identity` seals it via `platform::dpapi_protect` and only ever hands out a `Fingerprint` or a signature. Tests: `creates_and_reloads_identically`, `corrupt_identity_file_is_rejected`. |
| **INV-102** | A sync session is established only over TLS 1.3 with **mutual** authentication. | A - `sync::transport::tests::mutual_tls_succeeds_between_devices_that_trust_each_other` (real loopback TCP + TLS) |
| **INV-103** | A peer certificate is accepted only if its fingerprint is present in `trusted_devices`. There is no "trust on first use" during sync. | A - `sync::transport::tests::handshake_fails_when_client_is_not_trusted` |
| **INV-104** | Revoking a device causes the **TLS handshake itself** to fail - not an application-layer check that could be skipped. | A - `sync::transport::tests::revoked_fingerprint_is_rejected_on_the_next_handshake` |
| **INV-105** | The VMK is transferred only during pairing, only after the user has confirmed a matching short authentication string (SAS) on both devices. | A, M - `sync::handshake::tests::vmk_transfers_over_the_paired_connection` proves the wire mechanics; that a *human* actually compared the SAS before the IPC layer's `pairing_confirm` is called is enforced by the UI flow (`apps/ui/src/routes/vault/devices.tsx`), not by an automated test - **M**, not A, for that half. |
| **INV-106** | Pairing sessions are single-use and expire. An expired or consumed session cannot be resumed. | T, M - `PairingState` in `src-tauri/src/sync.rs` holds an `Option<Sender>` that `pairing_confirm`/`pairing_cancel` `.take()`, making reuse a type-level impossibility once consumed; connect/confirm timeouts (120s / 90s) bound how long a session waits. Not covered by an automated test (no `src-tauri` test suite yet) - verify by review. |
| **INV-107** | Discovery grants no trust. Being discoverable and being paired are independent. | A, D - `sync::discovery`'s tests never touch `TrustedFingerprints`; `sync::transport`'s verifier has no code path that reads a `DiscoveredPeer` at all, so a discovery result cannot influence a handshake decision even accidentally. |
| **INV-108** | Record payloads remain AEAD-sealed while in transit. The sync layer never handles plaintext secret values. | T, A - `sync::protocol`'s `WireRecord` carries only an opaque `sealed: Vec<u8>`, no plaintext field exists to populate; `sync::protocol::tests::two_vaults_converge_over_real_tls` |
| **INV-109** | Sync never destroys data without preserving the losing side of a conflict. | **A** - `storage::upsert_from_sync` distinguishes a fast-forward from a genuine fork via a per-record `VersionVector` (not the scalar Hlc alone, which cannot tell the two apart -- see `CRYPTOGRAPHY.md` section 8). A real fork's losing side is inserted into `record_conflicts`, never overwritten in place. Tested end-to-end against two real vaults over real loopback TLS: `sync::protocol::tests::two_devices_editing_the_same_record_offline_produce_a_recoverable_conflict`, plus focused `storage::tests::a_genuine_concurrent_edit_preserves_the_losing_side` and `a_resolved_conflict_can_be_deleted`. |
| **INV-110** | Deletions propagate as tombstones, never as immediate row removal, and old tombstones are purged after a bounded retention window rather than kept forever. | A - `storage::tests::soft_delete_leaves_a_tombstone`, `soft_delete_clears_the_ciphertext`, `purge_removes_only_tombstones_past_the_cutoff`, `purging_a_tombstone_also_purges_its_preserved_conflicts`. `Store::purge_expired_tombstones` runs opportunistically on every unlock (`Vault::unlock`/`unlock_with_platform`) against `storage::TOMBSTONE_RETENTION_MS` (90 days) - a device that has not synced within that window could still resurrect a peer's already-purged deletion, which is the tradeoff any bounded retention window makes, not a defect specific to this one. |

## 3. Retired: AI invariants (AI-INV-001 to AI-INV-009)

Releases before 0.2.0 shipped an optional local-AI subsystem, and this section held nine
invariants constraining it: that the model never received keys or whole-vault plaintext, could
not approve devices, bypass authentication, or act without confirmation, needed no Internet
service, persisted no prompt history, and that the vault kept working without it.

**0.2.0 removed the subsystem entirely**: the model, its worker process, the model download
(and with it the build's only HTTP client), and every AI command. The invariants were retired
with it, per the change process below: the threat they constrained, a model component with
access to vault-adjacent data, no longer exists anywhere in the build. Type and name
suggestions are now deterministic string rules in `envryn_core::classify`, covered by the vault
core invariants above like any other core code.

Reintroducing any model, local or remote, must restore these invariants (and their tests)
before it ships.

---

## 4. Change process

Adding, weakening, or removing an invariant requires:

1. a written justification in the pull request describing what threat the change accepts;
2. a corresponding update to `THREAT_MODEL.md`;
3. explicit sign-off - not an incidental part of a feature commit.

