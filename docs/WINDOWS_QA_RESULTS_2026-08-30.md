# Envryn Windows QA Results

Date: 2026-08-30  
Version: 0.1.9  
Workspace: current uncommitted working tree  
Plan: [Envryn Windows Manual Test Plan](MANUAL_TEST_PLAN_WINDOWS.md)

## Outcome

The current Windows source and rebuilt packages pass the automated desktop, Rust core, native IPC, real Local AI, security, accessibility, documentation, build, and launch checks that can be executed safely on this PC.

This is not a claim that all 292 manual cases passed. Every case that could be exercised through the disposable production UI boundary, native core, real local model, or a clean Windows Sandbox was tested. Several cases still require a human to operate the live secret vault, a second Windows PC, interactive Windows Hello, Windows session transitions, multiple displays, or assistive technology. Those cases remain explicitly blocked or human-only below.

Release readiness is currently blocked by two items:

1. The Windows executable, MSI, and setup EXE are not code-signed.
2. Real two-PC pairing and sync have not been run on two separate Windows installations.

## Changes made while testing

| Area                | Finding                                                                           | Fix                                                                                                | Verification                                                                            |
| ------------------- | --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Projects            | Strict Rust lint rejected two project sorting calls.                              | Replaced the unnecessary comparator with key-based sorting.                                        | Clippy with warnings denied and all 34 vault lifecycle tests pass.                      |
| Forms               | Visual field labels were not programmatically associated with inputs and selects. | Added shared label context for Envryn inputs and selects, plus an explicit Secure Note body label. | Desktop CRUD E2E can address fields by accessible name. TypeScript and axe checks pass. |
| Secret list         | A row acted as a button while containing three real buttons.                      | Replaced the nested interactive structure with a dedicated accessible row-selection button.        | The axe nested-interactive finding is gone and row actions still pass tests.            |
| Contrast            | Secret metadata measured 4.47:1 against a 4.5:1 WCAG AA minimum.                  | Increased the subtle foreground token slightly.                                                    | The complete vault axe audit has no serious or critical violations.                     |
| Regression coverage | Desktop browser coverage stopped at basic onboarding and one Database flow.         | Expanded the disposable native-boundary suite across projects, every secret type, settings, backup, devices, pairing UI, sync, conflicts, import, Local AI, shortcuts, lock/unlock, large vaults, and display modes. | All 19 Windows desktop flows pass together.                                             |
| Release version      | Settings displayed 0.1.8 while the application and installers were version 0.1.9.  | Made the UI read the version from `src-tauri/tauri.conf.json` at build time.                        | Settings and rebuilt 0.1.9 packages display the same version.                          |
| AI provider fields   | AI could identify an uncommon provider but leave the editable Provider field blank. | Applied provider suggestions from both deterministic and Local AI classification.                  | The uncommon-provider desktop flow fills IGDB as both the suggested name and provider. |
| Mobile accessibility | The compact search icon had no accessible name after its visible text was hidden. | Added an explicit accessible name to the vault search button.                                      | The complete mobile vault axe audit has no serious or critical violations.             |
| Offline installer    | Both installers failed on a clean PC with networking disabled because setup tried to download WebView2. | Changed the Tauri WebView install mode to embed the offline runtime. | Final MSI and NSIS install, launch, reinstall, and uninstall successfully in a network-disabled Windows Sandbox. |
| Large-vault search   | Exact-name search allocated and scored fuzzy candidates before checking exact names. | Added an allocation-light exact-name pass before fuzzy ranking. | A dedicated 1,000-secret, 50-project run meets the 3-second unlock and 300-ms exact-search budgets. |
| Forced colors        | Windows forced-colors emulation combined custom dark tokens with system white surfaces. | Mapped semantic tokens to Windows system colors and disabled nonessential motion when reduced motion is requested. | The resolution, forced-colors, reduced-motion, and WCAG AA E2E check passes. |

## Executed checks

### Desktop and TypeScript

| Check                       | Result             | Evidence                                                                                                                |
| --------------------------- | ------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| UI unit and component tests | PASS               | 92 of 92 tests passed in 14 files.                                                                                      |
| Desktop browser E2E         | PASS               | 19 of 19 Windows desktop flows passed.                                                                                  |
| Cross-viewport browser E2E  | PASS               | 23 passed; 15 Windows-only cases were intentionally skipped on the Android viewport.                                   |
| Desktop screenshot render   | PASS               | Current 1440 by 900 vault rendered with no runtime errors.                                                              |
| TypeScript                  | PASS               | `tsc --noEmit` completed successfully.                                                                                  |
| ESLint                      | PASS WITH WARNINGS | 0 errors and 8 existing Fast Refresh organization warnings.                                                             |
| Production web build        | PASS               | 1,977 modules transformed successfully.                                                                                 |
| Bundle budget               | PASS               | Largest JavaScript chunk 324.3 KiB, total JavaScript 650.0 KiB, total CSS 103.3 KiB.                                    |
| UI line coverage            | NEEDS IMPROVEMENT  | 25.74 percent of lines. Secret Form, Secret Panel, Backup, Devices, Sync, and Settings need more automated UI coverage. |

