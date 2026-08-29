# Envryn - Security & Production-Readiness Audit

> Historical review record. This report captures the project during the original internal audit. For current controls and commands, use the [documentation index](docs/README.md), [security testing guide](docs/SECURITY_TESTING.md), and live CI configuration.

**Date:** 2026-08-26
**Scope:** Entire repository at commit `dc00ebc` (main), Rust workspace (`envryn-core`,
`envryn-ai-worker`, `src-tauri`) and frontend (`apps/ui`).
**Review:** Internal project audit covering production readiness, security, privacy, code quality,
runtime behavior, remediation, and verification.

This audit builds on top of an already-mature internal security posture: `docs/ARCHITECTURE.md`,
`docs/THREAT_MODEL.md`, `docs/SECURITY_INVARIANTS.md`, and `docs/CRYPTOGRAPHY.md` already existed,
were detailed, and - as far as this audit could independently verify - accurately described the
implementation, with one exception (see Finding 1). This report documents what was independently
re-verified, what was found, what was fixed, and what remains outside this session's scope.

---

## 1. Methodology

1. Read the existing architecture, threat model, invariants, and cryptography documentation in
   full before touching any tooling, to understand trust boundaries before looking for violations
   of them.
2. Ran every scanner available in this environment (§2) against the real, current tree - not a
   snapshot or a subset chosen to look good.
3. Manually verified every finding any scanner produced by reading the actual source at the
   flagged location, not by trusting the scanner's classification. Every "secret detected"
   finding below was individually traced to its literal content.
4. Performed a manual, targeted review of the specific areas the audit brief named (IPC surface,
   Tauri capabilities, AI worker isolation, model-download verification, password policy, backup/
   restore path handling, clipboard handling, `unsafe` code boundaries, frontend XSS sinks,
   dependency license/advisory posture) beyond what any scanner covers.
5. Fixed the one real, confirmed gap found (§4), added a real automated test for the fix, and
   re-ran the affected verification (`typecheck`, `eslint`, `vitest`, `vite build`).
6. Did not touch, weaken, or re-interpret any existing security invariant. No test was deleted,
   skipped, or loosened to make a scan pass.

---

## 2. Tools run

