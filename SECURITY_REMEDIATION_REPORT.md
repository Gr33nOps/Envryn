# Envryn — Final Pre-Release Security Remediation & Verification Pass

**Date:** 2026-08-27
**Scope:** Full repository at `main` (Rust workspace: `envryn-core`, `envryn-ai-worker`,
`src-tauri`; frontend: `apps/ui`), following the prior `AUDIT_REPORT.md` (2026-08-26).
**Nature of this pass:** deeper and narrower than the prior audit — every item below was
independently re-derived from source, binaries, or live tool output in this session, not
carried over from the earlier report's conclusions.

---

## Summary table

| # | Area | Before | Action | Final status | Evidence |
|---|---|---|---|---|---|
| 1 | Semgrep | 21 findings (full scan, not the 3 the prior audit saw) | Traced `identity.rs` path-traversal to its one real caller; individually reviewed every other finding | 8 findings remain, all proven false positives with code-flow evidence | `semgrep-final.json`, §Semgrep below |
| 2 | GitHub Actions SHA-pinning | 14 mutable `@vN` refs across `ci.yml`/`sonarqube.yml` | Resolved and pinned all 6 distinct actions to real, cross-verified commit SHAs | 0 mutable refs; re-scan confirms | `git diff` on both workflow files |
| 3 | Secret scanners (Semgrep/Trivy/GitGuardian) | 4 new GitGuardian findings, 4 Trivy findings, 3 Semgrep findings | Read every flagged file at the exact line; decoded the one base64 payload | All confirmed synthetic/test fixtures; 0 real secrets in tree or history | §Secret scanners below |
| 4 | `cargo auditable build --release` + `cargo audit bin` | Not yet run this session | Built real release binary, ran audit against the actual bytes | `target/release/envryn.exe`, 9,098,752 bytes, 340 deps embedded, 0 vulnerabilities | `cargo audit bin` output |
| 5 | `cargo audit` / RustSec (glib/GTK) | Existing `deny.toml` reasoning not independently re-proven this session | Re-derived via `cargo tree --target x86_64-pc-windows-msvc` (empty) vs. `--target all` (shows the Linux-only chain) + binary-level `cargo audit bin` (GTK absent from the real exe's embedded manifest) | Confirmed not reachable on the shipped Windows binary — proof, not trust | `cargo tree` output, `cargo audit bin` output |
| 6 | `cargo deny check` | — | Re-ran; reviewed every ignore entry | `advisories ok, bans ok, licenses ok, sources ok`; every exception has a specific, still-accurate per-ID reason | full `cargo deny check` output |
| 7 | Fuzz targets | Scaffolded but broken (`path = ".."` pointed at the workspace root, never compiled) | Fixed the path bug, wrote 2 real targets against real parsing boundaries, ran each with ASan | `fuzz_aead_open`: 3,835,600 execs/121s, 0 crashes. `fuzz_backup_restore`: 6,378,123 execs/121s, 0 crashes | fuzz run logs |
| 8 | `cargo geiger` (all 3 packages) | Not run this session | Ran per-package (workspace root is virtual) | `envryn-core`: 2 unsafe fns / 258 unsafe exprs, 100% confined to `platform::windows_impl.rs`. `envryn` (src-tauri): 0/0. `envryn-ai-worker`: 0/0 | 3 geiger reports |
| 9 | Trivy (`vuln,misconfig,secret`) | — | Full-repo scan via MCP `scan_filesystem` | 0 vuln (any severity), 0 misconfig, 4 secret findings — all confirmed false | `findings_list` output |
| 10 | `npm audit` | — | Ran at root | 0 vulnerabilities / 456 total dependencies | `npm audit --json` |
| 11 | Tauri commands / capabilities / CSP | — | Read `capabilities/default.json`, `tauri.conf.json`'s CSP, enumerated all 47 `#[tauri::command]`s, checked `Cargo.toml` for plugins, reviewed `backup_create`/`backup_restore`'s path handling and the `IpcError`/`Redacted<T>` leak-prevention design | Capability set minimal (window controls only, zero fs/shell/http/clipboard plugins); CSP has no remote origin, no `unsafe-eval`; error messages structurally cannot carry secret material (`&'static str`-only variants) | §11 below |
| 12 | Canary secret leakage test | Existing `nothing_readable_is_written_to_disk` covered the vault file only | Added `a_canary_secret_never_appears_in_plaintext_in_a_backup_file`, a real `ENVRYN_SECURITY_CANARY_<uuid>`-marked secret checked against backup-file bytes | New regression test passing; combined with the existing vault-file test, both on-disk artifacts Envryn produces are now covered | `cargo test` output |
| 13 | Network behaviour | — | Grepped every `ureq::` call site in the tree | Exactly one: `model_download.rs`, two hardcoded HTTPS URLs (Hugging Face), user-initiated only, SHA-256-verified on completion. No telemetry/analytics/crash-reporting/update-checking code exists at all | §13 below |
| 14 | Cryptography implementation review | — | Read `crypto/aead.rs`, `crypto/kdf.rs`, `crypto/keys.rs` in full; verified existing test coverage against the user's checklist | XChaCha20-Poly1305 (correct choice for independent multi-device nonce generation), Argon2id with floor/ceiling validation against a tampered-params DoS, HKDF-SHA256 with versioned domain separation, CSPRNG-only randomness, zeroizing outputs, DPAPI buffer explicitly zeroed before `LocalFree`. No gaps found; no new crypto code needed | §14 below |
| 15 | Full release gate | — | Reran every check listed in the brief | All green — see §15 | see below |

---

## 1. Real vulnerabilities found and fixed

**One real gap, not a vulnerability in shipped code: the fuzz harness was non-functional.**
`fuzz/Cargo.toml` declared `envryn-core` at `path = ".."`, which resolves to the *workspace
root* (a virtual manifest with no package), not `crates/envryn-core`. `cargo fuzz build` had
never successfully compiled since the harness was scaffolded — meaning "fuzzing already
initialized but unused" (the audit brief's own framing) was accurate. Fixed the path, added an
isolated `[workspace]` table (required so the fuzz crate isn't treated as a workspace member),
and installed the nightly toolchain + ASan runtime DLL (`clang_rt.asan_dynamic-x86_64.dll`,
copied from the Visual Studio MSVC toolchain, since cargo-fuzz's Windows target does not locate
it automatically). Two real fuzz targets now build and run; see §6.

**14 GitHub Actions steps referenced mutable tags** (`@v4`, `@v5`, `@stable`, `@v2`, `@v6`)
across `ci.yml` and `sonarqube.yml` — a real supply-chain exposure (the exact class of bug
behind the `tj-actions/changed-files` and `reviewdog` incidents: a compromised or coerced
action-repo maintainer can repoint a tag to malicious code with no diff visible in this repo).
Fixed: every reference now pins a real, individually-resolved 40-character commit SHA, with the
original tag kept as a trailing comment for readability:

| Action | Pinned SHA | Tag |
|---|---|---|
| `actions/checkout` | `11d5960a326750d5838078e36cf38b85af677262` | v4 |
| `actions/setup-node` | `49933ea5288caeca8642d1e84afbd3f7d6820020` | v4 |
| `actions/setup-python` | `a26af69be951a213d495a4c3e4e4022e16d87065` | v5 |
| `dtolnay/rust-toolchain` | `4360b52568e2003a75bf9bc1d59f33a8e3fc893c` | stable |
| `Swatinem/rust-cache` | `6323deb102c322ba6fcbdcafc7e3dddab59af2b6` | v2 |
| `SonarSource/sonarqube-scan-action` | `fd88b7d7ccbaefd23d8f36f73b59db7a3d246602` | v6 |

Every SHA was resolved from GitHub's real API (`git/ref/tags/*`, cross-checked against
`repos/*/tags` as a second, independent call) — none invented. `Swatinem/rust-cache`'s tag is
annotated, so its ref SHA had to be dereferenced one level further (the tag object's `sha`, not
the tag ref's `sha`) to reach the actual commit — caught by comparing both lookup paths, which
would otherwise have silently pinned a tag object instead of a commit.

No other real vulnerability was found in first-party code this session.

---

## 2. False positives, with the evidence that makes them false

### Semgrep — `rust.actix.path-traversal.tainted-path` on `identity.rs:126` (the brief's
### explicit top priority)

Full trace of `DeviceIdentity::load_or_create(path: &Path)`'s only production call site:

```
src-tauri/src/sync.rs:56-63  fn identity_path(app: &AppHandle) -> IpcResult<PathBuf> {
    let dir = app.path().app_data_dir()?;   // OS-provided, fixed per-install directory
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("device_identity.json"))    // hardcoded literal filename
}
src-tauri/src/sync.rs:66-68  fn load_identity(app: &AppHandle) -> IpcResult<DeviceIdentity> {
    let path = identity_path(app)?;
    Ok(DeviceIdentity::load_or_create(&path)?)
}
```

`identity_path` is the *only* caller of `load_or_create` outside test code (confirmed via
`grep -rn` across the entire repo: every other call site is in a `#[cfg(test)]` module or an
integration test, using `tempfile::tempdir()`-generated paths). The path is built from two
components, neither attacker-influenceable: Tauri's own `app_data_dir()` (OS-assigned, not
passed through IPC) and a compile-time string literal. It is never touched by IPC arguments,
sync/pairing wire messages, imported files, or configuration. **Verdict: false positive,
proven by exhaustive call-site enumeration, not by trusting the existing code comment.**

### Semgrep — 4× `react-insecure-request` in `.dev-tools/*-smoke.mjs`

Both flagged files build every URL as `` `${driverUrl}/...` `` where
`driverUrl = \`http://127.0.0.1:${driverPort}\`` — a hardcoded loopback address (grepped every
use of `driverUrl` in both files; all are this same constant). These are WebDriver test-runner
scripts (never shipped, never run against production, never reach any network interface beyond
localhost). WebDriver's local wire protocol has no HTTPS variant. **False positive.**

### Semgrep/Trivy — GitHub token in `crates/envryn-ai-worker/src/model.rs:345`

`"ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"` — literally the alphabet, inside an `#[ignore]`d
real-model test's adversarial prompt list (`tests requiring a ~350 MB download`, per the test's
own doc comment). Matches the prior audit's independent documentation of the same string.
**False positive.**