### Rust, native IPC, and storage

| Check                                  | Result | Evidence                                                                        |
| -------------------------------------- | ------ | ------------------------------------------------------------------------------- |
| Rust workspace tests                   | PASS   | 278 non-ignored tests passed.                                                   |
| Vault lifecycle                        | PASS   | 34 of 34 real-file vault tests passed.                                          |
| Native IPC                             | PASS   | 6 of 6 real dispatch tests passed.                                              |
| Core library                           | PASS   | 213 of 213 non-ignored tests passed.                                            |
| AI worker unit tests                   | PASS   | 12 of 12 non-ignored tests passed.                                              |
| Prompt encapsulation compile-fail test | PASS   | Unsafe prompt construction remains inaccessible outside the gateway.            |
| Rust formatting                        | PASS   | `cargo fmt --check` completed successfully.                                     |
| Strict Clippy                          | PASS   | `cargo clippy --workspace --all-targets -- -D warnings` completed successfully. |

### Real Local AI

The installed 1.5B GGUF model, tokenizer, and native worker were used. These were not mock responses.

| Check                     | Result  | Evidence                                                                                                                           |
| ------------------------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Full real-model suite     | PASS    | 10 of 10 tests passed in 340.65 seconds.                                                                                           |
| Offline inference         | PASS    | Classification passed while HTTP, HTTPS, and all proxy variables in the worker were forced to an unreachable local address.        |
| Worker failure recovery   | PASS    | Killing the worker produced a recoverable `EngineUnavailable` error rather than a crash.                                           |
| Semantic search           | PASS    | Production database intent produced the expected structured filter.                                                                |
| Structured extraction     | PASS    | Host, port, username, and password fields were extracted from fake labeled data.                                                   |
| Name suggestions          | PASS    | OpenRouter and Stripe produced usable editable names.                                                                              |
| Legacy 0.5B compatibility | WARNING | 8 of 10 passed. Two generic API key classification cases were incorrectly labeled Database. The production 1.5B model passed both. |

The raw 1.5B model labeled `GITHUB_TOKEN` as Custom in one direct model output. The product's deterministic name classifier resolves that name as Token before the model is consulted, so the end-user path remains correct. This illustrates why deterministic classification must remain ahead of AI.

### Security and dependencies

| Check                      | Result               | Evidence                                                                                                                                                                                         |
| -------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Security invariant checker | PASS                 | All repository security invariants verified.                                                                                                                                                     |
| Secret scan                | PASS                 | 120 commits and about 2.71 MB scanned with no leak found.                                                                                                                                        |
| Semgrep                    | PASS                 | 5 custom rules across 51 Rust files produced 0 findings.                                                                                                                                         |
| npm production audit       | PASS                 | 0 vulnerabilities.                                                                                                                                                                               |
| Rust advisory gate         | PASS WITH ALLOWLIST  | No reachable issue for the shipped Windows target. Existing warnings are documented transitive or unshipped Linux dependencies.                                                                  |
| Encrypted storage tests    | PASS                 | Plaintext, secret metadata, trusted-device names, and backup canary values are absent from storage.                                                                                              |
| Clipboard core behavior    | PASS                 | Timed clear and preservation of newer clipboard content both pass native tests.                                                                                                                  |
| Screen capture protection  | PASS FOR CAPTURE API | The installed Envryn window rejects Windows capture with `SetIsBorderRequired failed`, consistent with active display-affinity protection. Other capture tools still require human verification. |

### Windows packages

| Artifact                     | Size       | Result                                                        | SHA-256                                                            |
| ---------------------------- | ---------- | ------------------------------------------------------------- | ------------------------------------------------------------------ |
| `envryn.exe`                 | 8.79 MiB   | Built, launches, responsive, NOT SIGNED                       | `AF49B4210FB0AC0714787DF433C6CB2720632537F439816907432F36853C945A` |
| `Envryn_0.1.9_x64_en-US.msi` | 254.18 MiB | Offline install lifecycle passed in clean Sandbox, NOT SIGNED | `9D839243131988EDADF6230B572D8D19CB72C5722FBC4596F6A7D908E79358B2` |
| `Envryn_0.1.9_x64-setup.exe` | 254.36 MiB | Offline install lifecycle passed in clean Sandbox, NOT SIGNED | `6EA406056436854418535842D1349519805F0105EA5B198F8AC8AAA2828F1C6C` |

