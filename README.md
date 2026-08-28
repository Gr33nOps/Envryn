<div align="center">

<img src="docs/assets/logo.png" alt="Envryn" width="88" height="88">

# Envryn

**A local-first secrets vault for developers.**
API keys, tokens, database credentials, SSH keys, and other credentials —
encrypted, stored only on your own machine, and never sent anywhere by default.

[![CI](https://github.com/Gr33nOps/Envryn/actions/workflows/ci.yml/badge.svg)](https://github.com/Gr33nOps/Envryn/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/Gr33nOps/Envryn?include_prereleases&label=release&color=2ea043)](https://github.com/Gr33nOps/Envryn/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-informational)](LICENSE)
[![Platform: Windows](https://img.shields.io/badge/platform-Windows-0a84ff)](#installing)
[![Platform: Android (experimental)](https://img.shields.io/badge/platform-Android%20(experimental)-3ddc84)](#installing)
[![Status: beta](https://img.shields.io/badge/status-beta-orange)](#project-status)

</div>

<br>

## Contents

[Why Envryn](#why-envryn) ·
[Features](#features) ·
[Installing](#installing) ·
[Uninstalling](#uninstalling-and-removing-your-data) ·
[Updating](#manual-updates-no-auto-updater-in-v01x) ·
[Development](#development) ·
[Architecture](#architecture-in-brief) ·
[Security docs](#security-documentation) ·
[Project status](#project-status)

---

## Why Envryn

Most "secrets managers" are either a plaintext `.env` file you hope nobody
commits, or a cloud service you have to trust with everything you store in
it. Envryn is neither:

- **Local-first.** Your vault is a single encrypted file on your own disk.
  There is no server, no account, and no cloud sync unless you explicitly
  pair a second device of your own over your local network.
- **Encrypted at rest, for real.** XChaCha20-Poly1305 for records, Argon2id
  for your master password, HKDF-SHA256 for subkey derivation. See
  [`docs/CRYPTOGRAPHY.md`](docs/CRYPTOGRAPHY.md) for the exact construction
  — not a marketing paragraph, the actual primitives and why each was chosen.
- **Fails closed.** The optional local-AI subsystem, sync, and every other
  non-essential feature can be deleted from the build entirely and the vault
  still works — locking, unlocking, storing, and retrieving secrets never
  depends on any of them. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
- **Honest about its limits.** Envryn has been through a real internal
  security audit ([`AUDIT_REPORT.md`](AUDIT_REPORT.md)) covering static
  analysis, dependency scanning, secret scanning, and manual review. It has
  **not** had an independent third-party audit, and no software is
  unhackable — see [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md) for what
  is and isn't defended against.

## Features

- Store API keys, tokens, environment variables, database credentials, SSH
  keys, OAuth secrets, webhooks, and free-form secure notes
- Organize by project and environment (Development / Staging / Production)
- Deterministic credential-type detection for ~25 well-known providers
  (Stripe, GitHub, AWS, OpenAI, Slack, and more), no network call involved
- Optional **local-only AI** for name suggestions, classification, and
  natural-language search — runs a small model entirely on-device via
  [candle](https://github.com/huggingface/candle); off by default, and the
  one-time model download is the *only* network access it ever makes (see
  [`docs/AI_SECURITY.md`](docs/AI_SECURITY.md) and
  [`docs/AI_DATA_ACCESS.md`](docs/AI_DATA_ACCESS.md))
- Direct device-to-device sync over your local network, with explicit
  pairing and a human-verified confirmation code — never a third-party relay
- Encrypted, password-protected backup and restore
- Auto-lock on idle, clipboard auto-clear, and optional Windows Hello unlock
- A native, custom-decorated desktop window — no browser, no Electron

## Installing

Download the latest release from
**[Releases](https://github.com/Gr33nOps/Envryn/releases)**.

**Windows** (primary platform): either the `.msi` or the `-setup.exe`
(NSIS) installs the same app.

**Android** (experimental): a universal `.apk`, signed with Envryn's own
release key. Android shows it as from an unknown developer, so you'll need to
allow installs from an unknown source — but it will install. This build has
been verified to compile and run its full test suite, but hasn't yet been
through the same device-level testing as the Windows build — treat it as
early. Android has no local AI; that feature is Windows-only.

> If you installed the `v0.1.4-beta` APK or earlier and got "package appears
> to be invalid", that was our bug, not yours: those APKs were published
> genuinely unsigned, which Android refuses outright. Use `v0.1.5-beta` or
> later.

> Envryn is in beta. Use it for real credentials once you're comfortable —
> most people start with lower-stakes or easily-rotated ones while they get
> a feel for it.

## Uninstalling and removing your data

**Uninstalling Envryn does not delete your vault.** This is deliberate: an uninstall should
never be a silent, irreversible way to lose your secrets. Both installers only remove the
program files they placed; your encrypted vault lives in a completely separate, standard Windows
per-user data folder that neither installer's uninstaller touches.

If you want your data gone too, not just the app, remove it yourself after uninstalling:

1. Press `Win+R`, type `%APPDATA%`, press Enter.
2. Delete the `dev.envryn.vault` folder you find there. This holds your encrypted vault database
   (`envryn.db` and its `-wal`/`-shm` files), your device identity, your settings, and — if you
   ever enabled the local AI feature — the downloaded model files. (This folder exists even if
   you never created a vault — Envryn creates it within seconds of any launch to store your
   auto-lock/clipboard preferences, which have to be readable before a vault exists. If you never
   created a vault, it holds nothing sensitive.)
3. Optionally, also check `%LOCALAPPDATA%\dev.envryn.vault` (press `Win+R`, type
   `%LOCALAPPDATA%`, Enter). If present, this holds the embedded browser engine's own cache and
   metrics data — never your secrets (Envryn's UI never holds key material; see
   [`docs/SECURITY_INVARIANTS.md`](docs/SECURITY_INVARIANTS.md)) — but delete it too if you want
   every trace gone.

Because your vault is encrypted at rest with your master password (see
[`docs/CRYPTOGRAPHY.md`](docs/CRYPTOGRAPHY.md)), a normal file deletion is enough — there is no
plaintext copy sitting elsewhere on disk to separately shred. If your threat model specifically
includes forensic recovery of deleted files from your own drive (a different, much narrower
concern than losing the master password), that's a general Windows/full-disk-encryption question
independent of Envryn, not something this app can solve for you after the fact.

## Manual updates (no auto-updater in v0.1.x)

Envryn does not check for or install updates automatically — see
[`docs/UPDATE_POLICY.md`](docs/UPDATE_POLICY.md) for why that's a deliberate choice for now, not
a missing feature. To update:

1. Get the new installer **only** from this repository's
   [Releases page](https://github.com/Gr33nOps/Envryn/releases) — never a link from anywhere
   else.
2. Verify its SHA-256 checksum against the value published in that release's notes **before**
   running it (PowerShell: `Get-FileHash .\the-installer-file -Algorithm SHA256`).
3. Run the new installer directly over your existing install. It upgrades in place and does not
   touch your vault, which lives outside the install directory (see "Uninstalling" above).

## Development

Requires [Node.js](https://nodejs.org) 20+ and the
[Rust toolchain](https://rustup.rs) with the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for Windows.

```sh
git clone https://github.com/Gr33nOps/Envryn.git envryn
cd envryn
npm install

npm run tauri:dev      # run the app in development mode
npm run tauri:build    # produce a real installer (.msi and .exe)
```

Other useful commands:

```sh
cargo test --workspace              # Rust test suite
cargo clippy --workspace --all-targets -- -D warnings
npm run typecheck                   # frontend
npm run lint                        # frontend
```

A local pre-commit/pre-push security gate (formatting, linting, the full
test suite) runs automatically once you've cloned the repo — see
`.githooks/`.

## Architecture, in brief

```
apps/ui/            React + TanStack Router frontend (untrusted by design —
                     every sensitive operation is enforced in Rust, never
                     assumed safe because the UI asked nicely)
crates/envryn-core/  Vault, crypto, sync, and platform logic. No Tauri
                     dependency — testable and reasoned about on its own.
crates/envryn-ai-worker/
                     The optional local-AI inference process. No dependency
                     on envryn-core; killing this process leaves a fully
                     functional vault.
src-tauri/           The Tauri shell: IPC commands, window/platform glue,
                     and nothing that a security decision should live in.
packages/contract/   TypeScript types generated directly from the real Rust
                     IPC types (ts-rs) — the frontend can't drift from what
                     the backend actually sends.
```

Full detail: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Security documentation

- [Security and privacy testing](docs/SECURITY_TESTING.md) — free local scanners, APK analysis, fuzzing, and the release gate.
- [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md) — what Envryn defends
  against, and what it explicitly does not
- [`docs/CRYPTOGRAPHY.md`](docs/CRYPTOGRAPHY.md) — the exact primitives and
  key hierarchy
- [`docs/SECURITY_INVARIANTS.md`](docs/SECURITY_INVARIANTS.md) — properties
  the codebase is designed to never violate
- [`SECURITY_ARCHITECTURE.md`](SECURITY_ARCHITECTURE.md) /
  [`SECURITY_CHECKLIST.md`](SECURITY_CHECKLIST.md) — the security posture
  and review checklist
- [`AUDIT_REPORT.md`](AUDIT_REPORT.md) — findings and remediation from the
  internal security audit, including confirmed false positives and what
  still needs human judgment

Found a real security issue? Use [private vulnerability reporting](https://github.com/Gr33nOps/Envryn/security/advisories/new); do not put exploit details or sensitive data in a public issue.

## Project status

Beta. Core vault functionality (create, lock/unlock, store, search, backup,
restore, device sync) is real, tested, and has been through a full internal
audit and a real production-build install/uninstall cycle. Rougher edges:

- Windows is the primary, most-tested platform; no code signing yet, so
  Windows SmartScreen will warn on first run
- Android is experimental: it builds and passes the full test suite, but
  hasn't had the same device-level verification as Windows yet
- Local AI is an optional, early feature — quality reflects the small
  on-device model it deliberately uses to stay GPU-free
- No auto-updater by design for now; update by downloading the latest
  release

---

## License

[MIT](LICENSE)
