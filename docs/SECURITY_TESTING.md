# Security and privacy testing

Envryn's standard security gate is local, reproducible, and free. It does not upload vault or source data.

## Standard gate

Run from the repository root:

```powershell
npm run security:scan
```

The gate verifies hardening invariants, scans Git history with Gitleaks, runs the project's Semgrep rules, enforces Rust dependency/license/source policy with `cargo-deny`, checks Rust advisories with `cargo-audit`, checks Cargo/npm locks and the Android release-runtime dependency slice with OSV-Scanner, and audits production npm dependencies. Android Gradle Plugin emulator/test-host tooling is deliberately excluded because it is not packaged in the APK. Reviewed target-specific exceptions live in `deny.toml` and `osv-scanner.toml`; new shipped-runtime findings still fail the command.

## Snyk GitHub analysis

The `Snyk Security` GitHub workflow adds two high-severity gates:

- Snyk Open Source checks the committed npm lockfile for vulnerable dependency paths and license issues.
- Snyk Code performs static analysis across supported TypeScript and Rust source files.

Both scans publish SARIF results to GitHub's Security tab. Snyk Code sends supported source files to Snyk for analysis; `.snyk` excludes four credential-shaped test/demo fixture files from that upload. Those files remain covered by the local deterministic secret scanner and the repository's existing tests.

Standard Snyk Open Source testing does not resolve Cargo manifests. Rust dependencies therefore remain gated by `cargo-audit`, OSV-Scanner, and `cargo-deny` in the existing CI workflow rather than being presented as incomplete Snyk coverage.

The workflow requires the encrypted `SNYK_TOKEN` repository secret. GitHub does not expose repository secrets to fork or Dependabot pull requests, so the Snyk job skips those untrusted contexts instead of failing authentication.

## Android APK analysis

With Docker running:

```powershell
npm run security:scan-apk -- .\target\release\Envryn_0.1.9_android-universal.apk
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