The final clean-machine report is stored at `.qa/windows-sandbox/output/sandbox-results.json`. Windows 10 Enterprise build 19041 had networking disabled. Both installers returned exit code 0, created a Start menu shortcut, installed version 0.1.9, opened a responsive titled window at about 22 to 23 MB working memory, supported two responsive launches, reinstalled with exit code 0, preserved a disposable app-data sentinel, uninstalled with exit code 0, and removed the executable. The first Sandbox run is retained as failure evidence and shows the pre-fix WebView download failure.

## Manual plan coverage by section

### Release smoke test

Status: PARTIAL

Build, launch, project creation and rename, all nine secret types, edit persistence, search, settings, backup UI and core behavior, lock and unlock, Local AI, device management, pairing UI, sync status, retry, and both conflict choices are verified. Windows session lock and real two-PC sync remain human-only.

### Installation, launch, and removal

Status: PASS FOR CLEAN WINDOWS 10 SANDBOX SCOPE

Fresh MSI and NSIS installation, normal launch, responsive window, Start menu shortcut, version, double launch, same-version reinstall, retained app data, uninstall, and program-file removal pass in a clean network-disabled Windows Sandbox. Upgrade from a previous public version, setup cancellation, post-restart launch, icon sharpness in every Windows shell location, and Windows 11 still require interactive validation.

### First run, vault creation, and unlock

Status: PARTIAL

First-run validation and creation pass in desktop E2E. Real vault create, persist, reopen, correct password, wrong password, lock denial, corrupted file, Unicode data, and offline core operation pass Rust lifecycle tests. Live unlock interaction on the user's real secret vault remains human-only.

### Windows account unlock and password changes

Status: PARTIAL

DPAPI round trips, tamper rejection, platform protection enable and disable, wrong-password rejection, password change, and data preservation pass. Interactive Windows Hello enrollment and the final live change-password submission remain human-only.

### Window controls, navigation, and shortcuts

Status: PARTIAL

The locked native window exposes Minimize, Maximize, Close, and accessible unlock controls. Title bar and resize unit tests pass. Desktop navigation plus `Ctrl+K` pass E2E. Snap, multiple monitors, prolonged use, and close behavior with unsaved live forms require human operation.

### Secret creation, types, editing, and deletion

Status: PASS FOR DISPOSABLE UI AND CORE SCOPE

All nine payload types are created through the production desktop forms with fake values. Multi-field storage, create, reveal, update, delete, restart persistence, field mapping, category visibility, and data validation pass. The Database flow also covers full edit and reopen persistence.

### Lists, categories, filters, sorting, and icons

Status: PARTIAL

Category counts, Database separation, search filtering, icon mapping, Imported-tag suppression, accessible list selection, and the rendered desktop layout pass tests. A 1,000-record visual scroll and every uncommon-provider icon still require manual observation.

### Projects

Status: PASS FOR AUTOMATED SCOPE

Project creation opens the new project, empty projects persist without placeholder secrets, duplicates are rejected case-insensitively, names are encrypted, stable IDs survive rename, and project IPC round trips pass.

### Search and command palette

Status: PASS FOR AUTOMATED SCOPE

Local fuzzy search, metadata matching without secret-value matching, keyboard palette opening, deterministic intent parsing, AI fallback, uncommon names, malformed output recovery, and live 1.5B search pass. The dedicated 1,000-record and 50-project test measured 1,463.7 ms from vault creation to a usable list, 169.9 ms for an exact-name result, and 103 ms to open a populated project. It meets the plan's 3-second, 300-ms, and 1-second budgets. The same scenario also passes a six-browser overload run with separate 4-second and 500-ms ceilings.

### `.env` import

Status: STRONG PARTIAL

Parser, deterministic classification, review-state, type override, row exclusion, import save, category placement, and Imported-tag suppression pass through the production desktop UI with fake data. A 100-row installed-UI import and forced partial write failure remain human-only or require a dedicated fault-injection test.

### Local AI and structured extraction

Status: PASS FOR AVAILABLE MODEL

All real 1.5B model cases pass, including offline execution and worker failure recovery. Download interruption and visual progress require a disposable model directory and live UI operation.

### Backup and restore

Status: PASS FOR DISPOSABLE UI AND CORE SCOPE

Desktop backup validation, write failure, success, wrong restore password, and successful restore navigation pass with a disposable native boundary. Encrypted backup round trip, plaintext absence, garbage, truncated input, tampered header, future version rejection, and restore between two real temporary vaults also pass. Disk-full, overwrite, and destructive restore of the installed vault were not attempted on the user's data.

### Trusted devices and PC-to-PC pairing

