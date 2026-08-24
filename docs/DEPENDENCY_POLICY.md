# Envryn — Dependency Policy

Envryn stores credentials. A dependency in the Rust core runs with access to the vault master
key; a dependency in the UI runs inside the WebView. Both are part of the attack surface, and
neither gets added casually.

---

## 1. Adding a dependency

Answer these in the pull request. If the answers are weak, write the code instead.

1. What does it do that we would otherwise write ourselves?
2. How much code would we write instead? *(Under ~100 lines of non-cryptographic logic — write it.)*
3. Is it maintained? Recent releases, responsive to advisories.
4. How many transitive dependencies does it pull in?
5. Does it contain native code, and does that code process untrusted input?
6. Does it perform I/O — filesystem, network, process spawning?
7. What is its licence? Permissive only (MIT / Apache-2.0 / BSD / ISC).
8. Could it reach key material or plaintext?

**Never write our own cryptography.** Question 2 does not apply to cryptographic primitives:
"write it ourselves" is always the wrong answer there.

---

## 2. Prohibited

These are blocked in `deny.toml`, and adding one requires amending this document:

- **Any HTTP client outside the model-download module.** `reqwest`, `ureq`, `hyper` as a client,
  and anything similar. INV-010 says Envryn makes no outbound connection except model download
  and LAN sync — that is enforced by the dependency graph, not by discipline.
- **Telemetry, analytics, and crash-reporting SDKs.** Any of them.
- **Alternative cryptographic implementations.** One implementation per primitive, listed in
  `CRYPTOGRAPHY.md`.
- **Crates with `unsafe` in a cryptographic path** unless audited and justified here.
- **Copyleft licences** (GPL, AGPL, SSPL).
- **Unmaintained crates**: no release and no advisory response in 24 months.

## 3. Requires explicit review

- Anything in `crates/envryn-core` — it runs with key access.
- Anything with native code or build scripts that download.
- Anything that spawns processes or touches the filesystem outside the vault directory.
- Anything in the inference runtime, tokenizer, model loader, or GPU acceleration path
  (spec section 26 — large native surfaces processing untrusted input).
- Archive and decompression libraries, which have a long history of path-traversal and
  memory-safety bugs and are reached by *downloaded* data.

## 4. Lower bar

UI-only dependencies that do not touch IPC — component libraries, icons, date formatting.
They still must be permissive-licensed and maintained, and they still ship to users. The bar is
lower because a WebView dependency cannot reach the VMK; it is not absent, because it can still
alter what the user sees before they approve something.

---

## 5. Pinning and updates

Both lockfiles (`Cargo.lock`, and the JS lockfile) are committed, including for the library
crates — reproducible builds matter more here than dependency-resolution flexibility.

Model checksums and the inference runtime version are pinned in application code, not fetched
at runtime. A checksum fetched over the network is not a checksum; it is a second thing to
compromise.

Security advisories are applied promptly. Routine updates are batched, reviewed, and land as
their own commits so a regression bisects cleanly rather than hiding inside a feature change.

---

## 6. Enforcement in CI

| Check | Tool |
|---|---|
| Known vulnerabilities | `cargo audit`, `npm audit` |
| Licences, bans, duplicate versions, untrusted sources | `cargo deny check` |
| No HTTP client outside `model_download` | Semgrep rule + `deny.toml` |
| AI worker does not depend on the vault crate | `cargo metadata` assertion |
| No telemetry endpoints in the bundle | Egress test + string scan |

The `cargo metadata` assertion deserves emphasis: it is what makes AI-INV-001, 002, 004 and 005
structural. The AI worker cannot receive a key because the types that represent keys are not in
its dependency graph at all — and CI fails the moment someone changes that, before review.

---

## 7. Current core dependencies

Every crate here is expected to be present. Anything else in `crates/envryn-core` is a review point.

**Cryptography** — `argon2`, `chacha20poly1305`, `hkdf`, `sha2`, `hmac`, `ed25519-dalek`,
`x25519-dalek`, `spake2`, `rustls`, `getrandom`, `zeroize`, `secrecy`

**Storage** — `rusqlite` (bundled SQLCipher), `serde`, `serde_json`

**Sync** — `tokio`, `rustls`, `rcgen` (self-signed device certificates), an mDNS crate

**Platform** — `windows` (DPAPI, Hello, display affinity), `jni` (Android Keystore, FLAG_SECURE)

**Tauri** — `tauri`, `tauri-plugin-biometric`, `tauri-plugin-barcode-scanner`

**Note on `tauri-plugin-stronghold`:** evaluated and **not** adopted. Envryn needs control over
the record format so that sealed payloads can move over sync, and the key hierarchy in
`CRYPTOGRAPHY.md` is specified around that requirement. Adopting a second, differently-shaped
secret store alongside it would add a dependency without removing any of our own code.
