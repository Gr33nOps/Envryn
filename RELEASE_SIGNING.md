# Envryn — Windows Release Signing

**Status: not yet signed.** This document specifies exactly what Envryn needs to ship signed
Windows binaries, and how the build pipeline is already structured so that adding a real
certificate later is a config addition, not a redesign. Nothing here purchases, provisions, or
weakens anything — it is the plan to execute once a certificate exists.

---

## 1. What "signing" actually means here

Every artifact `cargo tauri build` produces — `envryn.exe`, the WiX `.msi`, the NSIS
`...-setup.exe` — is currently **unsigned** (verified directly with
`Get-AuthenticodeSignature`, see `SECURITY_REMEDIATION_REPORT.md`'s addendum). Windows
Authenticode signing embeds a cryptographic signature tying the binary to a certificate issued
by a CA in Microsoft's Trusted Root Program. It matters for two independent reasons:

1. **Integrity/provenance** — a signature proves the file hasn't been altered since Envryn built
   it, and identifies who published it. This is the property most relevant to a *secrets
   manager* specifically: a user (or an antivirus product, or a future auto-update check) has no
   cryptographic way to distinguish a genuine Envryn build from a tampered one without it.
2. **SmartScreen reputation** — Windows Defender SmartScreen flags unsigned or low-reputation
   executables with a "Windows protected your PC" interstitial. Signing is *necessary* to build
   reputation but **not sufficient for it to disappear immediately** — see §3.

## 2. What certificate Envryn needs — and a real constraint that changed the landscape

As of **June 1, 2023**, the CA/Browser Forum's Baseline Requirements for Code Signing Certificates
mandate that **every** code-signing private key — Organization-Validated (OV) or
Extended-Validation (EV) alike — must be generated and held on a FIPS 140-2 Level 2 (or Common
Criteria EAL 4+) validated hardware device. Plain exportable `.pfx` software certificates are no
longer issued by any compliant CA for new orders. This one requirement decides the realistic
shape of a CI-integrated signing setup:

| Option | What it is | Rough cost | CI-friendliness |
|---|---|---|---|
| **OV cert on a physical USB HSM token** | Traditional route (SSL.com, Sectigo, DigiCert, GlobalSign) | ~$100–400/yr | Poor for CI unless the token stays permanently plugged into this specific machine — not portable, not scriptable from a fresh runner |
| **EV cert on a physical USB HSM token** | Same as above, stricter validation | ~$300–700/yr | Same physical-token limitation; historically got faster SmartScreen reputation, but Microsoft has walked back the "EV = instant trust" behavior — don't plan around it |
| **Cloud HSM signing service** (Azure Trusted Signing, DigiCert KeyLocker, SSL.com eSigner) | Same certificate-strength guarantee, key never leaves a cloud HSM, signing happens via an API/CLI call from any machine (including CI) | Azure Trusted Signing: ~$10/mo subscription. Others: comparable to OV/EV cert pricing plus a signing-service fee | **Best fit for this project** — no token to physically manage, works identically from the self-hosted runner or a future GitHub-hosted one, and Azure Trusted Signing specifically was built for exactly this indie/ISV CI use case |

**Recommendation (not a purchase, not acted on):** a cloud HSM signing service, most likely Azure
Trusted Signing given its price point and CI-first design, is the realistic fit for a zero-budget
project once code signing becomes a priority. A physical token is the cheaper *sticker* price but
turns "build a release" into a manual, single-machine ritual — the opposite of what a CI pipeline
is for.

## 3. Be honest about what signing does and doesn't buy

Do not let "we bought a certificate" become "SmartScreen warnings are solved." A brand-new
signing identity — OV or EV — still shows *some* friction until Microsoft's reputation service
has seen enough real installs of that specific, consistently-signed binary. Signing is the
prerequisite for that reputation to ever accrue; it is not an instant fix. Say this plainly to
users of an early release rather than promising a warning-free install experience the first
certificate purchase can't actually deliver.

## 4. Exact configuration Envryn will need (verified against the pinned `tauri-utils 2.9.3`
## source this project actually builds against — not assumed from memory)

`tauri-utils::config::WindowsConfig` (the schema behind `bundle.windows` in
`tauri.conf.json`/`tauri.windows.conf.json`) already has every field this needs, today, with no
schema changes required:

```jsonc
// src-tauri/tauri.windows.conf.json — additive only, nothing here today needs to change shape
{
  "bundle": {
    "externalBin": ["binaries/envryn-ai-worker"],
    "windows": {
      "digestAlgorithm": "sha256",          // required for signing; SHA-256 recommended
      "certificateThumbprint": "<SHA1 thumbprint of the cert, once imported>",
      "timestampUrl": "http://timestamp.digicert.com",  // or the CA's own RFC 3161 endpoint
      "tsp": false                           // set true only if the CA specifically requires RFC 3161 TSP
    }
  }
}
```

For a **cloud HSM** signing service (the recommended path, §2), `certificateThumbprint` alone
isn't enough — `signtool.exe` needs to invoke the service's own signing client instead of a
locally-installed cert. `WindowsConfig.sign_command` exists for exactly this:

```jsonc
"windows": {
  "digestAlgorithm": "sha256",
  "signCommand": "azuresigntool sign -kvu <vault-url> -kvc <cert-name> -tr <timestamp-url> -td sha256 %1"
}
```

(`%1` is replaced by Tauri with the path of the file being signed — this is the bundler's own
documented placeholder, not something this project invents.)

**Nothing above is committed to the repo yet.** Both are additive `bundle.windows` keys; adding
them later is a one-file config change, not a restructuring of `tauri.conf.json` or the build
process.

## 5. CI wiring plan (documented now, not built now — no secret exists to wire to)

Once a certificate/signing-service exists:

1. Store the credential as a **GitHub Actions repository secret** (e.g. `WINDOWS_SIGN_*`) —
   never in a committed file, matching every other secret-handling rule already enforced by
   `deny.toml`/`.semgrep/`.
2. Add one step to `.github/workflows/release.yml` (already scaffolded, commented out, with this
   exact insertion point marked) **before**
   `cargo tauri build` that either imports a cert into the runner's certificate store
   (`Import-PfxCertificate`, only relevant if a token/exportable cert is ever used) or simply
   exposes the cloud-HSM client's own auth env vars for the `signCommand` invocation above.
3. Pass `certificateThumbprint`/`signCommand` to the build via `cargo tauri build --config` (the
   Tauri CLI's own documented mechanism for merging a JSON fragment into the base config at
   build time) — so the real thumbprint/command never has to be committed to
   `tauri.windows.conf.json` in plaintext, avoiding an unnecessary secret-in-git even though a
   thumbprint alone isn't itself sensitive.
4. Re-run `Get-AuthenticodeSignature` (exactly as done in `SECURITY_REMEDIATION_REPORT.md`'s
   addendum) against the built artifacts as a real, automated verification step — "signed" should
   be proven by inspecting the actual binary, not assumed from the build succeeding.

This is a config-and-secrets addition to the existing pipeline shape, not a rewrite of it.

---

## Android signing — done, and the reasoning is different from Windows

Android signing is **already in place**, unlike Windows. The two are not comparable, and
conflating them caused a real shipped defect worth recording.

**Windows:** signing is optional and costs money. An unsigned installer runs; SmartScreen simply
warns. Shipping unsigned is a real but *degraded* experience.

**Android:** signing is mandatory and free. The package installer refuses an unsigned APK
outright — "package appears to be invalid" / "There was a problem parsing the package" — so an
unsigned APK is not degraded, it is **completely uninstallable**. `v0.1.1-beta` through
`v0.1.4-beta` each shipped an APK built as `app-universal-release-unsigned.apk` and labelled
"unsigned" in the release notes by analogy with the Windows build. That label was accurate and
the artifact was still useless: nobody could install any of them. Confirmed after the fact with
`apksigner verify`, which reported `DOES NOT VERIFY — Missing META-INF/MANIFEST.MF`.

**The Android release sequence, in order:**

```powershell
npm run sync:android-icons   # MUST run before build -- see why below
cargo tauri android build --apk --ci
npm run sign:apk             # unsigned APKs cannot install at all -- see below
```

`sync:android-icons` exists for the same "the generated project forgets what
this repo already fixed" reason as signing does: `cargo tauri android init`
scaffolds `src-tauri/gen/android/` from Tauri's own template, which ships its
own placeholder launcher icon (an orange/teal "8" mark), not the branded set
already sitting in `src-tauri/icons/android/`. `v0.1.5-beta` and every
release before it shipped an APK that ran correctly but showed that
placeholder on the home screen and app switcher instead of Envryn's mark.
`.dev-tools/sync-android-icons.mjs` copies the real icons over.

**It must run before `android build`, not after.** Gradle packages whatever
PNGs are sitting in `res/mipmap-*/` at build time; signing only wraps the
already-built APK afterward and cannot change what's inside it. Confirmed
empirically, not assumed: across three separate `cargo tauri android build`
runs on three different version bumps, the wrong (placeholder) icon files in
`gen/android/.../res/` never changed timestamp -- proving the build step
itself never touches them, so a sync run *after* the build has already
happened has no effect on that build's APK at all. Running it first, once
`gen/android` exists, is enough for every following build in the same
checkout, since the build step doesn't overwrite it back to the placeholder
either -- but run it every time to be certain, especially after any
`cargo tauri android init`.

**How it works now.** `.dev-tools/sign-apk.mjs` (`npm run sign:apk`) zipaligns and then signs the
Gradle output with APK Signature Scheme v2 + v3, and **fails the build if the result does not
verify** — so a silently-unsigned artifact cannot reach a release again. `minSdk` is 24, so v1
(JAR) signing is not required; v2+v3 is what every supported Android version verifies against.

**The keystore is the single irreplaceable artifact in this release process.**

- It lives at `~/.envryn/envryn-release.jks`, deliberately **outside the repository**, so it
  cannot be committed by accident. `.gitignore` also refuses `*.jks`/`*.keystore` as defence in
  depth.
- Android permanently binds an installed app's identity to its signing key. An update signed
  with a **different** key cannot install over an existing installation, and there is **no
  recovery path** — not through Google, not through the OS. Losing this file means every
  existing user must uninstall (destroying nothing but the app itself; the vault survives, see
  the README) before they can install any future version.
- **Back it up somewhere durable and offline**, along with its password. Treat it exactly like
  the root of a credential you cannot rotate — because that is what it is.

Self-signed is the correct and complete answer here. Android does not require a CA-chained
certificate for sideloading; it only requires that a package carry *a* valid, consistent
signature. There is no paid upgrade path that would improve this, and no equivalent of
SmartScreen reputation to earn. Only Google Play distribution would add further requirements,
and that is out of scope for this project.