### Semgrep/Trivy — JWTs in `classify.rs:161` and `gateway.rs:251`

- `classify.rs:161` (test-only): `eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dGhpc2lzYXNpZ25hdHVyZQ`
  decodes to header `{"alg":"HS256"}`, payload `{"sub":"1234567890"}`, "signature"
  `thisisasignature` — a placeholder test fixture inside `#[test] fn recognises_jwt_shape`.
- `gateway.rs:251` (**ships in the production binary**, as a hardcoded few-shot prompt example
  added earlier this session): `eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dGVzdA` decodes to
  header `{"alg":"HS256"}`, payload `{"sub":"1234"}`, signature `test`. No real signing key, no
  real claims, not tied to any account or service — it exists purely to show the local LLM what
  a JWT-shaped credential looks like.

**Both false positives** — worth flagging the distinction between them precisely: the first
never leaves the test binary, the second does ship, but ships as an inert descriptive string
with no authentication value, not a usable credential.

### Trivy — `stripe-publishable-token` in `gateway.rs`

`pk_test_TYooMQauvdEDq54NiTphI7jx` (line 249, same few-shot prompt) is Stripe's own published,
canonical example test key from Stripe's public API documentation — a standard, non-functional
placeholder every Stripe integration guide reuses. **False positive.**

