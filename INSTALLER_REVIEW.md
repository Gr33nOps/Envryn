# Envryn — Installer Configuration Review

Static review of what Envryn's two installer types actually do, verified against the pinned
`tauri-utils 2.9.3` config schema this project builds against (same source used for
`RELEASE_SIGNING.md`) and the project's own `tauri.conf.json`/`tauri.windows.conf.json`. This is
the source-level counterpart to `CLEAN_VM_TEST_CHECKLIST.md`, which is where each claim below
gets verified by actually running the installers.

## Install location and elevation

Envryn ships both installer types (`bundle.targets: ["msi", "nsis"]` in `tauri.conf.json`), and
they behave differently by Tauri/WiX/NSIS's own defaults — **neither `tauri.conf.json` nor
`tauri.windows.conf.json` currently sets `nsis.installMode`**, so the NSIS installer runs at its
schema default.

| | NSIS (`Envryn_x.y.z_x64-setup.exe`) | MSI (`Envryn_x.y.z_x64_en-US.msi`, via WiX) |
|---|---|---|
| **Default mode** | `NSISInstallerMode::CurrentUser` (the `#[default]` variant in `tauri-utils`'s own schema — confirmed by reading the source, not assumed) | Per-machine (WiX's own standard behavior for a `Package` without an explicit per-user `InstallScope`; `WixConfig` has no mode toggle at all, so Tauri does not override WiX's default here) |
| **Elevation (UAC)** | Not required | Required |
| **Install location** | A per-user directory (no admin-writable path needed) | Under `Program Files` |
| **Registry metadata** | `HKCU` | `HKLM` |

This is a real, user-visible difference between the two artifacts this project ships side by
side, not a bug — Tauri intentionally supports both because different users want different
tradeoffs (no-admin-needed vs. shared-machine availability). It should be **stated to users**
which one is which, since "the same app, one asks for admin and one doesn't" is otherwise
surprising. Neither installer's mode has been changed from Tauri's shipped default in this
review — this documents the existing behavior, it does not alter it.

## Where user data lives, and why neither installer touches it

The vault database, its WAL/SHM sidecar files, cached AI model files, and DPAPI-protected
platform-slot data all live under the OS application-data directory resolved by
`app.path().app_data_dir()` (`src-tauri/src/ipc.rs::vault_path`, `sync.rs::identity_path`) —
Windows' per-user `%APPDATA%\<identifier>` Known Folder, keyed by the app identifier
`dev.envryn.vault` from `tauri.conf.json`. This is **structurally separate** from both install
locations above (`Program Files` or the NSIS per-user program directory) — installing,
upgrading, or uninstalling either package writes to a different folder than the one holding the
user's actual secrets.

## Uninstall behavior

Neither `tauri.conf.json` nor `tauri.windows.conf.json` sets `nsis.installerHooks` (the
`NSIS_HOOK_POSTUNINSTALL`-style hook Tauri's own schema supports for exactly this kind of
cleanup) or references a custom `.wxs` fragment targeting the app-data directory. **Tauri's
default WiX and NSIS templates do not delete arbitrary app-data directories on uninstall** —
that behavior has to be explicitly added, and Envryn has not added it.

**Conclusion: uninstalling Envryn is expected to leave the vault database and all user secrets
on disk, untouched.** This is the correct default for a secrets manager — an uninstall should
never be a silent, irreversible way to destroy someone's vault — but it means:

- A user who wants their data gone (not just the app) needs to know to also delete
  `%APPDATA%\dev.envryn.vault` (or wherever it resolves) themselves. This is not currently
  documented anywhere user-facing; worth a line in the app's own uninstall-adjacent UI or a
  README section before a public release, not fixed in this pass since it's a documentation gap,
  not a code change this review was asked to make.
- `CLEAN_VM_TEST_CHECKLIST.md` §8 is where this claim gets an actual, dynamic confirmation on a
  real machine rather than resting on reading the config alone.

## What was not changed

This review is read-only against the current configuration — no installer behavior was modified.
If a future decision is made to change NSIS's default install mode, add an uninstall-cleanup
hook, or otherwise diverge from Tauri's shipped defaults, that is a deliberate product decision
requiring its own review (particularly: an uninstall hook that deletes user data needs a
confirmation prompt, not a silent default, to avoid turning "uninstall" into an unrecoverable
data-loss action).
