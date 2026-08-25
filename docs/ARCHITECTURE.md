# Envryn — Architecture

---

## 1. Shape

Envryn is a **Tauri v2** application: a React UI in a system WebView, with all security-relevant
logic in a Rust core, plus a separate AI worker process. One codebase targets Windows and Android.

```
+---------------------------------------------------------------+
|  apps/ui           React 19 + Tailwind v4 + Radix             |
|                    TanStack Router (SPA, no SSR)              |
|                    No crypto. No keys. Renders what Rust says.|
+-------------------------------+-------------------------------+
                                |  Tauri IPC (typed commands)
+-------------------------------v-------------------------------+
|  src-tauri         THE TRUST BOUNDARY                         |
|                                                               |
|   crypto/    KDF, AEAD, key hierarchy, zeroization            |
|   vault/     records, projects, environments, search          |
|   storage/   rusqlite + SQLCipher, migrations                 |
|   auth/      password, Windows Hello, biometric, auto-lock    |
|   platform/  DPAPI | Keystore | FLAG_SECURE | clipboard       |
|   sync/      identity, pairing, discovery, transport, protocol|
|   ai/        gateway, engine trait, prompts, schemas          |
|   ipc/       Tauri commands - the only surface the UI reaches |
+-------------------------------+-------------------------------+
                                |  loopback + per-session token
+-------------------------------v-------------------------------+
|  crates/envryn-ai-worker      SEPARATE PROCESS                |
|    has:      model path, socket, token                        |
|    does NOT: link the vault crate, hold a key, see the DB     |
+---------------------------------------------------------------+
```

`packages/contract` holds TypeScript types generated from the Rust types via `ts-rs`, so the
IPC contract has exactly one definition. Hand-maintaining the same shape twice is how the two
sides drift.

---

## 2. Why Tauri rather than Flutter

The product specification's roadmap named Flutter. This is a deliberate, recorded departure.

**For Tauri:** it preserves the existing React UI; the Rust ecosystem has the strongest available
libraries for every security-critical component here (RustCrypto, `rustls`, `rusqlite`, `spake2`,
`ed25519-dalek`); Tauri sidecars make the process isolation the specification asks for (section 45)
a first-class feature rather than a build-system project; and Tauri v2 has been independently
audited.

**Against:** Android runs in a WebView rather than natively, so screenshot protection needs a
small custom Kotlin plugin; and the Android toolchain is less mature than Flutter's.

**The deciding factor:** a Dart implementation would need FFI to Rust or C for crypto and local
inference regardless, so the Flutter route ends up maintaining two languages *and* discarding
the existing UI.

---

## 3. Layering rules

1. **The UI performs no cryptography and holds no key material.** It receives display data and
   sends intents. A compromised WebView must not be a compromised vault.
2. **The UI reaches Rust only through `ipc/`.** Every command validates its input; no command
   takes a key, a path outside the vault directory, or raw SQL.
3. **`ai/` cannot reach `storage/` directly.** It goes through `vault/` like everything else,
   and only via the gateway.
4. **`sync/` never handles plaintext.** It moves sealed payloads.
5. **`platform/` isolates every OS-specific call**, so the rest of the core is portable and
   testable without a device.

Dependencies point inward. `crypto/` knows nothing about records; `vault/` knows nothing about
Tauri; `ipc/` knows about everything and is deliberately thin.

---

## 4. Data model

```
projects        id, name, created_hlc, updated_hlc, deleted
environments    id, project_id, name
secrets         id, project_id, environment_id, name, type,
                nonce, ciphertext, fingerprint,
                created_hlc, updated_hlc, deleted, record_version
tags            secret_id, tag
trusted_devices device_id, name, cert_fingerprint, paired_at, last_sync
vault_meta      crypto_version, kdf_params, wrapped_vmk_password,
                wrapped_vmk_platform, device_id
```