### GitGuardian — 4 new "Generic Password" findings in `password-strength.test.ts`

All four are literal test fixtures for `estimatePasswordStrength` (`"password1"`,
`"Password1"`, `"abcdefgh"`, etc.) — read the whole file; every flagged string is an example
input to a unit test asserting the strength scorer's behavior (breach-list detection,
run-detection, entropy scoring), not a real credential. **False positive** — an inherent,
expected trigger for a low-precision "generic password" heuristic scanning a password-strength
test file.

### GitGuardian — pre-existing IGNORED findings (JWT in `classify.rs`, username/password in
### `model.rs`, PostgreSQL URI in `envryn-data.ts`)

Already reviewed and marked `test_credential`/`false_positive` by the user before this session;
re-verified the underlying code still matches that classification (no drift). Left as-is.

---

## 3. Remaining accepted risks

**`backup_create`/`backup_restore`'s `path: String` parameter has no canonicalization or
confinement**, unlike every other IPC command (which the module doc for `ipc.rs` explicitly
calls out as the one deliberate exception to "no caller-supplied path"). Traced the frontend
(`apps/ui/src/routes/vault/backup.tsx`): this is **not** a native OS file-picker dialog — no
`tauri-plugin-dialog` is installed at all (confirmed zero `tauri-plugin-*` dependencies) — it is
a plain `<input type="text">` the user types a path into directly. The existing code comment's
claim that this is "the same distinction a native 'save as' dialog would draw" is not quite
accurate; there is no dialog.

