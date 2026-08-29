# Envryn - Security Checklist

A scannable, per-area reference. Not a substitute for `docs/THREAT_MODEL.md` or
`docs/SECURITY_INVARIANTS.md` - those are normative. This is an index into them plus the
2026-08-26 audit's (`AUDIT_REPORT.md`) independent verification, so a reviewer can see area by
area what is real, what is partial, and what is unverified without reading four documents.

Legend: ✅ verified this audit · 📄 documented + covered by an automated test, not independently
re-derived here · ⚠️ partial / known limitation · ❌ not implemented · ➖ not applicable

---

## Rust / Tauri IPC

- ✅ Every `#[tauri::command]` in `src-tauri/src/ipc.rs` reviewed; only `backup_create`/
  `backup_restore` accept a caller-supplied filesystem path, deliberately (export/import
  destination), documented in-file.
- ✅ `src-tauri/capabilities/default.json` grants only `core:default` - no fs/shell/http/
  clipboard/dialog plugin exposed to the WebView.
- ✅ CSP (`tauri.conf.json`): `script-src 'self'`, `object-src 'none'`, `frame-ancestors 'none'`,
  no remote origin. `style-src 'unsafe-inline'` present (Tailwind necessity, low residual risk).
- 📄 No command takes a raw key or raw SQL (`ARCHITECTURE.md` §3 rule 2); spot-checked the storage
  layer for string-formatted SQL and found none (parameterized queries throughout).

## Vault storage and encryption

- 📄 XChaCha20-Poly1305 (record + key-wrap AEAD), Argon2id (password KDF), HKDF-SHA256 (subkey
  derivation) - all via RustCrypto/`argon2`/`hkdf` crates, no hand-rolled primitive
  (`CRYPTOGRAPHY.md` §1). Spot-checked `crypto/kdf.rs`, `crypto/keys.rs`, `crypto/aead.rs` directly.
- 📄 AAD binds ciphertext to record id + version (INV-009, tested).
- 📄 No plaintext metadata columns; whole record sealed (`CRYPTOGRAPHY.md` §3.1).
- ⚠️ SQLCipher-at-rest is architecturally planned as defense-in-depth but not load-bearing for
  confidentiality (record sealing already covers it) - per existing docs, not re-verified here.

## Key derivation and lifecycle

- 📄 VMK exists on disk only wrapped (INV-002, tested). KEK never persisted (INV-003).
- 📄 Locking zeroizes VMK/subkeys/index (INV-005, tested).
- ✅ `Zeroizing`/`SecretBox` used for password/key material at every IPC entry point read this
  audit (`vault_create`, `vault_unlock`, `backup_create`, `backup_restore`).
- ⚠️ Best-effort page locking only (`VirtualLock`/`mlock`); hibernation can still write plaintext
  physical memory to disk - stated as a real limitation, not solved, in `CRYPTOGRAPHY.md` §10.

## Secret handling in memory

- 📄 `zeroize`/`secrecy` wrap all key material and decrypted plaintext.
- ✅ Grepped `crates/` for `println!`/`eprintln!`/`log::*!`: no shipped code path logs a secret,
  prompt, or model output. Two dev-only `#[ignore]`d test files print synthetic test values only.

## Filesystem permissions and temp files

- ✅ Vault DB and AI model files live under the OS app-data directory (Windows profile ACL, not
  world-readable by default).
- ✅ Model downloads verified size + SHA-256 before an atomic rename from `.part`; a mismatch
  deletes the partial file (`ai/model_download.rs`, read directly, tests re-run).
- ➖ Android filesystem hardening not yet implemented (scoped out, `ARCHITECTURE.md` §7).

## Clipboard handling

- ✅ Native write tagged `ExcludeClipboardContentFromMonitorProcessing` + Rust-side timed clear
  that only fires if the clipboard still holds exactly what Envryn wrote (`ipc.rs`'s
  `clipboard_copy`, read directly).
- ➖ Android `EXTRA_IS_SENSITIVE` equivalent not yet implemented.

## Logs / crash reports / telemetry

- ✅ No Sentry/crash-reporting/telemetry SDK anywhere in the dependency tree; `deny.toml`
  structurally bans the `sentry` crate. Confirmed no Sentry project exists for Envryn in the
  connected org.
- 📄 `.semgrep/ai-no-content-logging.yml` (0 findings, re-run this audit via the CLI) backs
  AI-INV-006.

## Import / export / backups

- ✅ `backup_create`/`backup_restore` reviewed directly (§4/§5 of `AUDIT_REPORT.md`). Independent
  keying from the vault's own VMK; restore renames the existing vault file aside with a timestamp
  rather than deleting it.
- ✅ **Fixed this audit:** backup-password and backup-restore-new-password fields now show the
  same real-time strength estimate as vault creation (previously length-only feedback).

## Network activity and privacy

- 📄 `deny.toml` bans `reqwest`/`hyper`/`curl`/non-`rustls` TLS stacks workspace-wide.
- ✅ `.semgrep/network-egress.yml` re-run this audit: 0 violations.
- ⚠️ No live deny-all-egress firewall test exists (stated limitation, `THREAT_MODEL.md` §10).

