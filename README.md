# Envryn

**A local-first secrets vault for developers.** API keys, tokens, database
credentials, SSH keys, and other credentials — encrypted, stored only on
your own machine, and never sent anywhere by default.

[![CI](https://github.com/Gr33nOps/local-vault-for-devs/actions/workflows/ci.yml/badge.svg)](https://github.com/Gr33nOps/local-vault-for-devs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-informational)](LICENSE)
[![Platform: Windows](https://img.shields.io/badge/platform-Windows-0a84ff)](#installing)
[![Status: beta](https://img.shields.io/badge/status-beta-orange)](#project-status)

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

Download the latest installer from
**[Releases](https://github.com/Gr33nOps/local-vault-for-devs/releases)**
— either the `.msi` or the `-setup.exe` (NSIS) works the same way.

Envryn currently targets **Windows**. Support for other platforms may
follow but isn't a near-term goal.

> Envryn is in beta. Use it for real credentials once you're comfortable —
> most people start with lower-stakes or easily-rotated ones while they get
> a feel for it.

## Development

Requires [Node.js](https://nodejs.org) 20+ and the
[Rust toolchain](https://rustup.rs) with the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for Windows.

```sh
git clone https://github.com/Gr33nOps/local-vault-for-devs.git envryn
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

Found a real security issue? Please open an issue describing the class of
problem without public exploit details, or reach out directly — there is no
dedicated security contact address set up yet.

## Project status

Beta. Core vault functionality (create, lock/unlock, store, search, backup,
restore, device sync) is real, tested, and has been through a full internal
audit and a real production-build install/uninstall cycle. Rougher edges:

- Windows only; no code signing yet, so Windows SmartScreen will warn on
  first run
- Local AI is an optional, early feature — quality reflects the small
  on-device model it deliberately uses to stay GPU-free
- No auto-updater by design for now; update by downloading the latest
  release

## License

[MIT](LICENSE)
