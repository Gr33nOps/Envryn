# Envryn - Installer Configuration Review

> Historical installer review. Use [BETA_RELEASE_CHECKLIST.md](BETA_RELEASE_CHECKLIST.md) and [docs/RELEASE_PROCESS.md](docs/RELEASE_PROCESS.md) for the current release gate.

Static review of what Envryn's two installer types actually do, verified against the pinned
`tauri-utils 2.9.3` config schema this project builds against (same source used for
`RELEASE_SIGNING.md`) and the project's own `tauri.conf.json`/`tauri.windows.conf.json`. This is
the source-level counterpart to `CLEAN_VM_TEST_CHECKLIST.md`, which is where each claim below
gets verified by actually running the installers.

## Install location and elevation

Envryn ships both installer types (`bundle.targets: ["msi", "nsis"]` in `tauri.conf.json`), and
they behave differently by Tauri/WiX/NSIS's own defaults - **neither `tauri.conf.json` nor
`tauri.windows.conf.json` currently sets `nsis.installMode`**, so the NSIS installer runs at its
schema default.

|                       | NSIS (`Envryn_x.y.z_x64-setup.exe`)                                                                                                      | MSI (`Envryn_x.y.z_x64_en-US.msi`, via WiX)                                                                                                                                                 |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Default mode**      | `NSISInstallerMode::CurrentUser` (the `#[default]` variant in `tauri-utils`'s own schema - confirmed by reading the source, not assumed) | Per-machine (WiX's own standard behavior for a `Package` without an explicit per-user `InstallScope`; `WixConfig` has no mode toggle at all, so Tauri does not override WiX's default here) |
| **Elevation (UAC)**   | Not required                                                                                                                             | Required                                                                                                                                                                                    |
| **Install location**  | A per-user directory (no admin-writable path needed)                                                                                     | Under `Program Files`                                                                                                                                                                       |
| **Registry metadata** | `HKCU`                                                                                                                                   | `HKLM`                                                                                                                                                                                      |

This is a real, user-visible difference between the two artifacts this project ships side by
side, not a bug - Tauri intentionally supports both because different users want different
tradeoffs (no-admin-needed vs. shared-machine availability). It should be **stated to users**
which one is which, since "the same app, one asks for admin and one doesn't" is otherwise
surprising. Neither installer's mode has been changed from Tauri's shipped default in this
review - this documents the existing behavior, it does not alter it.

## Where user data lives, and why neither installer touches it

The vault database, its WAL/SHM sidecar files, cached AI model files, and DPAPI-protected
platform-slot data all live under the OS application-data directory resolved by
`app.path().app_data_dir()` (`src-tauri/src/ipc.rs::vault_path`, `sync.rs::identity_path`) -
Windows' per-user `%APPDATA%\<identifier>` Known Folder, keyed by the app identifier
`dev.envryn.vault` from `tauri.conf.json`. This is **structurally separate** from both install
locations above (`Program Files` or the NSIS per-user program directory) - installing,
upgrading, or uninstalling either package writes to a different folder than the one holding the
user's actual secrets.

## Why the data folder exists before any vault does - traced, not assumed

A clean-VM test run observed `%APPDATA%\dev.envryn.vault` existing seconds after first launch,
with no vault ever created. Traced to source, not guessed:

1. `src-tauri/src/lib.rs`'s `.setup()` calls `autolock::spawn(handle)` unconditionally, on every
   launch, regardless of whether a vault exists (`src-tauri/src/lib.rs:46`).
2. `autolock::spawn` starts a background loop (`src-tauri/src/autolock.rs`) that calls `tick()`
   every `POLL_INTERVAL` (5 seconds), for the lifetime of the process.
3. Every `tick()` calls `settings::load(app)` unconditionally, to read the current auto-lock
   threshold - by the module's own documented design, this **must** work before a vault exists:
   "the vault must be usable before it is ever unlocked -- an auto-lock timeout that could only
   be read from inside the very thing it locks would be a circular dependency"
   (`src-tauri/src/settings.rs`'s own module doc).
4. `settings::load` calls `settings_path(app)`, which calls `app.path().app_config_dir()` and
   then **unconditionally** runs `std::fs::create_dir_all(&dir)` before checking whether a
   settings file exists there at all (`src-tauri/src/settings.rs::settings_path`).
5. On Windows, `app_config_dir()` and `app_data_dir()` resolve to the **same physical path**.
   Verified directly against the pinned `tauri` 2.11.5 source
   (`tauri-2.11.5/src/path/desktop.rs`): `app_config_dir()` is `dirs::config_dir().join(identifier)`
   and `app_data_dir()` is `dirs::data_dir().join(identifier)` - and the `dirs` crate resolves
   both `config_dir()` and `data_dir()` to `%APPDATA%` on Windows, since Windows has no
   OS-level config/data distinction the way XDG does on Linux. Confirmed empirically too: a real
   install on a real machine shows `settings.json` sitting in the identical folder as
   `envryn.db`.

**What this does and does not mean:**

- The folder gets created within ~5 seconds of _any_ launch, vault or no vault. This is expected,
  deliberate behavior, not a bug - auto-lock's whole design requires settings to be readable
  before a vault exists.
- **Nothing security-sensitive is created by this path.** `settings::load` only _reads_; only
  `settings::save` (called exclusively by the `settings_set` IPC command, itself only invoked
  from the Settings screen when a user changes a preference - `apps/ui/src/routes/vault/settings.tsx`,
  grepped for every `settingsSet` call site to confirm this is the only one) ever _writes_
  `settings.json`. And even when it does, that file holds only `auto_lock_minutes`,
  `clipboard_clear_seconds`, and `ai_enabled` - explicitly documented as "non-secret application
  preferences... deliberately not stored in the vault" (`settings.rs`'s own module doc). No vault
  data, no key material, nothing sealed ever lands here as a result of this code path.
- Docs and the clean-VM checklist that implied this folder only appears _after_ vault creation
  were incorrect and have been corrected (`CLEAN_VM_TEST_CHECKLIST.md` §2) - not the code, which
  is working as designed.

## Uninstall behavior

Neither `tauri.conf.json` nor `tauri.windows.conf.json` sets `nsis.installerHooks` (the
`NSIS_HOOK_POSTUNINSTALL`-style hook Tauri's own schema supports for exactly this kind of
cleanup) or references a custom `.wxs` fragment targeting the app-data directory. **Tauri's
default WiX and NSIS templates do not delete arbitrary app-data directories on uninstall** -
that behavior has to be explicitly added, and Envryn has not added it.

**Conclusion: uninstalling Envryn is expected to leave the vault database and all user secrets
on disk, untouched.** This is the correct default for a secrets manager - an uninstall should
never be a silent, irreversible way to destroy someone's vault - but it means:

- A user who wants their data gone (not just the app) needs to know to also delete
  `%APPDATA%\dev.envryn.vault` (or wherever it resolves) themselves. This is not currently
  documented anywhere user-facing; worth a line in the app's own uninstall-adjacent UI or a
  README section before a public release, not fixed in this pass since it's a documentation gap,
  not a code change this review was asked to make.
- `CLEAN_VM_TEST_CHECKLIST.md` §8 is where this claim gets an actual, dynamic confirmation on a
  real machine rather than resting on reading the config alone.

## What was not changed

This review is read-only against the current configuration - no installer behavior was modified.
If a future decision is made to change NSIS's default install mode, add an uninstall-cleanup
hook, or otherwise diverge from Tauri's shipped defaults, that is a deliberate product decision
requiring its own review (particularly: an uninstall hook that deletes user data needs a
confirmation prompt, not a silent default, to avoid turning "uninstall" into an unrecoverable
data-loss action).