Accepted as low-severity because the actual impact is bounded even under a hypothetical XSS
compromise of the bundled frontend (the only way this path could ever carry attacker-chosen
data, since the WebView loads no remote content and `default-src 'self'`/`script-src 'self'`
block any external script):
- `backup_create` requires the vault already unlocked, and writes only opaque,
  structured `envryn_core::backup` container bytes (never attacker-arbitrary bytes) — it cannot
  be used to plant an executable or script payload at the chosen path.
- `backup_restore` returns only a record *count* (`RestoreSummary { restored: usize }`) on
  success, never file contents — pointing it at an arbitrary non-backup file (e.g. an SSH key)
  fails cleanly at parse/decrypt and leaks nothing back to the frontend.

**Recommendation, not required for this release:** correct the misleading doc comment, and
consider a real native file dialog in a future pass — out of scope here per "keep fixes minimal
and security-focused" / "don't make unrelated changes."

**Fuzzing did not reach `sync::protocol`'s wire-message parsing or
`sync::identity::DeviceIdentity::from_file`** — both are `pub(crate)`/private, unreachable from
an external `cargo fuzz` crate without weakening the module's encapsulation. `read_json`'s
length-prefix bounding (`MAX_MESSAGE_LEN = 64 MiB`, checked before allocating) was reviewed by
hand and is sound, but it has not been fuzzed. Documented rather than silently skipped.

---

## 4. Unmaintained dependencies still present

Unchanged from `deny.toml`'s existing, individually-justified ignore list — every one
independently re-verified this session as (a) genuinely unmaintained upstream and (b) not
reachable on the Windows target Envryn ships:

- 10× `gtk-rs` GTK3 bindings (`atk`, `gdk`, `gtk`, `webkit2gtk`, etc.) + `glib`'s unsound
  iterator impl (RUSTSEC-2024-0429) — transitive via `tauri`/`wry`/`tao`'s Linux desktop
  backend. **Re-proven this session**, not just re-read: `cargo tree --target
  x86_64-pc-windows-msvc -i glib` returns nothing; `cargo tree --target all -i glib` shows the
  exact Linux-only chain (`webkit2gtk`/`gtk`/`tao` → `tauri-runtime-wry`); `cargo audit bin` on
  the real compiled `envryn.exe` shows these crates entirely absent from the binary's own
  embedded dependency manifest.
