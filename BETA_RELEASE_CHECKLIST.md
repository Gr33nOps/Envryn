# Envryn — Controlled Beta Release Checklist

This is the exact process for cutting a beta release of the artifacts already built and verified
in this pass. **Nothing in this document publishes anything** — it's the checklist to work
through when you decide to.

## 0. Version-collision risk — resolved

The GitHub release `v0.1.2-beta` (published before the security-remediation and
release-hardening work landed) already had assets named `Envryn_0.1.2_x64_en-US.msi` and
`Envryn_0.1.2_x64-setup.exe`. A fresh build against `main` after that work still carried version
`0.1.2` and would have produced **filename-identical, checksum-different** artifacts — two
genuinely different binaries indistinguishable by name alone.

| File | Published `v0.1.2-beta` (old, unchanged, still live) | The `0.1.2`-labeled build this repo would otherwise have produced next |
|---|---|---|
| `Envryn_0.1.2_x64_en-US.msi` | `sha256:eb3a8d20f1ab98eaae289dd96652600384c99ba94859831339170f8fe3a796b6` | `sha256:34d44a6e0e55851d6e70555c489d0f67a9691a6ccce84530a2536721c2fbee92` |
| `Envryn_0.1.2_x64-setup.exe` | `sha256:e8ad069f5c40ed88dc9643d5e9b18896d34cc7553ac0ba1e5efe5f1f843faecf` | `sha256:d0be22db615776991419989c5ba8cec80a1201d0b25fd92fef5ae4729497cc9c` |

**Fixed: the version is now `0.1.3`** (`Cargo.toml`, `src-tauri/tauri.conf.json`,
`apps/ui/src/routes/vault/settings.tsx`'s About text — `Cargo.lock`/`fuzz/Cargo.lock` regenerated
to match). Every artifact §1 produces is now named `Envryn_0.1.3_...`, which cannot collide with
either existing `v0.1.1-beta` or `v0.1.2-beta` release. Neither of those releases was modified or
deleted — this only changes what the *next* build calls itself.

Locally-stale artifacts from earlier in this development machine's history (`0.1.0` and `0.1.1`
builds that were sitting in `target/release/bundle/` alongside newer ones) were also found and
deleted in an earlier pass.

## 1. Build

```powershell
npm run build --workspace @envryn/ui
npm run prepare:sidecar
cargo tauri build
```

Produces, under `target/release/bundle/`:
- `msi/Envryn_<version>_x64_en-US.msi`
- `nsis/Envryn_<version>_x64-setup.exe`

If this machine's build has been slow or failed with a memory-related error during this pass, see
`SECURITY_REMEDIATION_REPORT.md`'s and `RELEASE_SIGNING.md`'s notes on this being the
maintainer's own desktop, not a dedicated build box — `$env:CARGO_BUILD_JOBS = "2"` before
building is the known mitigation.

## 2. Compute and record checksums

```powershell
Get-FileHash target\release\bundle\msi\Envryn_<version>_x64_en-US.msi -Algorithm SHA256
Get-FileHash target\release\bundle\nsis\Envryn_<version>_x64-setup.exe -Algorithm SHA256
```

Record both `Hash` values — they go in the release notes (§4) verbatim, and this is what every
user is told to check before running the installer (README's "Manual updates" section,
`docs/UPDATE_POLICY.md`).

## 3. Confirm signing status honestly

```powershell
Get-AuthenticodeSignature target\release\bundle\msi\Envryn_<version>_x64_en-US.msi
Get-AuthenticodeSignature target\release\bundle\nsis\Envryn_<version>_x64-setup.exe
```

Expected right now: `Status: NotSigned` on both. That's correct, not a failed check — see
`RELEASE_SIGNING.md`. If this ever prints `Valid` instead, that means signing was actually wired
up since this checklist was written — update the release-notes template in §4 accordingly rather
than leaving the unsigned-warning language in place for a build that no longer needs it.

## 4. Release-notes template — label the build honestly, every time

Use this verbatim shape for the release description (fill in the version and checksums from
§1–§3):

```markdown
## Envryn v<version> — Beta, unsigned build

**This build is not code-signed.** Windows SmartScreen will very likely show a
"Windows protected your PC" warning the first time you run either installer. This is
expected for an unsigned binary and will remain true until a code-signing certificate is in
place (tracked in RELEASE_SIGNING.md) — it does not mean anything is wrong with this specific
download. If you see that warning, verify the checksum below before doing anything else; do not
proceed based on trusting the warning away.

**Verify before installing** (PowerShell):
```
Get-FileHash .\Envryn_<version>_x64_en-US.msi -Algorithm SHA256
Get-FileHash .\Envryn_<version>_x64-setup.exe -Algorithm SHA256
```
Expected:
- `Envryn_<version>_x64_en-US.msi` → `<sha256 from §2>`
- `Envryn_<version>_x64-setup.exe` → `<sha256 from §2>`

If the hash you compute doesn't match, do not run the installer — download again from this exact
release page, not anywhere else.

**No auto-updater.** Future versions must be installed manually the same way — see the "Manual
updates" section of the README.

**Uninstalling does not delete your vault.** Your data is stored separately and survives an
uninstall by design — see the README's "Uninstalling and removing your data" section if you ever
want it gone too.
```

Do **not** shorten or soften the SmartScreen paragraph, and do not add any instruction to
disable, bypass, or click through SmartScreen faster than a user would otherwise — the warning
existing is accurate given the current signing status, not a false positive to explain away.

## 5. Before actually publishing (out of scope for this pass — a human decision point)

- [ ] §0's version bump is done and this checklist's §1–§4 were re-run against the new version.
- [ ] At least one full pass of `CLEAN_VM_TEST_CHECKLIST.md` has genuinely completed on a real
      clean VM, not skipped or assumed.
- [ ] The release-notes text from §4 is used as-is (or expanded, never trimmed of its warnings).
- [ ] You've decided who "controlled beta" actually means — a private link, specific testers,
      an unlisted release — and configured GitHub's release visibility to match. This pass does
      not make that decision or publish anything.
