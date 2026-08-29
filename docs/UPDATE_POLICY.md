# Envryn - Update Policy (v0.1.x)

## Decision: no auto-updater for v0.1.x, and that is the right call right now, not a gap to
## apologize for

Envryn ships with **no update-checking code of any kind** - confirmed structurally, not just by
absence of a feature flag: zero `tauri-plugin-*` dependencies exist in the tree at all (see
`SECURITY_REMEDIATION_REPORT.md` §13/§14), so there is no `tauri-plugin-updater`, no
background version check, and no code path that could silently fetch and run a different binary
than the one the user installed.

This is a deliberate evaluation, not an oversight, and specifically because of *what* Envryn is:

1. **An auto-updater is a second, standing network-attack surface for a secrets manager.**
   Whatever endpoint it polls becomes something an attacker who compromises DNS, a CDN, or the
   update server itself can use to serve a malicious binary to every installed copy of the app at
   once - the single highest-leverage attack against any auto-updating desktop application.
2. **An unsigned auto-updater is arguably worse than no updater.** Tauri's updater plugin signs
   update artifacts with its own minisign-based scheme *in addition to* - not instead of - real
   Authenticode code signing on the binaries themselves. Envryn has neither yet (see
   `RELEASE_SIGNING.md`). Shipping an auto-updater before the underlying binaries are even signed
   would train users to trust silent, unverified binary replacement - precisely the pattern a
   password manager should not normalize.
3. **v0.1.x is not yet a public release.** There is no install base to protect from
   fragmentation yet, and no track record of update cadence to build automation around
   prematurely.

**Revisit this, not before:** real Authenticode signing exists (`RELEASE_SIGNING.md`), and there
is an actual install base past the private/beta stage this project is at today. Adding
`tauri-plugin-updater` at that point is an additive change - it does not require redesigning
anything reviewed in this document or in `RELEASE_SIGNING.md`.

## The manual update process, until then

Documented here so it's secure by construction, not left to a user's own judgment call about
what "the real download" looks like.

1. **Only get a new installer from the GitHub Releases page for this exact repository**
   (`github.com/Gr33nOps/Envryn/releases`) - never from a link in an email, a search result, or
   any third-party mirror. This project has one authoritative distribution point.
2. **Verify the SHA-256 checksum before running anything.** Every release's notes publish the
   checksum for each artifact (MSI and NSIS `.exe`) - this project's own convention, matching how
   `SECURITY_REMEDIATION_REPORT.md` and this pass's own build verification already record them.
   On Windows:
   ```powershell
   Get-FileHash .\Envryn_x.y.z_x64-setup.exe -Algorithm SHA256
   ```
   Compare the output against the release notes' published value before running the installer -
   not after. A mismatch means the download is corrupted or, worse, not the real artifact; do not
   run it either way.
3. **Run the new installer directly over the existing install** - this is a real in-place
   upgrade, not a reinstall-from-scratch:
   - The WiX (MSI) upgrade code is deterministically generated from the product name (see
     `RELEASE_SIGNING.md` §4's review of `WixConfig`) and stays stable release to release unless
     someone deliberately overrides it, which lets Windows Installer recognize a new MSI as an
     upgrade of the same product rather than a separate, conflicting install.
   - Neither installer touches the vault database, which lives under the OS app-data directory
     (`app_data_dir()`), entirely separate from the install directory both installers write to
     (`Program Files` for MSI, a per-user directory for NSIS - see `RELEASE_SIGNING.md`'s sibling
     review and `CLEAN_VM_TEST_CHECKLIST.md` §1/§8 for where exactly). An in-place upgrade does
     not touch, migrate, or risk the vault.
4. **Once signing exists** (`RELEASE_SIGNING.md`), this same manual process gains a second,
   independent integrity check for free: Windows itself will show the binary as signed by a
   verified publisher, on top of the checksum comparison above - belt and suspenders, not a
   replacement for either.

## What this policy does not cover

- **Notifying users a new version exists at all.** With no updater, that notification channel
  doesn't exist yet either - today, "check the Releases page yourself" is the whole mechanism.
  Worth a lightweight, explicitly-user-initiated "check for updates" IPC command (a single HTTPS
  GET to the GitHub Releases API, surfaced in Settings, never automatic) as a real middle ground
  before a full auto-updater - noted here as a reasonable next step, not built in this pass since
  it wasn't asked for and touches new network-egress surface that deserves its own review when it
  is.
- **Downgrade protection.** `WindowsConfig.allow_downgrades` defaults to `true` in Tauri's own
  schema (verified against the pinned `tauri-utils` source, see `RELEASE_SIGNING.md`), meaning a
  user could install an older MSI over a newer one today. Not a vulnerability by itself - the
  vault's own format-version refusal (`SECURITY_INVARIANTS.md` §11) already prevents an older
  build from silently misreading a newer vault format - but worth knowing this is the current,
  unreviewed default rather than a considered choice.
