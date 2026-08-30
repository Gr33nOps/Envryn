# Envryn beta release checklist

Use this checklist for every public beta. The detailed commands and rationale live in the [release process](../RELEASE_PROCESS.md).

## Release identity

- [ ] `Cargo.toml` contains the intended numeric application version.
- [ ] `src-tauri/tauri.conf.json` contains the same version.
- [ ] Envryn workspace packages in `Cargo.lock` contain the same version.
- [ ] `CHANGELOG.md` and the release notes contain the matching beta version.
- [ ] The tag does not already exist locally or on GitHub.

## Documentation

- [ ] README installation and support notes match the release.
- [ ] Product screenshots show the current interface and fabricated data only.
- [ ] `npm run test:docs` passes.
- [ ] User-facing limitations are stated plainly.
- [ ] Windows unsigned-package and Android unknown-source warnings are included.

## Quality gate

- [ ] Rust formatting passes.
- [ ] Strict Clippy passes for all targets and features.
- [ ] The complete Rust workspace test suite passes.
- [ ] Frontend lint and type checking pass.
- [ ] Frontend unit and component tests pass with coverage thresholds.
- [ ] Desktop and mobile Playwright journeys pass.
- [ ] Accessibility checks pass.
- [ ] The bundle budget passes.
- [ ] Generated TypeScript bindings have no drift.

## Security and privacy gate

- [ ] Security invariant tests pass.
- [ ] Gitleaks reports no secret.
- [ ] Semgrep reports no blocking finding.
- [ ] npm audit reports no vulnerability at the release threshold.
- [ ] OSV-Scanner reports no vulnerability in shipped dependency graphs.
- [ ] `cargo-audit` and `cargo-deny` pass with only documented reviewed exceptions.
- [ ] CodeQL and SonarQube Cloud pass on the release commit.
- [ ] Any APK scan findings have been reviewed.

## Build artifacts

- [ ] The universal Android release APK builds.
- [ ] The APK package name and version are correct.
- [ ] The APK uses the established Envryn signing identity.
- [ ] The Windows NSIS setup executable builds.
- [ ] The Windows MSI builds.
- [ ] Windows signing status is reported honestly.
- [ ] A CycloneDX SBOM is generated.
- [ ] `CHECKSUMS.txt` includes every published binary and the SBOM.

## Publish

- [ ] The release commit is pushed to `main`.
- [ ] All required GitHub checks pass.
- [ ] A signed annotated `v<version>-beta` tag points to the verified commit.
- [ ] The GitHub release is marked as a prerelease.
- [ ] Release notes explain the user-facing changes and limitations.
- [ ] Every expected asset is uploaded exactly once.
- [ ] GitHub artifact attestations exist for CI-built Windows installers and the checksum manifest.
- [ ] Published asset names and checksums are inspected after upload.

## Rollback triggers

Stop or withdraw the release if:

- A release check fails or is missing.
- A package has the wrong version, identity, or checksum.
- An installer cannot upgrade the previous beta without losing access to the vault.
- Pairing, sync, lock, unlock, backup, or restore fails in a supported flow.
- A new high-impact security or privacy finding appears.

Never replace a published binary silently under the same version. Fix the issue and publish a new patch release.