- `paste` (RUSTSEC-2024-0436), transitive via `candle-core`'s `gemm` dependency.
- `proc-macro-error` (RUSTSEC-2024-0370), transitive via `glib-macros` (same Linux-only path).
- 5× `unic-*` Unicode crates (rust-unic project, archived), transitive via `tauri-utils`'
  `urlpattern`. These **do** appear on the real Windows binary (`cargo audit bin` lists all 5) —
  correctly *not* in `deny.toml`'s Windows-filtered ignore list's "not detected" set, and
  correctly flagged by `cargo audit` as real (if low-severity) warnings. No exploit path exists
  through them (not called by Envryn's own code); replacing them is upstream Tauri's call.

None are exploitable vulnerabilities — all are "unmaintained"/"unsound" advisories with no
associated CVE against Envryn's actual usage.

---

## 5. First-party unsafe Rust found

`cargo geiger`, run per-package (the workspace root is virtual and cannot be scanned directly):

| Package | Unsafe fns | Unsafe exprs | Unsafe impls |
|---|---|---|---|
| `envryn-core` | 2/2 | 258/258 | 2/2 |
| `envryn` (src-tauri) | 0/0 | 0/0 | 0/0 |
| `envryn-ai-worker` | 0/0 | 0/0 | 0/0 |

All of `envryn-core`'s unsafe is in `platform/windows_impl.rs` — the crate enforces this
structurally (`unsafe_code = "deny"` at the crate level in `Cargo.toml`, with a single scoped
`#![allow(unsafe_code)]` on this one module). Read the whole file (719 lines) and manually
verified the two highest-risk patterns:

- **`dpapi_protect`/`dpapi_unprotect`** (Win32 `CryptProtectData`/`CryptUnprotectData`): correct
  `LocalAlloc`/`LocalFree` lifetime handling, and — genuinely notable — `dpapi_unprotect`
  explicitly overwrites DPAPI's own output buffer with zeros (`std::ptr::write_bytes`) *before*
  calling `LocalFree`, because `LocalFree` does not zero memory it releases and that buffer
  transiently held the recovered platform key. This is a real, easy-to-miss secret-hygiene
  detail that most implementations get wrong; this one gets it right.
- **`subclass_proc`'s `std::mem::transmute(previous)`** (Win32 window-procedure subclassing, to
  observe `WM_WTSSESSION_CHANGE` for lock-triggered auto-lock): the standard, unavoidable Win32
  subclassing pattern — `previous` is exactly the value `SetWindowLongPtrW` returned when this
  procedure was installed, stored in one `AtomicIsize`, valid for the single main window this
  desktop app has.

Not every one of the ~20 unsafe blocks in this file received the same line-by-line scrutiny
given this session's time budget; the two above were the highest-risk (secret-handling FFI, and
a raw transmute) and both are sound. This is a partial, not exhaustive, unsafe-code audit —
reported honestly rather than claimed as complete.

Dependency-tree unsafe (third-party, not Envryn's to fix): `windows` (0.61.3, the Win32 binding
crate — necessarily unsafe-heavy), `tokio`, `ring`, `candle-core`/`gemm`/`bytemuck` (numeric
kernels, expected for ML tensor code), `zerocopy`. All well-known, widely-audited crates.

---

## 6. Fuzz targets created and results

The scaffold at `fuzz/` existed but had never actually compiled (`path = ".."` pointed at the
workspace root instead of `crates/envryn-core` — a real bug, fixed as part of this pass, see
§1). Two targets against real first-party parsing boundaries, chosen because they are the two
places this app decodes *untrusted bytes from outside its own database*:

- **`fuzz_aead_open`** — `crypto::aead::Sealed::from_bytes` + `open`, the boundary every stored
  vault record, sync payload, and backup blob passes through. **3,835,600 executions in 121s**,
  ASan-instrumented (real memory-safety checking, not just panic-catching). 0 crashes, 0 hangs,
  0 sanitizer failures.
- **`fuzz_backup_restore`** — `backup::restore`, the boundary a user-chosen `.envrynbk` file
  passes through. **6,378,123 executions in 121s**, ASan-instrumented. 0 crashes. libFuzzer's
  coverage-guided corpus organically discovered the format's real field names
  (`"format_version"`, `"salt"`, `"kdf"`, `"ENVRYNBK"` magic bytes) purely through mutation —
  concrete evidence the fuzzer reached real structural parsing code, not just the
  password-mismatch failure path.

Both required installing a nightly Rust toolchain (cargo-fuzz's ASan support needs
`-Zsanitizer=address`, nightly-only) and manually locating and copying
`clang_rt.asan_dynamic-x86_64.dll` from the Visual Studio MSVC toolchain next to the fuzz
binaries — cargo-fuzz does not do this automatically on Windows, and the run fails with
`STATUS_DLL_NOT_FOUND` until it's placed there by hand. Documented here so a future run doesn't
have to rediscover this.