Status: PASS FOR DISPOSABLE UI AND PROTOCOL, REAL DEVICES BLOCKED

The production Devices UI passes trusted-device listing, detail, fingerprint display, rename, revoke cancellation, revoke confirmation, and manual pairing address and code display. Manual-code pairing over real TCP, mismatched codes, SAS agreement, VMK transfer, device identity, trust persistence, rename storage, and revocation also pass. Completing actual UI pairing still needs two Windows installations.

### PC-to-PC synchronization

Status: PASS FOR DISPOSABLE UI AND PROTOCOL, REAL DEVICES BLOCKED

The production Sync UI passes online peer display, successful sync, no-device messaging, network failure, retry, conflict discard, and conflict recovery with disposable peers. Real UDP discovery, mDNS browse, fallback discovery, mutual TLS, untrusted rejection, revocation, bidirectional convergence, idempotent repeat sync, tombstones, offline concurrent edit, and recoverable conflict also pass. A real mutual-TLS regression synced 1,000 fake records across 50 projects in 13.37 seconds, verified exact Unicode sample fields, and completed an unchanged repeat sync in 16.87 ms with zero records applied. Firewall profiles, IP changes, interrupted large sync, and two-PC UI state remain blocked without a second PC or VM.

### Settings and persistence

Status: STRONG PARTIAL

The production Settings UI passes auto-lock and clipboard setting persistence across navigation, Local AI download and enable state, Windows account unlock enable and disable, authentication errors, password validation and change, disabled destructive actions, and release-version display. Settings IPC, idle query, clipboard timing logic, and native session-lock event handling also pass. Actual timed idle waits, Windows-wide activity, and process-restart persistence still require live operation.

### Windows privacy and security behavior

Status: STRONG PARTIAL

At-rest plaintext absence, backup encryption, lock read denial, DPAPI, capture API protection, clipboard clear logic, session-lock event code, mutual TLS, and offline Local AI pass. Snipping Tool, conferencing tools, `Win+V`, cloud clipboard, sleep, hibernate, user switching, taskbar previews, and Resource Monitor still need a human on a disposable Windows environment.

### Accessibility, display, and usability

Status: STRONG PARTIAL

Onboarding, join, Settings, import review, populated desktop vault, and populated mobile vault have no serious or critical axe violations. Programmatic field labels, switches, text areas, search controls, focusable secret selection, nested interactive controls, and contrast were tested and fixed. Layout checks pass at 1366 by 768 and effective 125, 150, and 200 percent scaling sizes. Forced colors and reduced motion pass automated emulation with no serious or critical WCAG AA findings. Narrator, actual Windows High Contrast, OS text scaling, multiple monitors, and alternate input devices remain human-only.

### Reliability, recovery, and limits

Status: PARTIAL

Corrupt vault, missing vault, process worker death, large Unicode payload, database migration, clock monotonicity, sync retry primitives, tombstones, backup corruption, and 1,000-record UI and sync performance pass. Forced process termination during write or restore, full disk, monitor disconnect, time-zone change, and overnight lock remain unexecuted destructive or long-duration tests.

### Release package and documentation acceptance

Status: FAIL FOR PUBLIC RELEASE

Version 0.1.9, current README screenshot generation, documentation links, bundle creation, local hashes, build, and required local quality gates pass. The newly rebuilt packages are not signed, are not yet copied into the release staging folder with updated checksums, and are not yet published as new GitHub assets. Public download, GitHub checks, and clean-PC installation therefore remain unverified.

## Human-only completion checklist

Run these before calling the Windows release fully approved:

- Install the newly built MSI and NSIS package on a clean Windows 11 VM. Clean Windows 10 Sandbox coverage already passes.
- Upgrade the previous public version on a separate VM with a fake vault.
- Cancel each installer midway and verify rollback interactively.
- Pair and sync two separate Windows PCs or VMs through the UI.
- Test offline edits, concurrent edits, conflict recovery, IP change, firewall Public and Private profiles, revocation, and restart on both PCs.
- Use Windows Hello interactively if supported.
- Test `Win+L`, sleep, hibernate, and user switching.
- Verify Clipboard History and cloud clipboard exclusion with fake values.
- Verify Snipping Tool, Print Screen, and a conferencing screen share.
- Test Narrator, High Contrast, reduced motion, 125, 150, and 200 percent scaling, 1366 by 768, and multiple monitors.
- Test backup and restore through the installed app on a disposable Windows vault.
- Sign the Windows artifacts with a trusted code-signing certificate, then repeat hash and clean-install checks.

## Final recommendation

Do not publish the current Windows artifacts as the final public release yet. Core behavior and automated desktop flows are healthy, and the defects found during this run are fixed, but real two-PC UI sync and trusted Windows code signing remain release blockers.
