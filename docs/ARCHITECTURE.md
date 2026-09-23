# Envryn - Architecture

---

## 1. Shape

Envryn is a **Tauri v2** application: a React UI in a system WebView, with all security-relevant
logic in a Rust core. One codebase targets Windows and Android.

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
|   classify/  rule-based type and name suggestions, search     |
|   ipc/       Tauri commands - the only surface the UI reaches |
+---------------------------------------------------------------+
```

`packages/contract` holds TypeScript types generated from the Rust types via `ts-rs`, so the
IPC contract has exactly one definition. Hand-maintaining the same shape twice is how the two
sides drift.

**`sync/` is implemented** (identity, pairing, discovery, mutual-TLS transport, and the
manifest-exchange protocol) - see `CRYPTOGRAPHY.md` sections 6-8 for the cryptographic detail
and `THREAT_MODEL.md` section 7 for what has and has not been verified.

**Local AI was removed in 0.2.0.** Earlier releases shipped an optional on-device model in a
separate worker process. It was slow to start, needed a ~1 GB download, and only ever helped
where the rule engine (section 6) already answered better. Removing it took a separate process,
an HTTP client, and the whole ML dependency tree out of the attack surface.

---

## 2. Why Tauri rather than Flutter

The product specification's roadmap named Flutter. This is a deliberate, recorded departure.

**For Tauri:** it preserves the existing React UI; the Rust ecosystem has the strongest available
libraries for every security-critical component here (RustCrypto, `rustls`, `rusqlite`, `spake2`,
`ed25519-dalek`); Tauri sidecars make process isolation (section 45) a first-class feature
should a future component need it; and Tauri v2 has been independently audited.

**Against:** Android runs in a WebView rather than natively, so screenshot protection needs a
small custom Kotlin plugin; and the Android toolchain is less mature than Flutter's.

**The deciding factor:** a Dart implementation would need FFI to Rust or C for crypto
regardless, so the Flutter route ends up maintaining two languages *and* discarding the
existing UI.

---

## 3. Layering rules

1. **The UI performs no cryptography and holds no key material.** It receives display data and
   sends intents. A compromised WebView must not be a compromised vault.
2. **The UI reaches Rust only through `ipc/`.** Every command validates its input; no command
   takes a key, a path outside the vault directory, or raw SQL.
3. **`classify/` and `search/` are pure functions.** They see a value or a query string, never
   storage, keys, or the network.
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
trusted_devices device_id, fingerprint, sealed, paired_ms
vault_meta      crypto_version, kdf_params, wrapped_vmk_password,
                wrapped_vmk_platform, device_id
```

Rows are opaque. `secrets.sealed` holds the entire record - name, project, environment, tags,
notes and payload - as one AEAD blob. The remaining columns exist only so sync can order and
reconcile records without decrypting them, and so duplicates can be found by keyed fingerprint.

`trusted_devices` follows the same pattern: the device's display name and pairing history live
inside `sealed`, under the Record Key, not in a plaintext column. The one exception is
`fingerprint` itself - deliberately plaintext, because it is not a secret (the same role an SSH
host key fingerprint plays: it is read aloud and compared on screen during pairing) and
`sync::transport`'s TLS verifier needs the whole trusted set in memory to check every incoming
handshake, which is cheaper to build from a plain column than by unsealing every row up front
for a value that was never secret in the first place.

There is deliberately **no plaintext `name` or `project` column**; `CRYPTOGRAPHY.md` section 3.1
explains why, and a schema test fails if one is added. The accepted residual leak is record
count and modification timing, recorded in `THREAT_MODEL.md` as V-13.

**The payload is a typed union, not a string.** The current UI models `value` as a flat string
([envryn-data.ts](../apps/ui/src/lib/envryn-data.ts)), but SSH and database credentials are inherently
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

**Implemented and tested** - `storage::Hlc` (`crates/envryn-core/src/storage/hlc.rs`). Every
mutable row carries `(wall_ms, counter, device_id)`. Wall-clock alone is unusable - phone and
desktop clocks disagree, and a clock that jumps backwards would silently lose edits; `Hlc::tick`
guarantees monotonicity even when the wall clock itself moves backwards. Conflict *detection*
(as opposed to the deterministic-winner tiebreak) is decided by a per-record `VersionVector`
(`storage::version_vector`), not the scalar Hlc alone - see `CRYPTOGRAPHY.md` section 8 for why a
scalar comparison cannot tell "the peer was simply behind" apart from "a genuine fork," and for
how `record_conflicts` now preserves the losing side instead of discarding it (`THREAT_MODEL.md`
S-09, `SECURITY_INVARIANTS.md` INV-109 - both now implemented).

Deletions are tombstones (a `deleted` flag; content cleared, row kept), not immediate row
removal - a deletion racing a sync cannot be resurrected by a concurrent edit, since the
delete's HLC (and its version vector) advance like any other write. Tombstones are purged once
past a 90-day retention window (`storage::TOMBSTONE_RETENTION_MS`), opportunistically on unlock
rather than a background timer.

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
Lock:               zeroize VMK, subkeys, index -> checkpoint WAL
Triggers:           idle timeout (implemented) | Windows session lock (implemented --
                    see section 7) | Android background (not yet) | Ctrl+L | crash
