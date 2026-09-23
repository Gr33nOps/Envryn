# Changelog

Notable user-facing changes are recorded here. Envryn follows semantic versioning while the project is in beta.

## 0.2.0-beta - 2026-09-24

### Local AI removed: a faster, lighter app

- The optional local AI is gone: no model to download (it was about 1 GB), no background worker process, and no AI settings. The app starts faster, uses less memory, and the installer is smaller. On first launch, a model downloaded by an earlier version is deleted to free the space.
- Envryn now has no HTTP client at all. The only network traffic it can make is sync with your own paired devices on your local network.

### Suggest name and Suggest type, rule-based

- Both buttons now use built-in rules for more than 70 known key formats (Stripe, OpenAI, GitHub, AWS, Slack, database URLs, private keys, JWTs, and more) plus common variable-name conventions. Results are instant and work offline.
- When a value is not recognised, Envryn says "Unknown" and leaves your field alone instead of guessing. Pasting a full `NAME=value` line suggests `NAME`.
- Search still understands phrases such as "production database", using the same rules.

### Projects can be deleted

- A project page now has a Delete button. It confirms how many secrets go with the project, then removes both. The deletion syncs to paired devices.

### Backups that save where you choose

- Backup and restore now use the system Save and Open dialogs, on Windows and Android, so any folder you can pick (Downloads, a USB drive, a cloud folder) works. This fixes the "can't back up, change the location" error.
- Restoring while the vault is unlocked now works; the previous vault file is kept aside with a timestamp.

### Sync works both ways, automatically

- Fixed phone-to-PC sync on PCs with WSL, Hyper-V, or VPN network adapters. The phone was trying those virtual addresses first and timing out; it now tries the address on your own network first and gives up on dead addresses in seconds.
- New "Sync automatically" setting (on by default): while both apps are open and unlocked, paired devices sync every 30 seconds, so changes show up without pressing Sync.
- The sync notification now reports what was sent and what was received, instead of "0 secrets synced" after a sync that sent changes.

### Mobile fixes

- The rename and delete project controls and the secret row actions are now always visible on touch screens, where there is no hover to reveal them.
- Tidied the project header spacing and empty state on small screens.

## 0.1.10-beta - 2026-09-22

### Sync now updates what you see

- Fixed device sync reporting success while the receiving device kept showing its old secrets. Reconciled records were being written to the database but the running app kept serving the list it had loaded at unlock, so a synced phone or PC looked unchanged until it was relocked or restarted. Both the device that pushes and the device that receives now refresh their in-memory view the moment a sync applies anything, and the open screen refetches automatically.
- The Sync page now keeps its "Online / Offline" indicator live by re-checking the local network on a short interval, so a paired device that is open and reachable shows as connected without pressing Sync first.

### Easier .env import

- You can now drag a `.env` file straight onto the import dialog, or pick one with a file button, instead of only pasting its contents. Nothing leaves the device -- the file is read locally.
- Added an "Import .env" action inside a project (on the project header and its empty state), so credentials can be brought into the project you are already looking at, with the project and environment prefilled.

### Expiration dates

- Secrets can now have an optional expiration date, set when creating or editing, and applied to a whole `.env` import at once. It suits credentials that lapse -- an IGDB app token (about 60 days), a password with a rotation deadline.
- Expiring and expired secrets are flagged in the list and in the detail panel; a secret is never deleted automatically.

### Local AI starts with the app

- When local AI is enabled in Settings, its on-device worker now starts automatically when the app opens, instead of needing the toggle pressed again each session. It stays best-effort and silent: never blocks the window, and remains off unless you have turned it on.

## 0.1.9-beta - 2026-08-29

### Better release confidence

- Added Playwright journeys for desktop and Pixel-sized mobile layouts, including onboarding, navigation, sync, settings, and serious accessibility violations.
- Added frontend coverage thresholds and bundle-size budgets to prevent quiet quality regressions.
- Expanded the tested frontend behavior to 83 unit and component tests alongside the Rust workspace suite.
- Fixed the recurring SonarQube Cloud failure by running the Playwright binary installed by `npm ci` and preventing unpinned `npx` commands in workflows.

### Safer dependency scanning

- Updated the Android release runtime to Jackson 2.18.9.
- Changed OSV scanning to inspect the dependencies that actually ship in the Android release instead of reporting build-only Android tooling as application dependencies.
- Kept Cargo, npm, Android runtime, Semgrep, CodeQL, SonarQube Cloud, secret scanning, and invariant checks in the release gate.

### Project polish

- Added current desktop and mobile screenshots generated from a repeatable test with fabricated data.
- Reworked the README and documentation index for users and contributors.
- Added contribution, support, conduct, issue, pull request, ownership, and release-note templates.
- Removed em dashes from project documentation.

## 0.1.8-beta - 2026-08-28

- Restored Android-to-PC sync discovery by holding Android's Wi-Fi multicast lock and keeping the sync listener alive for the full unlocked session.
- Reworked the vault for mobile with bottom navigation, touch-sized controls, responsive cards and sheets, safe-area spacing, and phone-first alignment while preserving Envryn's desktop branding.
- Added Android screenshot blocking, sensitive clipboard labels with safe timed clearing, disabled Android backups, and immediate background/screen-off locking.
- Raised Android support to Android 10 (API 29) or newer so a secrets manager cannot be installed on platform versions that no longer receive security fixes.
- Added reproducible free security gates using Gitleaks, OSV-Scanner, Semgrep, cargo-audit, cargo-deny, npm audit, invariant checks, protocol/identity fuzzing, and local MobSF APK analysis.
- Added a native Android clipboard plugin and hardened Android packaging so generated projects receive the required security and multicast configuration automatically.

## 0.1.6-beta

- Fixed Android pairing and restored branded Android launcher assets.