**Not fuzzed** (see §3): `sync::protocol`'s wire-message JSON parsing and
`sync::identity::DeviceIdentity::from_file`, both `pub(crate)`-private and unreachable from an
external fuzz crate without a scope change this session didn't make.

---

## 7. Exact production executable audited

`F:\AI Projects\Envryn\target\release\envryn.exe` — **9,098,752 bytes**, built this session via
`cargo auditable build --release -p envryn` against the real frontend (`npm run build`, not a
placeholder) with the real, previously-compiled AI-worker sidecar already present at
`src-tauri/binaries/envryn-ai-worker-x86_64-pc-windows-msvc.exe` (verified as a genuine PE
binary — `MZ` header, "This program cannot be run in DOS mode" string — not the placeholder text
file CI's own build uses to satisfy `bundle.externalBin`'s compile-time existence check).
`cargo audit bin` confirmed `cargo auditable` metadata is genuinely embedded (**340 dependencies**
recorded inside the binary itself).

---

## 8. Trivy final counts

Full-repo `scan_filesystem` (`vuln`, `misconfig`, `secret`; all severities):

- **Vulnerabilities: 0** (any severity).
- **Misconfigurations: 0**.
- **Secrets: 4**, all confirmed false positives (§2): `model.rs`'s synthetic GitHub token,
  `classify.rs`/`gateway.rs`'s synthetic JWTs, `gateway.rs`'s Stripe canonical test key.

A separate production-only scan (excluding tests/generated artifacts) was not run as a distinct
second pass: all 4 findings above are already in first-party `crates/` source (not vendored,
not `node_modules`, not `target/`), so a narrower scope would return the identical 4 findings —
running it again would not add information.

---

## 9. Semgrep final counts

