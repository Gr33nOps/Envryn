# Changelog

## 0.1.8-beta — 2026-08-28

- Restored Android-to-PC sync discovery by holding Android's Wi-Fi multicast lock and keeping the sync listener alive for the full unlocked session.
- Reworked the vault for mobile with bottom navigation, touch-sized controls, responsive cards and sheets, safe-area spacing, and phone-first alignment while preserving Envryn's desktop branding.
- Added Android screenshot blocking, sensitive clipboard labels with safe timed clearing, disabled Android backups, and immediate background/screen-off locking.
- Raised Android support to Android 10 (API 29) or newer so a secrets manager cannot be installed on platform versions that no longer receive security fixes.
- Added reproducible free security gates using Gitleaks, OSV-Scanner, Semgrep, cargo-audit, cargo-deny, npm audit, invariant checks, protocol/identity fuzzing, and local MobSF APK analysis.
- Added a native Android clipboard plugin and hardened Android packaging so generated projects receive the required security and multicast configuration automatically.

## 0.1.6-beta

- Fixed Android pairing and restored branded Android launcher assets.