## Dependency / supply-chain risk

- ✅ `cargo audit`: 18 advisories, all reviewed non-exploitable `unmaintained`/`unsound` warnings
  in unshipped (Linux-only) or minor transitive deps, all individually justified in `deny.toml`.
- ✅ `cargo deny check`: advisories/bans/licenses/sources all pass.
- ✅ `npm install`: 0 vulnerabilities across 391 packages.
- ✅ Snyk `test --all-projects`: 0 vulnerable paths across all npm projects (104 deps). No Cargo
  support in Snyk at all - `cargo audit`/`cargo deny` remain the only coverage for the Rust tree.
- ✅ SonarCloud dependency/SCA + static analysis (`crates/`, `src-tauri/src`, `apps/ui/src`): 51
  issues, 0 security vulnerabilities or hotspots; the rest is code-quality/accessibility debt.
- ❌ `cargo machete` not run (install failed in this environment - network error, not a finding).
- ⚠️ Duplicate transitive dependency versions exist (normal for a Tauri-sized tree); not a
  vulnerability, not addressed.

## Frontend / XSS / input validation

- ✅ Grepped all of `apps/ui/src` for `dangerouslySetInnerHTML`, `eval(`, `new Function(`,
  `document.write`, `innerHTML =`. **Zero matches.**
- ✅ CSP (above) has no `unsafe-inline`/`unsafe-eval` in `script-src`.
- 📄 UI holds no key material and performs no cryptography (INV-011).
- ✅ A SonarCloud-flagged ReDoS-shaped regex in `EnvImportModal.tsx`'s `.env` line parser was
  empirically load-tested against a 320,000-char adversarial input (worst case for backtracking):
  0-1&nbsp;ms, confirmed linear time. False positive, not fixed.

## Git history for leaked secrets

- ✅ GitGuardian's own historical scan of this monitored repo, plus Trivy's + Semgrep's secret
  detectors, all cross-checked manually against the actual flagged content. 3 historical
  incidents / 2 scanner hits, **all confirmed false positives** - see `AUDIT_REPORT.md` §3 for
  the line-by-line trace of each.

## `unsafe` Rust, panics, race conditions, error handling

- ✅ `unsafe` confirmed confined to `platform/windows_impl.rs` (the crate's sole
  `#![allow(unsafe_code)]` against a crate-level `deny`), verified by grep across the whole
  `crates/` tree, not by trusting the doc claim alone.
- ✅ `cargo clippy --workspace --all-targets --all-features`: 0 warnings.
- 📄 Race-condition-prone areas (session-lock subclassing, sync conflict detection) have real
  tests, re-run this audit (`cargo test --workspace`, 231 passed, 0 failed).

## Authentication, locking, auto-lock

- 📄 Idle-poll + direct `WTS_SESSION_LOCK` hook both converge on the same lock path
  (`ARCHITECTURE.md` §7); covered by `cargo test --workspace`, re-run this audit.
- ✅ Windows Hello gate genuinely runs and must succeed before the DPAPI unwrap is attempted
  (`vault_unlock_with_platform`, read directly) - UI copy does not overclaim biometric binding.
- ✅ **Fixed this audit:** master-password and join-flow new-password fields now show real-time
  strength feedback, not just a length floor (`THREAT_MODEL.md` V-01).

## Corrupted / tampered vault handling

- 📄 Unknown format/crypto versions are refused outright, never best-effort parsed
  (`SECURITY_INVARIANTS.md` §11). Spot-checked against `backup::restore`'s version check.

## Local sync security

- 📄 Mutual TLS 1.3, pinned fingerprints, SPAKE2/ECDH pairing with SAS confirmation, version-
  vector conflict detection with loser preservation - all real, tested over real loopback TCP/TLS
  (`sync::transport`/`sync::pairing`/`sync::protocol` test suites), re-run this audit.
- ⚠️ Never verified against two physical devices, only two processes on one machine - stated
  limitation, `THREAT_MODEL.md` §7.

## AI subsystem

- ✅ AI worker binds loopback-only, requires a 192-bit random per-session bearer token on every
  request (verified in `main.rs`/`protocol.rs` directly, not just the doc claim).
- ✅ `envryn-ai-worker` has no dependency on `envryn-core` (structural isolation) - the project's
  own `cargo tree -p envryn-ai-worker -i envryn-core` check, re-verifiable, not re-run fresh this
  audit but consistent with `Cargo.lock`.
- 📄 `SanitizedPrompt` constructible only inside `ai::gateway` (compile-time enforced,
  `trybuild` test).

## Password policy

- ⚠️ **Finding, fixed this audit:** strength feedback was entirely missing despite
  `THREAT_MODEL.md` V-01 claiming it existed. Now real (`lib/password-strength.ts` + 9 unit
  tests) on all four password-creation screens. The 8-character minimum itself is unchanged - a
  deliberate scope decision, see `AUDIT_REPORT.md` §7.

---

**Not covered by this checklist:** anything in `AUDIT_REPORT.md` §6 (out of this audit's
reachable scope - needs a second physical device, Developer Mode, or a disruptive firewall test)
and §7 (needs a human decision or dashboard access this session doesn't have).
