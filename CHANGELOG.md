# Changelog

Notable user-facing changes are recorded here. Envryn follows semantic versioning while the project is in beta.

## 0.1.10-beta - 2026-09-22

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