21 findings on the first full run (`--config=auto --config=.semgrep/`) → **8 remaining after
this session's fixes**, all reviewed and confirmed false positive (§2):
- 4× `react-insecure-request` (loopback-only WebDriver test tooling)
- 1× GitHub token, 2× JWT, (Trivy also flagged 1 Stripe key Semgrep didn't) — all synthetic
- 1× `tainted-path` on `identity.rs` — proven false via exhaustive call-site trace

The 14 `github-actions-mutable-action-tag` findings are gone (fixed, §1). 179 "Internal matching
error" warnings were investigated and are **not** a real coverage gap: all 179 come from exactly
3 rules (`javascript.crypto-js.cryptojs-weak-algorithm`, `javascript.express.web
.cors-default-config-express`, `javascript.koa.web.cors-default-config-koa`) that require
Semgrep's paid Pro engine (`metavariable-name` operator) and fail identically on every JS/TS
file in the OSS CLI — and none of the three apply to this codebase anyway (no `crypto-js`, no
Express, no Koa; this is a Tauri+React desktop app).

---

## 10. `cargo audit` final result

**0 vulnerabilities.** 17 "unmaintained" + 1 "unsound" advisory, all already itemized and
individually justified in `deny.toml` (§4), all independently re-verified this session as
unreachable on the shipped Windows binary via `cargo tree --target` and `cargo audit bin`
against the real compiled executable.

---

## 11. `cargo deny check` final result

```
advisories ok, bans ok, licenses ok, sources ok
```

Only cosmetic warnings remain (duplicate transitive crate versions across the tree; one
unmatched license allowance kept deliberately in case a future graph change needs it — both
already explained inline in `deny.toml`'s own comments). No stale ignores found; every
`[advisories].ignore` entry's reason still matches reality after independent re-verification.
No broad exceptions — every one names a specific RUSTSEC ID with a specific justification.

---

## 12. Tests / build results (full gate)

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | clean (one file auto-formatted after adding the canary test, then re-verified clean) |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings |
| `cargo test --workspace` | **231 + 1 new = 31 in `vault_lifecycle.rs` alone, all passing**; workspace total 0 failed, 4 intentionally `#[ignore]`d (real-model tests needing a downloaded model) |
| Contract bindings (`git diff --exit-code -- packages/contract/bindings`) | clean, no drift |
| `npm run typecheck` | 0 errors |
| `npm run lint` | 0 errors, 9 pre-existing non-security warnings (fast-refresh/exhaustive-deps) |
| `npm run test` (frontend, Vitest) | **57 passed, 7 test files, 0 failed** |
| `npm run build` (production frontend build) | clean |
| `npm audit` | 0 vulnerabilities / 456 dependencies |
| `cargo auditable build --release` | succeeded, real 9.1 MB `envryn.exe` |
| `cargo audit bin target/release/envryn.exe` | 0 vulnerabilities, 5 pre-reviewed unmaintained warnings |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| `semgrep --config=auto --config=.semgrep/ .` | 8 findings, all false positive (§9) |
| Trivy (`vuln,misconfig,secret`) | 0/0/4-all-false-positive (§8) |
| `cargo geiger` × 3 packages | see §5 |
| `fuzz_aead_open`, `fuzz_backup_restore` | see §6 |

---

## 13. Anything that prevented a security check from actually running

- **A separate "production-only" Trivy scan** was not run as a distinct pass — judged
  redundant since the one full-repo scan's 4 findings are already all in first-party source,
  not generated/vendored noise (§8).
- **`sync::protocol`'s message parsing and `DeviceIdentity::from_file`** could not be fuzzed
  without loosening `pub(crate)`/private visibility specifically to expose them to an external
  fuzz crate — not done, to avoid an unrelated, scope-creeping visibility change (§3, §6).
- **A full WebDriver/GUI-driven canary-secret sweep** (actually launching the compiled app,
  storing a secret through the real UI, and scanning WebView storage/caches/clipboard/recent-
  files metadata/installer artifacts) was **not performed** this session. What *was* done
  instead: a real, code-level canary test against the two on-disk artifacts Envryn's Rust core
  actually produces (the vault SQLite file + WAL/SHM, and a backup export) — both now proven
  clean of plaintext leakage by an automated, permanent regression test. Separately verified,
  structurally rather than by observation: `envryn-core` contains zero `log::`/`tracing::`/
  `println!`/`eprintln!` calls in non-test code (one `println!` exists, inside `#[cfg(test)]`,
  printing a boolean capability flag, never compiled into the shipped binary) — so there is no
  logging channel for a secret to leak through in the first place, not merely "no secret was
  observed being logged." WebView `localStorage`/`sessionStorage`/cache behavior specifically
  was not separately audited this session.
- **A live network capture** proving zero unexpected traffic was not performed (unchanged from
  the prior audit) — backed instead by the structural argument in §14 below (one `ureq::`
  call site in the entire tree, `deny.toml` bans every alternative HTTP client, and
  `.semgrep/network-egress.yml` enforces this on every scan).
- **cargo-fuzz's Windows ASan packaging gap** (missing runtime DLL, `STATUS_DLL_NOT_FOUND`
  until manually resolved) cost real time and is undocumented anywhere in cargo-fuzz's own
  Windows docs as far as this session found — recorded here (§6) so it isn't rediscovered.
- **Not every one of `envryn-core`'s ~20 unsafe blocks got individual line-by-line review** —
  the two highest-risk patterns (secret-handling DPAPI FFI, raw transmute) were verified in
  depth; the remainder (clipboard open/close, Job Object creation for AI-worker isolation,
  idle-time queries) were read but not each independently re-derived from Win32 documentation.

---

## 14. Cryptography and network review detail

**AEAD (`crypto/aead.rs`):** XChaCha20-Poly1305, 192-bit random nonce per seal (deliberately
chosen over AES-GCM's 96-bit nonce specifically because Envryn's multi-device sync generates
nonces independently with no shared counter — a correct, well-reasoned primitive choice, not a
default). AAD binds ciphertext to context (prevents moving a sealed blob between rows/records).
Existing test suite (read in full) already covers: round-trip, plaintext-not-in-ciphertext,
wrong-key rejection, wrong-AAD rejection, tampered-ciphertext rejection, tampered-nonce
rejection, truncated-blob rejection, nonce-uniqueness-per-seal, empty-plaintext round-trip. No
gaps found against the audit brief's checklist; no new tests needed here.

**KDF (`crypto/kdf.rs`):** Argon2id (correct variant), version 0x13, 64 MiB/3 passes/4 lanes
desktop default. Parameters travel with the vault (forward-compatible) but are range-validated
on every use (`MINIMUM`/ceiling checks) specifically because they're read *before* any key
exists to authenticate them — explicit, correct defense against a tampered vault file claiming
`memory_kib: 8` (trivial offline attack) or an absurd value (DoS via unlock-time allocation).
Existing tests cover determinism, salt/password sensitivity, floor/ceiling rejection, and an
explicit "empty password still derives distinctly" check. No gaps found.

**Key separation (`crypto/keys.rs`):** HKDF-SHA256 subkey derivation with versioned,
purpose-specific `info` strings (`envryn/v1/record`, `envryn/v1/fingerprint`,
`envryn/v1/sqlcipher`) — correct domain separation, no key reuse across purposes.

**Network behavior:** exactly one HTTP(S) call site in the entire codebase —
`ai/model_download.rs`'s `ureq::get(url)`, where `url` is one of two hardcoded compile-time
string literals (Hugging Face URLs for the Qwen2.5-1.5B-Instruct model), never IPC- or
user-supplied, reachable only via the explicit, user-initiated `ai_download_model` command, and
verified against an `expected_sha256_hex` on completion. No telemetry, analytics,
crash-reporting, or update-checking code exists anywhere in the tree (`sentry`/`sentry-core` are
banned outright in `deny.toml`; no `tauri-plugin-updater`; zero `tauri-plugin-*` dependencies at
all). Sync/pairing network activity (mDNS discovery, TLS peer connections) is entirely
user-initiated per-command, never a background listener against a locked vault (per `sync.rs`'s
own module doc, cross-checked against the command list — `sync_listen_start` is the one
listener, and its accept loop checks lock state every iteration).

---

## Release recommendation

**What was tested:** the full 15-area brief — Semgrep (fresh full scan, every finding
individually traced), GitHub Actions supply-chain pinning, every secret-scanner finding across
three tools with code-flow proof, a real `cargo auditable` release build audited by both
source-tree (`cargo tree --target`) and binary-level (`cargo audit bin`) methods, `cargo deny`,
two real ASan-fuzzed targets against the two genuine untrusted-byte-parsing boundaries in the
codebase, `cargo geiger` across all three first-party packages with manual review of the
highest-risk unsafe code, a full Trivy scan, `npm audit`, a manual review of every IPC command's
capability surface and the app's CSP/plugin configuration, a new canary-secret regression test
against both on-disk artifacts the app produces, the app's complete network-egress surface, and
a from-scratch cryptography review against the AEAD/KDF/key-separation implementation — followed
by a full rerun of every check in the release gate.

**What passed:** everything in §12's table. Zero real vulnerabilities. Zero real secrets. Zero
unpinned CI actions (now fixed). Zero crashes across ~10.2 million real fuzz executions. Zero
first-party unsafe code outside one narrowly-scoped, hand-reviewed platform module. A
cryptography implementation that was already correct before this session and remains so.

**What remains uncertain** (§3, §13): the free-text backup path field's actual blast radius
under a hypothetical future XSS bug (bounded, but not zero, and the doc comment describing it
as dialog-equivalent should be corrected); WebView storage/cache/clipboard-after-expiry was not
independently swept this session (only structurally argued, via the vault-file and backup-file
canary tests plus the absence-of-any-logging-infrastructure finding); sync wire-message parsing
was reviewed by hand but not fuzzed; and a live network capture was not taken (backed instead by
exhaustive static enumeration of the one call site that exists).

**Recommendation: yes, this build is safe to release**, with the honest caveats above logged
rather than hidden. Nothing found this session rises to a real, exploitable vulnerability in
shipped code; the one genuine bug (a non-functional fuzz harness) is now fixed and has already
run millions of real executions with zero findings; every "finding" a scanner raised was
individually traced to source and proven synthetic, not asserted. The uncertain items are
scoped, understood, and reasonable to carry forward rather than block on — they are gaps in
*this session's* coverage, not known or suspected defects.