Rows are opaque. `secrets.sealed` holds the entire record — name, project, environment, tags,
notes and payload — as one AEAD blob. The remaining columns exist only so sync can order and
reconcile records without decrypting them, and so duplicates can be found by keyed fingerprint.

There is deliberately **no plaintext `name` or `project` column**; `CRYPTOGRAPHY.md` section 3.1
explains why, and a schema test fails if one is added. The accepted residual leak is record
count and modification timing, recorded in `THREAT_MODEL.md` as V-13.

**The payload is a typed union, not a string.** The current UI models `value` as a flat string
([envryn-data.ts](../src/lib/envryn-data.ts)), but SSH and database credentials are inherently
multi-field:

```rust
enum SecretPayload {
    ApiKey    { value: String, provider: Option<String> },
    Token     { value: String, expires_at: Option<DateTime> },
    EnvVar    { key: String, value: String },
    Database  { host: String, port: u16, database: String,
                username: String, password: String },
    Ssh       { private_key: String, passphrase: Option<String>,
                host: Option<String>, username: Option<String> },
    OAuth     { client_id: String, client_secret: String },
    Webhook   { endpoint: String, secret: String },
    Note      { body: String },
    Custom    { fields: Vec<(String, String)> },
}
```

The UI already anticipates this in its `typeFields` map; that map becomes **generated** from
this enum rather than hand-maintained, so a new field cannot be added in Rust and forgotten in
the form.

### Hybrid logical clocks

Every mutable row carries `(wall_ms, counter, device_id)`. Sync resolves last-writer-wins by HLC
with device id as a deterministic tiebreak. Wall-clock alone is unusable — phone and desktop
clocks disagree, and a clock that jumps backwards would silently lose edits.

Deletions are tombstones with a retention window. Immediate row removal would let a deletion
racing a sync resurrect the record.

---

## 5. Vault lifecycle

```
  Uninitialised --create--> Locked <--lock-- Unlocked
                              |                 ^
                              +----unlock-------+

Unlock (password): derive KEK -> unwrap VMK (password slot) -> derive subkeys
                    -> build in-memory index
Unlock (platform):  DPAPI-recover platform key -> unwrap VMK (platform slot)
                    -> derive subkeys -> build in-memory index
Lock:               zeroize VMK, subkeys, index -> checkpoint WAL -> kill AI worker
Triggers:           idle timeout (implemented) | Windows session lock (not yet --
                    see section 7) | Android background (not yet) | Ctrl+L | crash
```

Lock is idempotent and must never fail. A lock path that can error is a lock path that can
leave the vault open. Idle-timeout auto-lock is a background poll in the Tauri shell
(`src-tauri/src/autolock.rs`), not a listener on the vault itself -- see section 7 for why
polling was chosen over the `WTS_SESSION_LOCK` window message.

---

## 6. AI subsystem

Detailed in `AI_SECURITY.md`. Structurally:

```
User intent
   -> ai::gateway          resolves ids, applies level policy, redacts, budgets
   -> SanitizedPrompt      constructible only inside the gateway module
   -> LocalAiEngine        trait; llama.cpp sidecar is one implementation
   -> grammar-constrained decode
   -> strict deserialisation
   -> UI shows a suggestion
   -> user confirms
   -> vault applies the change
```

The `LocalAiEngine` trait exists so the model and runtime can change without touching feature
code (spec section 7). Raw model calls appear in exactly one place.

**Deterministic before probabilistic.** Classification runs a rules engine over known credential
prefixes and shapes first; the model is the fallback for unrecognised values. `.env` parsing uses
a real parser. Exact duplicate detection uses keyed HMAC. Most of what looks like an AI feature
is ordinary code, which is faster, more private, and works with no model installed.

---

## 7. Platform specifics

