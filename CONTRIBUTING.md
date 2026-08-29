# Contributing to Envryn

Thanks for taking the time to help. Envryn handles sensitive data, so small, well-tested changes are more useful than large rewrites that are hard to review.

## Before you start

- Search existing issues and pull requests.
- Open an issue before a large feature, storage change, cryptography change, or sync protocol change.
- Use GitHub private vulnerability reporting for security issues. Do not publish exploit details in a public issue.

## Set up the project

You need Node.js 20 or newer, Rust stable, and the Tauri prerequisites for Windows.

```powershell
git clone https://github.com/Gr33nOps/Envryn.git
cd Envryn
npm ci
git config core.hooksPath .githooks
```

Run the app with:

```powershell
npm run tauri:dev
```

## Make a change

- Keep security decisions in `envryn-core`, not the React interface.
- Do not add network access without an issue and threat-model review.
- Never put real credentials in fixtures, screenshots, commits, logs, or examples.
- Keep generated bindings in sync by running the Rust tests.
- Add or update tests for behavior changes.
- Update documentation when behavior, setup, or support changes.
- Do not use em dashes in project documentation.

## Validate locally

At minimum:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
npm run lint
npm run typecheck
npm run test:coverage --workspace @envryn/ui
npm run test:e2e
npm run test:security-invariants
```

The pre-push hook runs the main local gate. CI adds dependency, secret, static-analysis, accessibility, and bundle checks.

## Pull requests

A good pull request:

- Explains the problem in plain language
- Describes the chosen fix and important tradeoffs
- Lists the checks that were run
- Includes screenshots for interface changes
- Calls out storage, cryptography, sync, permission, or privacy impact
- Avoids unrelated formatting or refactoring

Use a clear commit message such as `fix: keep the sync listener alive` or `docs: clarify Android installation`.

By contributing, you agree that your contribution is licensed under the MIT License.