```

Lock is idempotent and must never fail. A lock path that can error is a lock path that can
leave the vault open. Idle-timeout auto-lock is a background poll in the Tauri shell
(`src-tauri/src/autolock.rs`); the Windows session-lock hook (same file) is a second,
independent trigger for the identical lock sequence -- see section 7 for both.

---

## 6. Rule-based suggestions

**Implemented** (`crates/envryn-core/src/classify.rs`, `crates/envryn-core/src/search.rs`,
`src-tauri/src/detect.rs`). Everything that suggests or interprets is ordinary, deterministic
Rust: instant, offline, and it never sends a value anywhere.

```
Suggest type   value -> known prefix / connection-string scheme / shape (PEM, JWT)
                     -> else the variable name's convention (IGDB_CLIENT_SECRET -> OAuth)
                     -> else None, shown as "Unknown"
Suggest name   NAME=value line -> NAME
                     -> prefix-specific name (pk_live_ -> STRIPE_PUBLISHABLE_KEY)
                     -> provider's documented name (ghp_ -> GITHUB_TOKEN, postgres:// -> DATABASE_URL)
                     -> else None, shown as "Unknown"
Search         plain name matching first; if that finds nothing, parse the query into
               environment / kind / free-text filters ("production database")
```

Prefix rules use **longest match**, so a specific rule always beats a general one sharing its
start (`sk-or-v1-` beats `sk-`) regardless of table order. A `None` is a real answer: the UI
says "Unknown" and leaves the field alone rather than presenting a guess with the same
authority as a match. `.env` parsing uses a real parser, and exact duplicate detection uses
keyed HMAC (`CRYPTOGRAPHY.md` section 5).

Earlier releases (before 0.2.0) layered an optional local model on top of these rules for
values they did not recognise. In practice the rules answered the cases that mattered, the
model's answers for the rest were often wrong with confidence, and it cost a ~1 GB download,
a separate worker process, and an HTTP client. It was removed.

---

## 7. Platform specifics

| Concern | Windows | Android |
|---|---|---|
| Key storage | **Implemented:** DPAPI (`CryptProtectData`/`CryptUnprotectData`), `platform::windows_impl` | Not yet. Keystore + `setUserAuthenticationRequired` + StrongBox where present, planned |
| Screen capture | **Implemented:** `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`, applied unconditionally at startup | Not yet. `FLAG_SECURE` via a custom Kotlin plugin, planned |
| Clipboard | **Implemented:** native write + `ExcludeClipboardContentFromMonitorProcessing` tag + Rust-side timed clear, configurable in Settings | Not yet. `ClipDescription.EXTRA_IS_SENSITIVE`, planned |
| Lock trigger | **Implemented:** system-wide idle poll (`GetLastInputInfo`, every 5s) plus a direct `WTS_SESSION_LOCK` hook (window-procedure subclass via `WTSRegisterSessionNotification` -- see below), both converging on the same lock sequence | Not yet. Lifecycle background trigger, planned |
| Min version | Windows 10 1809+ | API 26+ (StrongBox 28+) |

**The direct `WTS_SESSION_LOCK` hook, implemented.** `platform::windows_impl::watch_session_lock`
subclasses the Tauri main window's procedure (`SetWindowLongPtrW(GWLP_WNDPROC, ...)`, forwarding
every other message to the window's real procedure via `CallWindowProcW`) and calls
`WTSRegisterSessionNotification` so Windows delivers `WM_WTSSESSION_CHANGE` to it. On
`WTS_SESSION_LOCK` specifically, it fires the same `autolock::lock_now` the idle poll uses --
`Win+L`, the screen saver locking, or a remote session disconnecting now locks the vault
immediately rather than waiting for the next idle-poll tick to notice. Proven against a real
native window (the built-in "STATIC" class, never shown) and a real `SendMessageW` delivery in
`platform::windows_impl::tests::subclassed_window_reports_a_session_lock_and_forwards_everything_else`,
which also confirms an unrelated message still reaches the window's original procedure unchanged.
Installed alongside the idle poll (`src-tauri/src/lib.rs`'s `.setup()`), not instead of it: a
window-message hook can still fail to register (no main window yet, a non-Windows target, the OS
call itself failing), in which case the idle poll remains the only trigger -- the same coverage
this app shipped with before. This closes the item that stayed open through the Phase 4
(M22-M28) hardening pass, which had focused on the (since removed) local-AI attack surface, the
network-privacy proof, and supply-chain policy enforcement instead (see `DEPENDENCY_POLICY.md`
section 6) rather than platform-trigger coverage.

**What "Unlock with this Windows account" actually is, and what the optional Hello gate adds.**
The unlock itself is still DPAPI (`CryptProtectData`), tied to the current Windows user account --
DPAPI may itself be backed by a TPM or a Hello-protected profile depending on the machine's
configuration, but Envryn does not invoke that layer directly for the unwrap itself. What changed:
`platform::hello` now uses `KeyCredentialManager` for real, but as an authentication *gate* placed
in front of that same DPAPI unwrap, not a replacement for it -- `KeyCredentialManager` only exposes
signing, and standard ECDSA signatures are not deterministic, so there is no way to derive a stable
unwrap key from a signature the way DPAPI's recovered bytes work. When a vault has the Hello gate
enabled, `vault_unlock_with_platform` calls `platform::hello_verify` (a real biometric/PIN prompt)
first, and only proceeds to the unchanged DPAPI unwrap if that succeeds. The UI must still not
claim the vault key is bound to the biometric itself -- it is not; only the *gate* is real Windows
Hello. See `docs/CRYPTOGRAPHY.md` section 2 for how the platform slot's key hierarchy stays
independent of this distinction (DPAPI protects a random platform key, never the VMK directly),
and `platform::hello`'s own module doc for the full reasoning.

---

## 8. Repository layout

```
apps/ui/              React SPA
src-tauri/            Tauri shell: window creation, vault IPC (ipc.rs), sync/
                       pairing/discovery/trusted-device IPC (sync.rs), non-secret
                       app settings (settings.rs), idle auto-lock (autolock.rs),
                       capture protection (capture_protection.rs)