| Concern | Windows | Android |
|---|---|---|
| Key storage | **Implemented:** DPAPI (`CryptProtectData`/`CryptUnprotectData`), `platform::windows_impl` | Not yet. Keystore + `setUserAuthenticationRequired` + StrongBox where present, planned |
| Screen capture | **Implemented:** `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`, applied unconditionally at startup | Not yet. `FLAG_SECURE` via a custom Kotlin plugin, planned |
| Clipboard | **Implemented:** native write + `ExcludeClipboardContentFromMonitorProcessing` tag + Rust-side timed clear, configurable in Settings | Not yet. `ClipDescription.EXTRA_IS_SENSITIVE`, planned |
| Lock trigger | **Implemented:** system-wide idle poll (`GetLastInputInfo`, every 5s). **Not implemented:** `WTS_SESSION_LOCK` -- see below | Not yet. Lifecycle background trigger, planned |
| Local AI | Bundled llama.cpp sidecar -- Phase 3, not started | **Not in v1** (spec section 52) |
| Min version | Windows 10 1809+ | API 26+ (StrongBox 28+) |

**Why idle-poll rather than `WTS_SESSION_LOCK` for now.** A native session-lock hook needs a
window-message subscription (`WTSRegisterSessionNotification` plus handling `WM_WTSSESSION_CHANGE`
in the window procedure) -- a real native hook, not a library call. The idle poll
(`src-tauri/src/autolock.rs`) covers the common case -- the user walked away -- at a fraction of
the implementation cost, and works identically regardless of whether the OS session itself locks.
Reacting to the session-lock event directly remains open for M22 hardening.

**What "Unlock with this Windows account" actually is.** It is DPAPI (`CryptProtectData`), tied to
the current Windows user account -- not `KeyCredentialManager`, and not a biometric gesture. The
UI is worded to match: "Unlock with this Windows account," never "Windows Hello." DPAPI may
itself be backed by a TPM or a Hello-protected profile depending on the machine's configuration,
but Envryn does not invoke that layer directly, so it does not claim to. See
`docs/CRYPTOGRAPHY.md` section 2 for how the platform slot's key hierarchy stays independent of
this distinction (DPAPI protects a random platform key, never the VMK directly).

Android receives AI-generated metadata through sync once confirmed on Windows (spec section 53),
so shipping without on-device inference costs organisation, not correctness.

---

## 8. Repository layout

```
apps/ui/              React SPA
src-tauri/            Tauri shell: window creation, IPC (ipc.rs), non-secret
                       app settings (settings.rs), idle auto-lock (autolock.rs),
                       capture protection (capture_protection.rs)
crates/
  envryn-core/        crypto, model, storage, vault, backup, platform --
                       no Tauri dependency
  envryn-ai-worker/   sidecar; not yet started (Phase 3). Will not depend on
                       envryn-core, per AI-INV-001/002/004/005
packages/contract/    generated TS types -- not yet started; the IPC contract
                       is hand-maintained today in apps/ui/src/lib/ipc.ts
docs/                 this directory
```

`envryn-core` is free of Tauri so the security-critical code can be tested as a plain library,
without a windowing system -- including `platform::windows_impl`, whose tests exercise real
DPAPI and the real OS clipboard, not mocks. That is also what makes the "AI disabled" CI run
cheap, once Phase 3 makes that distinction meaningful.

`envryn-core::platform` is the one place in the vault core permitted to contain `unsafe` (the
crate-level lint is `deny`, not `forbid`, specifically so this one module can carry a scoped
`#[allow(unsafe_code)]`) -- every other module, including every cryptographic one, remains
unsafe-free.

---

## 9. Build and release

CI on every commit: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`,
`cargo test --no-default-features` (the AI-disabled run), `cargo deny check`, `cargo audit`,
Semgrep, `eslint`, `tsc --noEmit`.

Release additionally requires: signed Windows and Android binaries, no development AI endpoints,
no remote inference configuration, no debug unlock path, no test secrets in the bundle, and a
passing egress test.