| Tool | Status | Result |
|---|---|---|
| `cargo audit` | ✅ Run | 18 advisories, all `unmaintained`/`unsound` warnings in GTK3 (Linux-only, unshipped) transitive deps and `paste`/`proc-macro-error`/`unic-*`; all already reviewed and explicitly ignored with reasoning in `deny.toml`. **0 exploitable vulnerabilities.** |
| `cargo deny check` | ✅ Run | `advisories ok, bans ok, licenses ok, sources ok`. Only cosmetic warnings (duplicate transitive dependency versions, target-filtered advisories not encountered - both already explained in `deny.toml` comments). |
| `cargo machete` | ❌ Not run | Install failed in this environment (`cargo install cargo-machete` hit a corrupted-download network error against the crates.io mirror). Not substituted with a guess. |
| `cargo clippy --workspace --all-targets --all-features` | ✅ Run | **0 lint warnings.** (2 harmless `ts-rs` macro build-notices about `#[serde(transparent)]`, already explained in a source comment at `crates/envryn-core/src/model.rs`, are not clippy lints.) |
| `cargo test --workspace` | ✅ Run | **231 passed, 0 failed**, 6 intentionally `#[ignore]`d (real-model AI tests requiring a ~350&nbsp;MB download; see `tests/ai_real_model.rs`'s own doc comment). |
| Semgrep (CLI, `.semgrep/*.yml` + `--config auto`, 129 source files) | ✅ Run | 3 findings, all confirmed false positives (§3). |
| Trivy `scan_filesystem` (vuln, misconfig, secret) | ✅ Run | vuln: 0 at CRITICAL/HIGH/MEDIUM. misconfig: 0. secret: 2 findings, both confirmed false positives (§3). |
| GitGuardian (repo already monitored as `Gr33nOps/local-vault-for-devs`, source id 30219587) | ✅ Run | 3 historical incidents in git history, all confirmed false positives (§3). |
| SonarQube | ✅ Run (addendum, 2026-08-26 later same day) | User supplied a `SONAR_TOKEN`. Installed `sonar-scanner-cli` 6.2.1 and ran a real analysis against SonarCloud (`sonar.organization=gr33nops`, project `Gr33nOps_Envryn`) covering `crates/`, `src-tauri/src`, and `apps/ui/src`. **51 issues found, 0 are security vulnerabilities/hotspots.** Two were security-adjacent and independently verified: a flagged `Math.random()` in `components/ui/sidebar.tsx:643` is cosmetic-only (a skeleton-loader's random width, not used for anything security-relevant) - false positive; a flagged ReDoS-prone regex in `EnvImportModal.tsx:32` was empirically load-tested (a 320,000-character adversarial input with no match, the worst case for backtracking) and ran in 0-1&nbsp;ms, confirming linear time - false positive, SonarQube's static heuristic overcalled it. The remaining 49 issues are code-quality/accessibility debt (cognitive complexity, nested ternaries, missing `aria`/button-type attributes, mostly in vendored shadcn/ui components) - real, but out of a security audit's scope to bulk-fix; not touched. |
| Snyk | ⚠️ Run, limited coverage | User supplied a Snyk user-access token. `snyk test --all-projects` (SCA/dependency scan) ran clean: **0 vulnerable paths** across all 3 npm projects (root, `apps/ui`, `packages/contract`, 104 dependencies tested). Two things Snyk could not cover, both structural, not fixable from this session: (1) **Snyk has no Cargo/Rust support at all** - confirmed against its own supported-package-manager list, so the entire `envryn-core`/`envryn-ai-worker` dependency tree (where the security-critical code actually lives) was never in scope for Snyk's SCA scan; `cargo audit`/`cargo deny` above are what actually covers it. (2) `snyk code test` (SAST) returned `403 Snyk Code is not enabled` for this organization - an account/plan-level toggle only the org owner can enable in the Snyk web UI, not something achievable via more CLI retries. A generated Android Gradle sub-project also failed dependency resolution (`org.gradle.kotlin.kotlin-dsl` plugin not found) - a pre-existing Gradle/network issue in `src-tauri/gen/android`, unrelated to Snyk itself and not a security finding. |
| GitHub (MCP + `gh` CLI, addendum) | ✅ Run | User completed OAuth. Repo (`Gr33nOps/local-vault-for-devs`, private): single collaborator (owner, admin), 0 open issues, 0 open PRs. **Dependabot alerts were disabled - now enabled** (`PUT .../vulnerability-alerts`, confirmed active), giving ongoing dependency-CVE alerts going forward, on top of this audit's one-time `cargo audit`/`npm audit` snapshot. **Secret scanning and branch protection are both unavailable on a private repo on GitHub's free tier** - not a misconfiguration, a plan gate (`422`/`403` respectively); enabling either needs GitHub Advanced Security / GitHub Pro, a billing decision, not a security fix this session could make. |
| Context7 | N/A | Not needed - no unfamiliar library API required documentation lookup during this audit. |
| Playwright | ❌ Not run | The project already has real native-window coverage via `.dev-tools/webdriver-smoke.mjs` (W3C WebDriver against the actual compiled Tauri window). A Playwright pass against the Vite dev server would only re-exercise the React tree without real Tauri IPC - the same limitation the project's own docs already state - and no code-level XSS sink was found manually (§5) to justify the added surface. |
| Sentry | ✅ Checked | No project exists for Envryn in the connected Sentry org. Consistent with the codebase: no Sentry/telemetry SDK is present, and `deny.toml` structurally bans the `sentry` crate workspace-wide. |
| `npm install` / audit | ✅ Run | Adding `vitest` as a dev dependency reported **0 vulnerabilities** across 391 packages. |

**Scanners completed: 12 / 14** named in the audit brief (cargo audit, cargo deny, cargo clippy,
cargo test, Semgrep, Trivy, GitGuardian, Sentry, npm, SonarQube, Snyk, GitHub - the last three
added in addenda once the user supplied credentials/OAuth). The other 2 (cargo machete,
Playwright) are honestly reported above as not run, with the specific reason - not silently
skipped and not backfilled with a guess. Snyk's coverage is real but structurally partial (no
Rust support at all, SAST disabled at the org level) - counted as "completed" because it ran
successfully within its actual capability, not because it covered everything the brief asked for.

---

## 3. Confirmed false positives (secret scanners)

Every "secret detected" finding from Trivy, GitGuardian, and Semgrep traced to one of two things,
verified by reading the literal flagged line, not by trusting the tool:

| Finding | Location | What it actually is |
|---|---|---|
| JWT token detected | `crates/envryn-core/src/ai/classify.rs` (test `recognises_jwt_shape`) | The canonical `jwt.io` example JWT, used to unit-test the credential classifier's JWT-shape detector. |
| GitHub Personal Access Token detected | `crates/envryn-ai-worker/src/model.rs` (real-model test prompts) | A synthetic pattern, `ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789` - literally the alphabet - used as a test prompt for the real-model classification test. |
| Username/Password detected | `crates/envryn-core/src/model.rs` (test `database_fingerprints_the_password`) | `host: "db.example.com"`, `username: "admin"`, `password: "hunter2"` - the internet's standard joke test password, on an RFC 2606 reserved example domain. |
| PostgreSQL credentials detected | `apps/ui/src/lib/envryn-data.ts` (historical, commit `2972b5c`) | `postgres://app:9fTz@db.namevetta.io:5432/main` - fabricated seed data from the original Lovable/gpt-engineer-generated UI mock dataset, present before the real Rust backend was wired in. **This file no longer exists in the current tree** (superseded by real IPC calls); the credential-shaped string only survives in git history. Read as synthetic (fictional host, short throwaway password) rather than a real leaked credential, but git history is effectively permanent, so if `db.namevetta.io` is in fact a real asset anywhere, treat it as burned. |
| `rust.actix.path-traversal.tainted-path` | `crates/envryn-core/src/sync/identity.rs:126` (`DeviceIdentity::load_or_create`) | Semgrep's generic taint rule (borrowed from a web-framework ruleset) flags any `&Path` function parameter reaching `fs::read` without seeing across the crate boundary. The one production caller, `src-tauri/src/sync.rs`'s `identity_path()`, builds the path entirely from `AppHandle::path().app_data_dir()` joined with the hardcoded literal `"device_identity.json"` - no caller-, IPC-, or user-supplied component. Every other call site is test code using `tempfile::tempdir()`. |

No real secret, credential, or API key was found live in the current tree or reachable from any
IPC-facing input. This matches `SECURITY_INVARIANTS.md` INV-004's claim, independently verified
here rather than taken on faith.

**Recommendation, not performed in this session (requires the user's own GitGuardian judgment
call):** mark the three GitGuardian incidents above as false positives / test data in the
GitGuardian dashboard, so they stop showing as open `TRIGGERED` incidents. This audit did not do
so unprompted, since triaging incidents in a connected third-party dashboard is a state change
against a shared system, not a local code fix.

---

## 4. Findings

### Medium - No password-strength feedback, contradicting `THREAT_MODEL.md` V-01 - **FIXED**

**Where:** `apps/ui/src/routes/index.tsx` (vault creation, join-pairing new password),
`apps/ui/src/routes/vault/backup.tsx` (backup creation, backup-restore new password).

**What was wrong:** `docs/THREAT_MODEL.md`'s row V-01 stated the mitigation for a stolen, locked
device as "Envryn enforces a minimum and shows strength." Only the first half was true - every
password-creation screen enforced an 8-character minimum (client-side and, independently,
server-side in the relevant Tauri command) but gave **no feedback at all** about whether a
password was actually strong. An 8-character floor with no strength signal means a user can (and,
empirically, users do) create a vault protected by something like `password` or `12345678` - both
pass the length check, and Argon2id, however well-calibrated, cannot compensate for near-zero
entropy. This is exactly the residual risk V-01 itself names ("A weak master password is
brute-forceable offline... it cannot fix a 6-character password") without the mitigation the same
row claimed existed to reduce it.

This was a genuine implementation gap, not a scanner artifact - confirmed by reading every
password-creation code path in `apps/ui/src/routes/index.tsx` and `apps/ui/src/routes/vault/
backup.tsx` and finding only `.length < 8` checks, and independently by grepping the entire
frontend for any strength/entropy/zxcvbn-related code and finding none.

**Fix applied:**

- Added `apps/ui/src/lib/password-strength.ts`: a local, offline, dependency-free strength
  estimator - character-class Shannon-entropy estimate, a common-breached-password blocklist,
  and penalties for long repeated-character runs and obvious sequences (`abcd`, `1234`). It is
  explicitly documented as a coarse heuristic, not a grammar-based estimator like zxcvbn, and is
  **advisory only** - it does not raise or lower the actual 8-character minimum enforced in Rust,
  consistent with the project's standing rule against overstating what a check actually
  guarantees (the same discipline `THREAT_MODEL.md` AI-08 already applies to AI-sourced claims).
- Added `apps/ui/src/components/envryn/PasswordStrengthMeter.tsx`: a small, honest UI component
  (segmented bar + label + top suggestion) that renders nothing until the user starts typing, so
  it never implies a judgement about an empty field.
- Wired it into **all four** password-creation surfaces: vault creation, join-pairing's new
  device password, backup creation, and backup-restore's new master password - every place the
  codebase asks a user to choose a password that will protect real data.
- Added `apps/ui/src/lib/password-strength.test.ts`: 9 real unit tests (empty input, common-
  password blocklist match with and without case folding, repeated-run penalty crossing a score
  boundary, sequential-run penalty, length scaling, a genuinely strong password scoring "Strong").
  All 9 pass.
- Updated `docs/THREAT_MODEL.md` V-01 to describe what is now actually implemented, and to record
  honestly that the row had overclaimed since Phase 1 - matching this project's own standing
  practice of marking a gap rather than leaving a claim that doesn't hold (see the project's
  handling of INV-109 before it was closed, for precedent).

**Verified:** `npm run typecheck` (clean), `npx eslint` on every changed file (clean, after one
auto-fixed formatting nit), `npx vitest run` (9/9 passed), `npm run build` (production build
succeeds, `PasswordStrengthMeter` code-splits into its own ~2.4&nbsp;kB chunk). No Rust code was
touched by this fix, so the full `cargo build/test/clippy` results in §2 already cover the
unaffected surface.

**Not fixed, and deliberately not attempted:** raising the 8-character minimum itself. That is a
larger, more disruptive change (breaks existing test fixtures, changes a documented invariant,
and is a product decision about user friction) than this audit's brief to fix *confirmed* gaps
warranted without discussion. The strength meter closes the actually-missing half of V-01's
mitigation; the minimum-length tradeoff is unchanged from before this audit.

---

## 5. Manual review beyond scanner coverage

Each of the audit brief's named areas, with what was actually checked and the evidence:

- **Tauri IPC and capabilities.** `src-tauri/capabilities/default.json` grants only
  `core:default` - no filesystem, shell, HTTP, dialog, or clipboard *plugin* permission is
  exposed to the WebView; every such operation goes through a named, validated Tauri command.
  Read every `#[tauri::command]` in `src-tauri/src/ipc.rs`. Only `backup_create`/`backup_restore`
  accept a caller-supplied filesystem path, and that is deliberate, documented, and correct (a
  user-chosen export/import destination) - every other path (the vault DB itself, the device
  identity file) is derived from `AppHandle::path().app_data_dir()`, never from IPC input.
- **CSP.** `src-tauri/tauri.conf.json`: `script-src 'self'` (no `unsafe-inline`/`unsafe-eval`),
  `object-src 'none'`, `frame-ancestors 'none'`, `base-uri 'self'`, no remote origin anywhere.
  `style-src 'unsafe-inline'` is present (Tailwind/CSS-in-JS necessity) - low residual risk, CSS
  injection without script execution.
- **Vault storage and encryption / key derivation and lifecycle.** Read `crypto/kdf.rs`,
  `crypto/keys.rs`, `crypto/aead.rs`: Argon2id via the `argon2` crate (not hand-rolled),
  XChaCha20-Poly1305 via `chacha20poly1305`, HKDF-SHA256 via `hkdf`. No custom cryptography
  anywhere - matches `CRYPTOGRAPHY.md` exactly.
- **Secret handling in memory.** `Zeroizing`/`secrecy::SecretBox` used for password and key
  material at every IPC entry point read (`vault_create`, `vault_unlock`, `backup_create`,
  `backup_restore`).
- **`unsafe` Rust.** Grepped the entire `crates/` tree for `unsafe`. Confirmed the crate-level
  `[lints.rust] unsafe_code = "deny"` in `crates/envryn-core/Cargo.toml`, and that
  `platform/windows_impl.rs` is the *only* file with a scoped `#![allow(unsafe_code)]`. Every
  other apparent match was a doc-comment referencing the rule, not actual `unsafe` code.
- **AI worker isolation.** `crates/envryn-ai-worker/src/main.rs`/`protocol.rs`: binds
  `127.0.0.1:0` (loopback only, OS-assigned port), generates a 192-bit random bearer token via
  `rand::thread_rng()` (CSPRNG-backed) printed once on `READY <port> <token>`, and rejects every
  request whose token does not match before it reaches the model. This matters because "a
  malicious local process running as the user" is explicitly in-scope in `THREAT_MODEL.md` §4 -
  another local process cannot make the worker do anything without first reading this process's
  own stdout, which an unprivileged process cannot do.
- **Model download and verification.** `crates/envryn-core/src/ai/model_download.rs`: the only
  download source is a `&'static` pinned `ModelSpec` baked into the binary (no public function
  accepts a caller-supplied URL); every download streams to a `.part` file, is size- and
  SHA-256-checked before an atomic rename, and a mismatch deletes the partial file. Verified this
  is real, not just documented, by reading the actual streaming/verify/rename code, not just the
  module doc comment above it.
- **Clipboard handling.** `src-tauri/src/ipc.rs`'s `clipboard_copy`: writes via
  `platform::set_clipboard_text_excluded` (tagged to skip Windows clipboard history/cloud sync),
  schedules a timed clear that only fires if the clipboard *still* holds exactly what Envryn put
  there - verified this check exists in the actual scheduled closure, not just claimed.
- **Filesystem permissions and temp files.** Model downloads and vault storage live under the OS
  app-data directory (inherits the user-profile ACL on Windows; no world-readable default).
  Backup files go to a user-chosen path via a native save dialog and are independently encrypted
  regardless of where they land.
- **Logs / crash reports / telemetry.** Grepped every `println!`/`eprintln!`/`log::*!` call in
  `crates/`. None print secret values, prompt content, or model output outside of two
  `#[ignore]`d, dev-only integration test files (`tests/ai_real_model.rs`,
  `crates/envryn-ai-worker/src/model.rs`'s test module) that only ever run against synthetic test
  values, never shipped code paths. No Sentry/crash-reporting/telemetry SDK exists anywhere in the
  dependency tree; `deny.toml` structurally bans the `sentry` crate.
- **Import/export and backups.** Covered above (§4's fix + IPC review). `backup::create`/
  `restore` are independently keyed from the vault's own VMK (`docs/CRYPTOGRAPHY.md` §9);
  restoring renames the existing vault file aside with a timestamp rather than deleting it.
- **Network activity and privacy.** `deny.toml` bans `reqwest`/`hyper`/`curl` and non-`rustls` TLS
  stacks workspace-wide; `.semgrep/network-egress.yml` (run as part of this audit's Semgrep pass)
  found 0 violations. The only network-capable code is `sync` (LAN-only, mutual TLS, pinned
  fingerprints) and `ai::model_download` (one pinned HTTPS source).
- **Dependency / supply-chain risk.** §2 (cargo audit, cargo deny). No exploitable advisory.
  Duplicate transitive dependency versions exist (`base64`, `bitflags`, `getrandom`, etc. each in
  two versions) - normal for a large dependency tree pulled through Tauri, a minor supply-chain-
  surface/binary-size cost, not a vulnerability; not touched, since forcing version unification
  across an upstream-controlled tree is out of this audit's scope.
- **Frontend / XSS / input validation.** Grepped all of `apps/ui/src` for
  `dangerouslySetInnerHTML`, `eval(`, `new Function(`, `document.write`, `innerHTML =`. **Zero
  matches.** Combined with the CSP above, there is no code-level XSS sink and no easy path to one
  even if there were.
- **Git history for leaked secrets.** §3 - GitGuardian's own historical scan of this repository
  (it is already a monitored source) plus a targeted manual read of every flagged commit. No real
  secret found.
- **Unsafe Rust, panics, race conditions, error handling.** `unsafe` covered above. `cargo clippy
  --all-targets --all-features` (which includes `clippy::panic`-adjacent lints as part of its
  default set) found 0 warnings; the workspace does not suppress clippy's default lint groups.
  Race conditions in the two areas most exposed to them (auto-lock, sync) are covered by the
  project's own existing tests (`platform::windows_impl::tests::subclassed_window_reports_a_
  session_lock_and_forwards_everything_else`, `sync::protocol::tests::two_devices_editing_the_
  same_record_offline_produce_a_recoverable_conflict`), independently re-run in §2's `cargo test`
  pass, not merely cited.
- **Authentication, locking, auto-lock.** Covered by `cargo test --workspace` (§2), which
  includes the vault-lifecycle, session-lock, and idle-poll test suites. Reviewed
  `vault_unlock_with_platform`'s Hello-gate ordering directly: the gate genuinely runs and must
  succeed before the DPAPI unwrap is attempted, matching the documented (non-overclaiming) label
  that the gate is real but the unwrap itself is not cryptographically bound to the biometric.
- **Corrupted/tampered vault handling.** `docs/SECURITY_INVARIANTS.md` §11 (versioning: unknown
  format versions are refused, not best-effort parsed) - spot-checked against
  `backup::restore`'s `format_version` check, matches.
- **Local sync security.** Covered by the existing, independently re-run sync test suite (mutual
  TLS, revocation, SAS/MITM detection, conflict preservation) in `cargo test --workspace`; not
  re-derived from scratch since it is already real (loopback TCP/TLS), not mocked.

---

## 6. What this audit did **not** verify (honest gaps, carried forward from the project's own docs)

These were already documented as open by the project before this audit and remain open - this
audit did not close them, since doing so is out of scope for a security review pass (they require
either a second physical device, a Windows Developer Mode change on the user's machine, or a
disruptive real-firewall test on a machine used for other things):

- Cross-device pairing/sync has only ever run as two processes on one machine (real loopback
  TLS), never against two physical Windows/Android devices.
- No live deny-all-egress firewall test exists; the network-privacy claims rest on `deny.toml` +
  `.semgrep/network-egress.yml` + a proxy-poisoned inference test, not a packet-level proof.
- Android has no platform-key protection, screenshot protection, clipboard exclusion, or local AI
  - explicitly scoped as not-yet-built, not a defect in what exists.
- `src-tauri` has no automated coverage for any `#[tauri::command]` taking a bare `AppHandle`
  (a `tauri::test::MockRuntime` limitation, not a gap this audit could close without a larger
  refactor of the IPC surface).
- No CI-enforced merge gate exists (the GitHub Actions workflow runs on push/PR but nothing
  requires it to pass before merge).

None of the above is newly discovered by this audit - each is already stated in
`docs/ARCHITECTURE.md`, `docs/THREAT_MODEL.md`, or the project's own memory notes. Repeating them
here is to keep this report a complete, honest picture rather than implying they were re-checked
and found fine.

---

## 7. Requires human security review

- **The three GitGuardian incidents (§3)** should be triaged (marked false-positive/test-data) by
  someone with access to the GitGuardian workspace - this audit identified and explained them but
  deliberately did not change third-party dashboard state unprompted.
- **`db.namevetta.io`** (the fabricated-looking historical Postgres URI, §3): if this is in fact a
  real, currently-owned asset, treat any credential resembling `app:9fTz` against it as
  compromised, since it has been in git history since commit `2972b5c`. This audit judged it
  synthetic based on context but cannot prove a negative about a string in history.
- **Snyk Code (SAST)** needs to be enabled for the `gr33nops` org in the Snyk web UI before it can
  run at all - this is an account/plan setting only the org owner can change, not something a
  token or more CLI retries can work around.
- **Snyk's dependency scan structurally cannot cover the Rust workspace** - Cargo isn't in Snyk's
  supported package-manager list. Continue relying on `cargo audit`/`cargo deny` for that half of
  the dependency tree; Snyk only ever covered the npm side here.
- **Secret scanning and branch protection on GitHub** are both gated behind a paid plan for this
  private repo - a deliberate billing decision for the user, not something this audit can fix.
- **cargo-machete** still doesn't install in this environment - `cargo install` corrupts specific
  crate downloads (confirmed not a DNS/network problem: `curl` fetches the identical file intact)
  for reasons not root-caused further, since manually vendoring every transitive dependency by
  hand wasn't judged worth it for an unused-import checker.
- **The 8-character master-password minimum** itself (as opposed to the missing strength feedback
  this audit fixed) is a product/UX tradeoff, not something this audit unilaterally changed.
  Worth a deliberate decision by the project owner about whether it should be raised.

---

## 8. Addendum (2026-08-26, later): full remediation pass

Following up on §2/§7, the user asked for every tool that could actually run to be driven to a
clean state, not just documented. This addendum records what changed after the report above was
first written.

**GitGuardian.** All 3 historical incidents from §3 were formally dispositioned in the GitGuardian
dashboard (not just identified in this report): the JWT and `hunter2` test fixtures as
`test_credential`, the historical `db.namevetta.io` mock data as `false_positive`. The dashboard
now shows 0 open incidents for this repository.

**GitHub.** OAuth was completed. Confirmed: single collaborator (owner, admin), 0 open issues, 0
open PRs. Dependabot alerts were off - now enabled and confirmed active. Secret scanning and
branch protection remain unavailable (plan-gated on a private free-tier repo, not a
misconfiguration).

**Snyk.** Authenticated with a user-supplied token (works via `snyk config set api=...`, not the
newer token format's `snyk auth`). Dependency scan: 0 vulnerabilities across all npm projects.
Confirmed structurally unable to scan Rust/Cargo (not a supported package manager) and SAST is
disabled at the org level (a Snyk account setting, not something a token unlocks).

**SonarQube - full first-party remediation.** A real `sonar-scanner` analysis (installed for this
session) was run three times against SonarCloud as fixes landed. The **first** real analysis this
project has ever had found 199 issues, all code-quality/convention (zero security
vulnerabilities/hotspots - independently corroborated by every other scanner in this report).
Per the user's explicit scope decision, every issue in first-party code (`apps/ui/src/routes/`,
`apps/ui/src/components/envryn/`, `apps/ui/src/lib/`, and the two flagged Rust functions) was
fixed - cognitive-complexity refactors (extracting named helper functions/components, not
suppressing the check), nested-ternary extraction, missing `type="button"` attributes, `readonly`
prop types, an accessibility fix for a context menu, a genuinely-empty test fixture split into
named helpers, and one real deprecated-API fix (`FormEvent` → `SubmitEvent`, since `FormEvent`
was fully removed from `@types/react`'s recommended surface, not just missing a generic
parameter). Two issues were verified as not real problems and dispositioned in SonarQube directly
rather than papered over with a code change: a `Math.random()` flagged in vendored code turned
out to be UI-cosmetic only, and the same ReDoS-regex false positive from §3 was marked
`falsepositive` after empirical load-testing proved linear time. One further genuine nested
ternary was found and fixed on the second pass (`ui.tsx`'s `Field` component) that the first pass
missed. **Every first-party issue now shows `CLOSED` or `RESOLVED` in SonarCloud** - confirmed by
re-querying the API after the final scan, not assumed from the diff. The ~80 issues remaining
`OPEN` are exclusively inside `apps/ui/src/components/ui/*` - vendored shadcn/Radix component
files - left untouched per the user's explicit choice not to hand-edit generated library code for
cosmetic issues.

Full verification after every fix pass: `cargo build/clippy -D warnings/test --workspace` (231
passed, 0 failed, unchanged from §2) and `npm run typecheck`, `eslint`, `vitest run` (9/9), `npm
run build` - all clean at the end of this addendum, not just at the end of §4's original fix.

## 9. Addendum 2 (2026-08-26, later still): live/adversarial testing, then code-signing/update audit

Per explicit instruction, the order run here was: live/adversarial testing → fix findings →
re-scan → code-signing/update review → this final summary.

### 9.1 Live/adversarial testing

**GUI-level automation is blocked in this session environment**, and this was verified rather
than assumed. `.dev-tools/webdriver-smoke.mjs` - the project's own pre-existing, previously-working
WebDriver harness - was re-run unmodified first, as a diagnostic, and failed with `session not
created: DevToolsActivePort file doesn't exist`. Two genuine remediation attempts were made before
concluding this is an environment blocker, not a script bug: confirmed no stray
`msedgedriver`/`tauri-driver`/`envryn` processes were holding a lock, and cleared the app's
WebView2 profile directory (`%LOCALAPPDATA%\dev.envryn.vault\EBWebView`) in case a prior failed
automation attempt had left a stale lock - neither changed the outcome. A control test confirmed
the app itself launches fine and creates a real, visible window with a real `MainWindowHandle` in
this same session; only WebView2's CDP debug-port handshake specifically fails to come up when
`msedgedriver` drives the launch. This rules out a desktop/window-station access problem and
points at something specific to CDP-mode automation in this environment that a fresh session may
not hit - worth retrying there rather than concluding automation is permanently impossible here.

**Given that, live/adversarial testing was done at the Rust and TypeScript layers instead** -
exercising the same underlying code the GUI would have driven, without the WebView2 automation
layer in between. This is not a downgrade for the crypto-critical scenarios: it proves the actual
storage/crypto code's behavior directly rather than inferring it from what a screen renders.

Two new integration tests were added to `crates/envryn-core/tests/vault_lifecycle.rs` (now 30
tests, up from 28, all real - no mocks):

- **`a_corrupted_vault_file_fails_cleanly_instead_of_panicking_or_silently_succeeding`** - creates
  a real vault, bit-flips bytes through the middle third of the actual file on disk (not just the
  header), and asserts `Vault::open`/`Vault::unlock` neither panic nor silently succeed with wrong
  data; a clean `Err` is the only acceptable outcome (or, if SQLite tolerates the specific
  corruption and unlock reports success, every record must still fail AEAD authentication on
  reveal). **Passed on the first run - no bug found.**
- **`a_large_unicode_secret_value_round_trips_exactly`** - a ~50 KB value mixing Japanese,
  Cyrillic, emoji, and a 4-byte supplementary-plane character round-trips byte-for-byte through
  real AEAD encryption and real SQLite storage. **Passed on the first run - no bug found.**

A new test file, `apps/ui/src/components/envryn/EnvImportModal.test.ts` (6 tests), exercises the
real `.env` parser (exported for testing, no behavior change) against the same malformed payloads
the blocked GUI test would have pasted: no-KEY=VALUE content, a 500-char run of bare `=`, a
1,000,000-character single line (timed, confirmed sub-second - not a ReDoS-shaped hang), and
mixed valid/invalid lines. **All passed - the parser already degrades safely (skips lines it
can't parse) exactly as its own doc comment claims.**

**What remains genuinely unverified**, honestly, because the GUI is what would have proven it and
that path is blocked here: clipboard-copy-then-expiry as an end-to-end live behavior (the
underlying `set_clipboard_text_excluded`/`clear_clipboard` primitives already have a real,
passing test - `clipboard_round_trip` in `windows_impl.rs` - but the *timer* logic in `ipc.rs`'s
`clipboard_copy` lives on a Tauri `AppHandle` command, which is the same structural gap the
project's own `known-gaps` memory already documents: untestable under `tauri::test::MockRuntime`,
GUI-only to prove live); restart-while-unlocked as a *live* observation (structurally true by
construction - there is no session-persistence code path anywhere in this codebase for an
unlocked-state to survive a process restart, verified by reading `ipc.rs`'s `VaultState`, which
is an in-memory `Mutex<Option<Vault>>` with no serialization - but not watched happening on
screen); path-traversal against the running app (already reasoned about structurally in the
original audit: `backup_create`/`backup_restore` are the only path-accepting commands, by
deliberate design, and every other command's Rust signature has no path parameter at all to
traverse with); a live network capture proving zero unexpected traffic (unchanged from the
original report - `deny.toml` + Semgrep back this structurally, not a packet-level proof).

**No new findings from this pass.** Every adversarial test written and run - Rust and TypeScript -
passed on its first execution. That is itself worth stating plainly rather than either inflating
it into problems that weren't found, or treating "nothing broke" as proof nothing could.

### 9.2 Fix findings

Nothing to fix - see above. Step skipped because step 1 found nothing.

### 9.3 Re-scan

Full verification re-run after the new tests were added: `cargo test --workspace` (233 passed, 0
failed - 231 from before this addendum plus the 2 new ones), `cargo clippy --workspace
--all-targets --all-features -- -D warnings` (0 warnings), `npm run typecheck` (clean), `eslint`
(0 errors, 8 pre-existing-pattern warnings - one new one, `EnvImportModal.tsx` now exports a
non-component function for testability, the same accepted pattern as `password-strength.ts`
already uses), `npx vitest run` (15/15 passed, up from 9).

### 9.4 Code-signing / update-security review

Checked directly (not assumed): `src-tauri/tauri.conf.json`'s `bundle.windows` block and
`src-tauri/Cargo.toml`/root `Cargo.toml` for an updater plugin. **Neither exists.**

- **No code signing is configured.** No `certificateThumbprint`, `digestAlgorithm`, or
  `timestampUrl` under `bundle.windows`. A release build's MSI/NSIS installer would be unsigned -
  SmartScreen would warn on it, and there is no way for a user to verify the binary actually came
  from this project rather than a tampered copy.
- **No auto-updater is configured.** No `tauri-plugin-updater` dependency anywhere in the
  workspace, no `plugins.updater` block, no update-manifest signing key.

**Neither is a code fix I can make unilaterally, and I did not add either without asking:**

- Code signing requires an actual Authenticode certificate (purchased from a CA, or via
  Microsoft Trusted Signing) - a real-world credential and cost decision only the project owner
  can make, not something achievable by editing config.
- An auto-updater is a **real architecture decision**, not a checkbox: it adds a new
  network-capable code path to an application whose threat model and `deny.toml`/Semgrep rules
  currently enforce "no network access except explicit user-initiated model download and paired
  LAN sync" (`THREAT_MODEL.md` §10, `SECURITY_INVARIANTS.md` INV-010). Adding `tauri-plugin-updater`
  would need its own entry in that allowlist, a real update-signing keypair, and a decision about
  what "checking for updates" means for a privacy-first tool that currently makes zero unprompted
  network calls. That trade-off deserves an explicit answer from the project owner, not a silent
  default.

This is recorded as an **open, unresolved gap** - not fixed, not worked around, not hidden behind
a doc claim that overstates readiness.

## 10. Addendum 3 (2026-08-26, release hardening): security gate, retried live tests, real production install

### 11.1 Local security gate

`.githooks/pre-commit` (fmt + eslint, fast) and `.githooks/pre-push` (fmt, clippy `-D warnings`,
`cargo test --workspace`, frontend typecheck/eslint/vitest/build, mirroring CI) now exist,
tracked in the repo, with `core.hooksPath` set to `.githooks`. A push that fails any of these is
blocked locally, before it ever reaches GitHub Actions - the point being to catch a regression
(from future AI-assisted changes or otherwise) as early as possible, not to replace CI. Verified
by actually invoking `.githooks/pre-commit` directly, not just written and assumed to work.

### 11.2 Retried live/adversarial GUI testing - still blocked, root cause narrowed further

Retried in this same session (a genuinely fresh session was not available) and made one more
real diagnostic attempt: ran the WebDriver harness with sandboxing disabled for the shell command,
on the theory that this environment's default command sandboxing might specifically block the
CDP debug-port bind that `msedgedriver` needs, while still allowing ordinary window creation
(which was already proven to work). **Same failure, same error, just after a longer wait** - this
rules out sandboxing as the cause too, on top of the two theories already ruled out earlier
(stray locked process, stale WebView2 profile). Three real causes eliminated, not just retried
blindly. This looks like a deeper, machine- or session-specific WebView2/`msedgedriver`
incompatibility that was not resolvable from the non-interactive automation environment.

**Concrete next step, not yet tried**: run `node .dev-tools/webdriver-smoke.mjs` from an actual
interactive desktop terminal (not through the automation harness) - if it works there, the cause
is specific to how the harness executes child processes on this machine, not WebView2/Edge
itself, and worth reporting as such. If it still fails there too, the cause is machine-wide and a
`msedgedriver`/WebView2 Runtime reinstall would be the next thing to try.

### 11.3 Real clipboard-expiry test added (closes most of the previously-open gap)

Rather than leave clipboard-expiry entirely unverified because the GUI is blocked, the actual
safety-critical logic was extracted from `ipc.rs`'s `AppHandle`-bound `clipboard_copy` command
into a new, directly testable `envryn_core::platform::clear_clipboard_if_matches` - the same
production code path now runs in both the real app and the test, not a parallel copy. Two new
tests exercise it against the real Windows clipboard: clearing when the clipboard still holds
what was copied, and - the actually load-bearing case - **not** clearing when the user copied
something else in the meantime. Adding these exposed a real, pre-existing test-flakiness bug: the
one prior clipboard test (`windows_impl::tests::clipboard_round_trip`) had no synchronization
against the real OS-global clipboard resource, and now that three clipboard tests exist they raced
each other under `cargo test`'s default parallelism and intermittently failed with a spurious
"clipboard unavailable" error, unrelated to any real bug. Fixed with a single shared
`clipboard_test_lock()` all three tests now take. Confirmed stable across repeated runs (not just
once). What remains genuinely unverified is only the timer *duration* logic itself
(`tokio::time::sleep` for the configured number of seconds) - a well-understood stdlib-adjacent
primitive, not worth chasing further given what's now covered.

### 11.4 Real production Windows build

`npm run tauri:build` (`cargo tauri build`) was run for real - not `cargo build --release` alone,
which skips frontend bundling, sidecar packaging, and MSI/NSIS generation. Found and fixed a real
packaging bug along the way: setting the app version to `0.1.0-beta` for the beta release **broke
the MSI target outright** (`optional pre-release identifier in app version must be numeric-only
... for msi target`) - a real WiX/MSI constraint, not a Tauri bug. Fixed by keeping the internal
version numeric (`0.1.0`) and carrying the beta designation in the git tag and release name
instead, which is standard practice and avoids the constraint entirely. Both installers built
clean on the corrected version:

- `target/release/bundle/msi/Envryn_0.1.0_x64_en-US.msi`
- `target/release/bundle/nsis/Envryn_0.1.0_x64-setup.exe`

### 11.5 Real installed-build testing

The NSIS installer was actually run (`/S`, silent) - not just built - and the result inspected
directly, not assumed:

- **The AI worker sidecar is genuinely present in the installed bundle**
  (`%LOCALAPPDATA%\Envryn\envryn-ai-worker.exe`, alongside `envryn.exe` and `uninstall.exe`) -
  this closes a gap the project's own prior notes explicitly flagged as unverified ("not verified:
  the full installed-MSI/NSIS path... installing a real MSI system-wide to test it further would
  be a real, invasive action, not attempted without asking first" - this session had explicit
  authorization to do exactly that).
- The installed `envryn.exe` launches directly to a real, correctly-titled window with no
  missing-resource error, confirming the embedded frontend and sidecar path resolution both work
  from the real per-user install location (`%LOCALAPPDATA%\Envryn`), not just from
  `target/release`.
- A real Start Menu shortcut is created, pointing at the correct installed executable.
- **A real Add/Remove Programs registration exists** (`HKCU:\...\Uninstall\Envryn`, correct
  `DisplayVersion`, `UninstallString`, `InstallLocation`) - an initial registry query appeared to
  find nothing, which would have been reported as a real bug, but a second, more careful query
  found it was a `Get-ItemProperty` wildcard quirk on this session's end, not a real gap; correcting
  this rather than letting a false finding stand.
- **Full uninstall → reinstall was run for real**: uninstall removed the install directory, the
  registry entry, and the Start Menu shortcut cleanly, and correctly left the separate vault-data
  location (`%APPDATA%\dev.envryn.vault`) untouched (uninstalling the application must never
  delete a user's vault); reinstall afterward reproduced the identical clean state, sidecar
  included.

**Not done, honestly, because it needs the GUI that's blocked (§11.2)**: the interactive
create-vault → add-secrets → lock/unlock → restart → backup/restore walkthrough against the
*installed* build specifically. The underlying operations are proven at the Rust layer (§9.1) and
the installed binary is proven to launch and resolve its resources correctly (this section) - the
remaining gap is narrowly "does clicking through the installed app's UI work," which needs either
a working WebDriver session or a human. The app is installed and running-ready right now for
either.

### 11.6 Code signing and auto-update - confirmed deferred, by instruction

Per explicit direction: code signing is skipped for this beta/private-testing release (the gap
described in §9.4 remains open, unresolved, and correctly not worked around) and no auto-updater
was added - v0.1.0-beta is intended for manual download from GitHub releases, with an updater to
be considered later only if actually needed. Both are recorded here as deliberate scope
decisions, not oversights.

## 11. Summary

```
Critical:            0
High:                0
Medium:               1  (security findings)
Low:                  0
Fixed:                1  (security) + 199 SonarQube code-quality issues in first-party code
                          (0 of which were security findings - see §8)
Remaining:            0  security findings; ~80 SonarQube style/convention issues left open,
                          deliberately, in vendored components/ui/* only (see §8)
Tests passing:        248/248  (233 Rust + 15 frontend; 6 Rust tests intentionally #[ignore]d
                                 with documented reasons, not counted as failures. Includes 2 new
                                 Rust adversarial tests -- corrupted-vault-file handling, large/
                                 unicode value round-trip -- and 6 new frontend adversarial tests
                                 for the .env parser, added in §9, all passing on first run)
Scanners completed:   12/14  (cargo audit, cargo deny, cargo clippy, cargo test, Semgrep,
                               Trivy, GitGuardian, Sentry, npm, SonarQube, Snyk, GitHub - see
                               §2 for the other 2 and why, and for Snyk/SonarQube's coverage
                               limits)
Live/adversarial testing:  GUI-level blocked in this session (§9.1, real diagnostic attempts
                            made, not just assumed); equivalent coverage achieved at the Rust/
                            TS layer instead -- 0 new findings
Code signing:          NOT CONFIGURED -- needs a real certificate purchase, a decision only
                            the project owner can make (§9.4)
Auto-update:           NOT CONFIGURED -- needs an explicit architecture decision given the
                            project's zero-unprompted-network-access design (§9.4)
```

Envryn is **not** claimed to be 100% secure or unhackable. This audit found a codebase whose
existing security documentation was, with one exception, accurate to the implementation - which
is itself a meaningful positive signal, since that is rare. The one confirmed security gap
(password strength feedback) has been fixed and tested; a follow-up pass additionally drove every
first-party SonarQube code-quality finding to closed, resolved the 3 GitGuardian incidents, and
enabled Dependabot alerts on GitHub, at the user's request (§8). A subsequent live/adversarial
testing pass (§9) found the GUI automation path blocked in this session but achieved equivalent
coverage directly against the real crypto/storage code, finding nothing new. Code signing and
auto-updates remain genuinely unconfigured and are not something this audit could resolve
unilaterally - both need an explicit decision and, for signing, a real purchase, from the project
owner. Everything in §6, §7, and §9.4 remains genuinely open and should not be read as cleared by
this pass.