crates/
  envryn-core/        crypto, model, storage, vault, backup, platform, sync,
                       classify (rule-based suggestions), search -- no Tauri dependency
  envryn-android-clipboard/
                       Android sensitive-clipboard Tauri plugin
packages/contract/    generated TS types (ts-rs, `cargo test --workspace
                       export_bindings`) -- packages/contract/bindings/*.ts are
                       generated and committed; index.ts is the one
                       hand-maintained barrel re-exporting them. CI fails if
                       `cargo test` regenerates a different bindings/ than
                       what's committed. apps/ui/src/lib/ipc.ts imports these
                       rather than declaring the shapes itself; only its
                       command wrappers, IpcError, and isTauri() remain
                       hand-written, since those are UI-side glue, not wire
                       shapes
docs/                 this directory
```

`envryn-core` is free of Tauri so the security-critical code can be tested as a plain library,
without a windowing system -- including `platform::windows_impl`, whose tests exercise real
DPAPI and the real OS clipboard, not mocks.

`envryn-core::platform` is the one place in the vault core permitted to contain `unsafe` (the
crate-level lint is `deny`, not `forbid`, specifically so this one module can carry a scoped
`#[allow(unsafe_code)]`) -- every other module, including every cryptographic one, remains
unsafe-free.

---

## 9. Build and release

**A real CI pipeline now exists**: `.github/workflows/ci.yml`, GitHub Actions. It mirrors the
exact manual commands this repo's commit history already verified by hand for every phase rather
than inventing a separate CI-only check -- `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`
(`EmbarkStudios/cargo-deny-action`), and `cargo audit` on `windows-latest` (deliberately not
`ubuntu-latest`: most of the sync/hardening work lives behind `#[cfg(windows)]`, and building
on Linux would silently compile the `stub` fallbacks and test none of it); Semgrep and the
frontend job (`eslint`, `tsc --noEmit`, `vite build`) run on `ubuntu-latest` instead, since
neither is platform-specific and both are faster there. A live deny-all-egress firewall test
remains not run in CI -- a real firewall rule was judged too disruptive to configure against a
real development machine. What stands in for it: the build contains no HTTP client at all
(`cargo deny` bans `ureq`/`reqwest`/`hyper`/`curl`, and `.semgrep/network-egress.yml` flags any
HTTP call in source), so LAN sync is the only code path that opens a socket. The workflow triggers on push/PR to `main`; it has
not yet been extended to gate merges (no branch protection rule requiring it to pass configured).

**Native GUI verification, real as of this pass.** `.dev-tools/webdriver-smoke.mjs` drives the
actual compiled Tauri window through the W3C WebDriver protocol via `tauri-driver` (wrapping
`msedgedriver.exe`) -- not a browser-only preview of the Vite dev server, which is what every
earlier phase's docs correctly said was the limit of this environment. It launches the real
release binary, types into real form fields, submits via the W3C Actions API (the legacy
WebDriver `Element Click` command did not reliably trigger this app's Radix/shadcn-styled
buttons -- a real, previously-undiscovered quirk, not a workaround invented to dodge a problem),
creates a real vault, and navigates to and screenshots the real Settings page. Building and
running it for the first time also surfaced a real, previously-undiscovered production bug: a
plain `cargo build --release` (not through the Tauri CLI) served the webview from `devUrl`
(`localhost:1420`) instead of the embedded frontend, because `src-tauri/Cargo.toml` was missing
the `default = ["custom-protocol"]` feature every Tauri scaffold has -- fixed there. Without that
fix, any real release build handed to a user would have shipped a blank "can't reach this page"
window. This is a manual script, not wired into CI (WebView2/Edge automation needs a real
Windows desktop session, which GitHub-hosted `windows-latest` runners do provide but this pass
did not attempt to wire up) -- run it by hand per its own header comment.

Release additionally requires: signed Windows and Android binaries, no debug unlock path, no
test secrets in the bundle, and a passing egress test.
