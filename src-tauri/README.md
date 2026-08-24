# src-tauri

The application shell. Thin by design: it owns window creation and the IPC
surface, and nothing else. Every security decision lives in
[`crates/envryn-core`](../crates/envryn-core), which has no dependency on Tauri
and can therefore be tested without a windowing system.

## The CSP is a real control

`tauri.conf.json` sets a restrictive Content-Security-Policy. This is not
boilerplate copied from a template.

A webview application can exfiltrate its entire contents with one line of
JavaScript, and Envryn's central claim to users is that it does not talk to
anything (INV-010). `connect-src` permits only the Tauri IPC channel, so there
is no remote origin the page can reach even if a dependency tries. Fonts and
images are self-hosted, so no external host appears anywhere in the policy.

There are three independent layers enforcing the same property, because any one
of them can be weakened by a well-meaning change:

| Layer | Catches |
|---|---|
| CSP here | A runtime attempt to reach a remote host |
| ESLint (`no-restricted-globals`) | `fetch`/`WebSocket`/`XMLHttpRequest` in UI source at build time |
| `envryn-core` dependency graph | A network client entering the Rust core at all |

## Capabilities are minimal

`capabilities/default.json` grants `core:default` and nothing further. The UI
needs no filesystem, shell, HTTP, or clipboard plugin permission, because every
such operation is performed by the Rust core behind a named IPC command.

If a future change adds a plugin permission here, that is a security review
point, not a routine edit.

## IPC rules

See the module documentation in [`src/ipc.rs`](src/ipc.rs). In short:

- no command returns a key, KDF parameter, or wrapped blob;
- no command accepts a filesystem path, so a compromised webview cannot
  redirect reads or writes;
- listing returns summaries that structurally cannot hold secret material;
  revealing is a separate, single-record call with no bulk form;
- errors do not distinguish "no vault" from "wrong password" (INV-006).
