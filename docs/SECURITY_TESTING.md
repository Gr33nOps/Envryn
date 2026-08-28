# Security and privacy testing

Envryn's security checks are local, reproducible, and free. No vault or source data is uploaded by these commands.

## Standard gate

Run from the repository root:

```powershell
npm run security:scan
```

The gate verifies hardening invariants, scans Git history with Gitleaks, runs the project's Semgrep rules, enforces Rust dependency/license/source policy with `cargo-deny`, checks Rust advisories with `cargo-audit`, checks all lockfiles with OSV-Scanner, and audits production npm dependencies. Reviewed target-specific exceptions live in `deny.toml` and `osv-scanner.toml`; new findings still fail the command.

## Android APK analysis

With Docker running:

```powershell
npm run security:scan-apk -- .\target\release\Envryn_0.1.8_android-universal.apk
```

This starts MobSF bound only to `127.0.0.1`, uses an ephemeral random API key, stores the JSON report under ignored `target/security/`, and removes the container afterward.

## Fuzzing

The fuzz package covers encrypted-record opening, backup restore, sync wire messages, and device identity parsing. A short local smoke run is:

```powershell
cargo +nightly fuzz run fuzz_sync_wire -- -max_total_time=30
cargo +nightly fuzz run fuzz_device_identity -- -max_total_time=30
```

Longer fuzz runs improve coverage. Corpora, crashes, coverage output, compiled targets, APK reports, and other generated security artifacts stay outside Git.

## Required release checks

Every release also runs Rust formatting, Clippy with warnings denied, the full Rust workspace test suite, frontend lint/type checking/tests/build, signed-APK verification, an Android launch smoke test, and checksum generation. A physical Windows-to-Android sync pass remains valuable because emulators cannot reproduce every router, OEM Wi-Fi, and battery-management behavior.
