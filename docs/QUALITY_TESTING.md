# Quality testing

Envryn uses only free, open-source test tooling and GitHub-hosted Actions for
the public repository. The suite is layered so a failure points to the
smallest useful surface instead of relying on one large end-to-end test.

## Automated test matrix

| Layer                       | Target                                                                                            | Command                                        | CI platform       |
| --------------------------- | ------------------------------------------------------------------------------------------------- | ---------------------------------------------- | ----------------- |
| Rust unit and integration   | Vault lifecycle, storage, crypto, pairing, sync, conflicts, and local AI boundaries               | `cargo test --workspace`                       | Windows           |
| Frontend unit and component | React behavior, state, forms, navigation, native-command adapters, and a coverage baseline        | `npm run test:coverage --workspace @envryn/ui` | Linux             |
| Browser journey             | Vault onboarding through the responsive application shell                                         | `npm run test:e2e`                             | Linux             |
| Responsive layout           | Desktop (1440 x 900) and Pixel 7-sized Android viewport, including horizontal-overflow protection | `npm run test:e2e`                             | Linux             |
| Accessibility               | WCAG 2.0/2.1 A and AA automated checks, failing on serious or critical findings                   | `npm run test:e2e`                             | Linux             |
| Bundle budget               | Largest JavaScript chunk, total JavaScript, and total CSS regression limits                       | `npm run test:bundle-budget`                   | Linux             |
| Static quality              | ESLint, TypeScript, formatting, Clippy, and generated contract drift                              | CI commands in `.github/workflows/ci.yml`      | Linux and Windows |
| Security and privacy        | Dependency, secret, static-analysis, invariant, fuzz, and APK checks                              | See `SECURITY_TESTING.md`                      | Linux and Windows |

Playwright runs the same application code used by the Tauri shells. Its test
boundary mocks native commands deterministically; Rust integration tests cover
the actual command-side behavior and protocol implementation. This keeps the
browser suite fast and repeatable while preserving native coverage at the
lower layer.

When a browser test fails in CI, its Playwright report, trace, and failure
screenshot are retained for seven days as a downloadable workflow artifact.

## Local commands

```sh
npm ci
npm run lint
npm run typecheck
npm test --workspace @envryn/ui
npm run test:e2e
npm run build
npm run test:bundle-budget
cargo test --workspace
```

Install the browser once on a new development machine with:

```sh
npx playwright install chromium
```

On Linux CI or a fresh Linux workstation, use `npx playwright install
--with-deps chromium` to install its operating-system dependencies too.

## What remains device-level

Browser emulation verifies layout and user journeys, not Android WebView,
Windows WebView2, radio behavior, or operating-system lifecycle rules. Before
a release, retain these physical checks:

- Pair a clean Windows vault with a real Android device, sync in both
  directions, edit both sides offline, reconnect, and resolve a conflict.
- Lock, background, resume, rotate, and force-stop the Android app; confirm no
  decrypted content remains visible and the vault recovers correctly.
- Exercise Wi-Fi changes, denied permissions, battery saver, and temporary
  peer loss, then confirm the interface explains recovery without data loss.
- Install and launch the signed release artifacts on supported Windows and
  Android versions.

These checks should not be represented as automated until they run on real
hardware. Emulator/device automation can be added later without replacing the
physical Windows-to-Android release pass.
