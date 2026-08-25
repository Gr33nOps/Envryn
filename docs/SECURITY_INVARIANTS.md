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
| **INV-101** | The device private identity key never leaves the platform keystore. | A, M |
| **INV-102** | A sync session is established only over TLS 1.3 with **mutual** authentication. | A |
| **INV-103** | A peer certificate is accepted only if its fingerprint is present in `trusted_devices`. There is no "trust on first use" during sync. | A |
| **INV-104** | Revoking a device causes the **TLS handshake itself** to fail — not an application-layer check that could be skipped. | A |
| **INV-105** | The VMK is transferred only during pairing, only after the user has confirmed a matching short authentication string (SAS) on both devices. | A, M |
| **INV-106** | Pairing sessions are single-use and expire. An expired or consumed session cannot be resumed. | A |
| **INV-107** | Discovery grants no trust. Being discoverable and being paired are independent. | A |
| **INV-108** | Record payloads remain AEAD-sealed while in transit. The sync layer never handles plaintext secret values. | T, A |
| **INV-109** | Sync never destroys data without preserving the losing side of a conflict. | A |
| **INV-110** | Deletions propagate as tombstones with a retention window, never as immediate row removal. | A |

## 3. AI invariants

These restate §61 of the product specification. The *Enforced by* column is the addition.

| ID | Invariant | Enforced by |
|---|---|---|
| **AI-INV-001** | The AI never receives vault master keys. | T, D |
| **AI-INV-002** | The AI never receives device private identity keys. | T, D |
| **AI-INV-003** | The AI never has automatic whole-vault plaintext access. | **T** — see below |
| **AI-INV-004** | The AI cannot approve trusted devices. | D |
| **AI-INV-005** | The AI cannot bypass vault authentication. | D |
| **AI-INV-006** | AI prompt history containing secrets is not persisted. | A, S |
| **AI-INV-007** | AI inference requires no external Internet service. | A |
| **AI-INV-008** | AI suggestions cannot perform destructive operations without user confirmation. | T, A |
| **AI-INV-009** | The vault remains fully functional if the AI subsystem fails, is disabled, or was never installed. | **A** — see below |

### How AI-INV-003 is made structural

`ai::gateway::SanitizedPrompt` is a newtype whose inner field is private to the
`ai::gateway` module. The `LocalAiEngine` trait accepts no other input type.
Therefore no code outside the gateway can construct a value that the engine will accept,
and "hand the model raw vault data" is a **compile error** rather than a review miss.

`AiOperation` variants reference records by `RecordId`, never by value. A caller
*cannot* pass a secret to the AI layer, because no AI-facing type has a field that holds one.

A `trybuild` compile-fail test asserts that constructing a `SanitizedPrompt` from
outside the gateway module does not compile. If someone makes the field `pub`,
that test fails.

### How AI-INV-009 is tested

The single most important AI test in the suite: the full vault integration suite runs
twice in CI — once with the AI subsystem compiled in, and once with it disabled at
the feature-flag level. Both runs must pass identically. AI is a productivity layer,
not infrastructure; if any vault test depends on it, the layering has been violated.

### Enforcing AI-INV-001/002/004/005 by dependency graph

`crates/envryn-ai-worker` does **not** depend on the vault crate. It cannot name a
`Vmk`, a `DeviceKey`, a `TrustedDevice`, or a database handle, because those types are
not in its dependency graph at all. A CI step asserts this by inspecting
`cargo metadata` — if someone adds the vault crate as a dependency of the worker,
the build fails before any human reviews it.

---

## 4. Change process

Adding, weakening, or removing an invariant requires:

1. a written justification in the pull request describing what threat the change accepts;
2. a corresponding update to `THREAT_MODEL.md`;
3. explicit sign-off — not an incidental part of a feature commit.

Adding a **new** AI capability additionally requires answering the §69 decision checklist
in the pull request description. If the answers are not good, the feature waits.
