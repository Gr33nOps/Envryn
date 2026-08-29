# Release process

Envryn releases are built from a clean `main` branch after every required check has passed. The project currently uses beta versions such as `0.1.9-beta`.

## 1. Choose the version

Use semantic versioning:

- Patch: fixes, tests, documentation, and compatible improvements
- Minor: meaningful new features or user-facing workflows
- Major: incompatible vault, sync, API, or support changes

Keep these values aligned:

- Workspace version in `Cargo.toml`
- Application version in `src-tauri/tauri.conf.json`
- Envryn package entries in `Cargo.lock`
- Version heading in `CHANGELOG.md`

## 2. Prepare the release

1. Update `CHANGELOG.md` with user-facing changes.
2. Write release notes under `docs/releases/`.
3. Refresh README screenshots with `npm run screenshots:readme` when the interface changed.
4. Confirm there are no em dashes in Markdown with `rg ([char]0x2014) -g "*.md"` in PowerShell.
5. Review installer and compatibility notes.

## 3. Run the release gate

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
npm run lint
npm run typecheck
npm run test:coverage --workspace @envryn/ui
npm run build
npm run test:bundle-budget
npm run test:e2e
npm run test:security-invariants
npm audit --audit-level=high
```

Also verify:

- GitHub CI, CodeQL, SonarQube Cloud, and secret scanning pass on the release commit.
- OSV-Scanner, `cargo-audit`, `cargo-deny`, Semgrep, and Gitleaks report no unreviewed release blocker.
- The Android release APK builds and uses the established Envryn signing identity.
- The Windows MSI and NSIS installer build from the same commit.
- The generated TypeScript bindings have no uncommitted drift.

## 4. Build artifacts

The release should include:

- Universal Android APK
- Windows NSIS setup executable
- Windows MSI package
- SHA-256 checksum file
- CycloneDX software bill of materials

Do not publish a new signing identity by accident. Android updates must continue using the existing release key. Windows packages remain unsigned until the process in `RELEASE_SIGNING.md` is completed.

## 5. Publish

1. Commit the version and release documentation.
2. Push `main` and wait for all checks.
3. Create an annotated tag such as `v0.1.9-beta` at the verified commit.
4. Create a GitHub prerelease from the prepared notes.
5. Upload every artifact and checksum.
6. Download or inspect the published asset list and verify the release points to the intended commit.

## 6. Roll back

If a release artifact is wrong, mark the release as a draft or remove the affected asset before announcing it. If users may already have downloaded it, publish a clear warning and issue a new patch release. Never replace a published binary silently under the same version.
