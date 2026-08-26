# Envryn — Clean-Machine Release Test Checklist

**Purpose:** every check below needs a Windows machine that has never had Envryn, its
dependencies, or this repo's build tools on it — a fresh VM snapshot, reverted after each run.
This session's environment cannot perform any of this: it is the developer's own machine, with
Rust/Node/WebView2/build tools already present, which is exactly what these checks need to be
*absent* to mean anything. Nothing in this checklist has been run yet.

**Snapshot discipline:** revert to a clean snapshot before *every* run of this checklist, not
just the first. A machine that has already installed and uninstalled Envryn once is no longer a
clean machine for testing first-install or leftover-file behavior.

**Target machine:** Windows 10 1809+ or Windows 11, no prior WebView2 runtime assumed (test both
with and without it pre-installed — see §1), standard (non-admin-by-default) user account.

---

## 0. Before you start

- [ ] Snapshot is confirmed clean (no prior Envryn install, no dev tools required to *run* the
      app — WebView2 is the one exception, tested deliberately in both states below).
- [ ] Network capture tool ready (Wireshark, or even just Resource Monitor's network tab) running
      *before* the installer is launched, so nothing is missed between download and first launch.
- [ ] Have both installer artifacts ready to test independently: the MSI and the NSIS `.exe`.

## 1. Installation

- [ ] **Without WebView2 pre-installed:** run the installer. Confirm `webviewInstallMode:
      downloadBootstrapper` actually triggers a WebView2 download+install rather than failing or
      silently proceeding with a broken window.
- [ ] **With WebView2 already installed:** confirm the installer does not reinstall/downgrade it.
- [ ] NSIS installer: confirm it does **not** prompt for administrator elevation (per-user
      `CurrentUser` install mode is the default — see `RELEASE_SIGNING.md`'s sibling review) and
      installs to a per-user location, not `Program Files`.
- [ ] MSI installer: confirm it **does** prompt for UAC elevation (WiX/MSI's standard per-machine
      behavior) and installs under `Program Files`.
- [ ] Record the literal install path each installer chose.
- [ ] Confirm a Start Menu shortcut is created and launches the app.
- [ ] Confirm no SmartScreen bypass beyond the expected "unknown publisher" warning (expected and
      correct right now — the binaries are genuinely unsigned, see `RELEASE_SIGNING.md`). The
      warning itself, appearing, is the *expected pass* for this specific check pre-signing.

## 2. First launch

- [ ] App opens to the vault-creation screen (no vault exists yet on a clean machine).
- [ ] Window chrome renders correctly (custom frameless title bar, resize borders, dark theme,
      background color) — this is the exact class of bug the border/shadow issue earlier this
      project had; a clean machine with no dev-time WebView2 profile cache is a real test of it.
- [ ] Create a vault with a real password. Confirm the password-strength meter renders and
      updates live (a real fix from the earlier security pass — verify it shipped, don't assume).
- [ ] Confirm `%APPDATA%\dev.envryn.vault\` (or wherever `app_data_dir()` actually resolves for
      this identifier) is created only *after* vault creation, not at first launch before that.

## 3. Vault lifecycle

- [ ] Add one of each secret type (API Key, Token, Env Var, Note, Database, SSH, OAuth, Webhook,
      Custom) and confirm each round-trips through create → list → reveal correctly.
- [ ] Search, edit, and delete a secret; confirm the list updates correctly each time.
- [ ] Lock the vault (`Ctrl+L` or the Lock button). Confirm every secret-bearing view immediately
      shows locked/no-data state — no stale plaintext left rendered anywhere in the DOM.
- [ ] Unlock with the correct password; confirm all secrets are exactly as left.
- [ ] Unlock with a **wrong** password; confirm the error message does not distinguish "wrong
      password" from any other failure mode (INV-006 — worth a real, not just source-level, check).

## 4. Clipboard expiry

- [ ] Copy a secret's value. Confirm the configured clear countdown (Settings, default 30s)
      actually clears the OS clipboard when it elapses.
- [ ] Copy a secret, then copy something *else* (e.g. a browser URL) before the timer elapses.
      Confirm Envryn's timer does **not** clear the new clipboard content — it must only clear if
      the clipboard still holds exactly what Envryn put there (this exact behavior has a real
      unit test in the codebase; this is the live, OS-level confirmation of it).
- [ ] Confirm the copied value never appears in clipboard history tools (Win+V) in a way that
      survives past the app's own exclusion tag — Envryn tags clipboard writes to exclude them
      from monitoring; verify Win+V clipboard history genuinely doesn't retain it.

## 5. Backup / restore

- [ ] Create a backup to a chosen file path. Confirm the file is created and open it in a hex
      viewer — no plaintext secret values, names, or notes should be readable in it.
- [ ] Restore from that backup into a fresh vault with a new master password. Confirm every
      secret is recovered correctly and the new vault uses the new password, not the original or
      the backup password.
- [ ] Attempt a restore with the wrong backup password; confirm it fails cleanly with no partial
      import.

## 6. Lock / unlock and idle behavior

- [ ] Leave the app idle (no mouse/keyboard input) past the configured auto-lock threshold;
      confirm it locks itself.
- [ ] Lock the Windows session directly (`Win+L`) while Envryn is unlocked; confirm Envryn locks
      itself too (the `WTS_SESSION_LOCK` hook) — this is explicitly documented as unverified on
      real hardware in `docs/ARCHITECTURE.md`; this is the first real chance to verify it.
- [ ] If Windows Hello / platform protection is enabled: confirm unlocking via the platform slot
      genuinely requires the biometric/PIN prompt to succeed first.

## 7. Restart behavior

- [ ] With the vault unlocked, fully quit and relaunch Envryn. Confirm it starts **locked** —
      there is deliberately no persisted "stay unlocked" state across process restarts.
- [ ] Reboot the whole VM with Envryn set to launch at logon (if that's ever enabled) or launched
      manually after reboot; confirm the vault file survives intact and unlocks normally.

## 8. Uninstall

- [ ] Uninstall via each installer's own mechanism (Settings → Apps, or `Add/Remove Programs` for
      the MSI). Confirm it completes without error.
- [ ] Confirm the Start Menu shortcut is removed.
- [ ] **Deliberately check whether the vault database and its `-wal`/`-shm` files survive
      uninstall or are deleted.** Neither Tauri's default WiX nor NSIS templates delete
      `app_data_dir()` on uninstall unless a project explicitly adds an uninstall hook to do so —
      Envryn has not (no `NSIS_HOOK_POSTUNINSTALL`/custom `.wxs` component targeting it was found
      in this review). **Expected/current behavior: user data survives uninstall.** Confirm this
      is what actually happens, and treat any *unexpected* deletion during uninstall as a real
      data-loss bug, not a feature.
- [ ] Confirm DPAPI-protected platform-slot data (if platform protection was enabled) doesn't
      leave orphaned Windows Credential Manager / DPAPI blobs elsewhere.

## 9. Leftover files

After uninstall, with the VM still on the same (now post-uninstall) state:

- [ ] Diff a full filesystem listing of `%LOCALAPPDATA%`, `%APPDATA%`, `%PROGRAMFILES%` (or
      wherever the per-user/per-machine install went, per §1) against a pre-install baseline.
      Expect: the vault data directory (if user data preservation is confirmed correct in §8),
      and nothing else.
- [ ] Confirm no registry keys survive under `HKCU`/`HKLM` beyond what the installer's own
      uninstall metadata legitimately needs to have already cleaned (NSIS/WiX both remove their
      own registry entries on a clean uninstall by design — verify this actually happened, don't
      assume).
- [ ] Confirm the WebView2 runtime installed in §1 is *not* removed by Envryn's uninstaller (it's
      a shared system component, not Envryn's to manage) — and equally, confirm Envryn's own
      uninstaller doesn't error out trying to touch it.

## 10. Unexpected network connections

- [ ] With the network capture from §0 running for the *entire* session (install through
      uninstall), review it afterward for any connection that is **not**:
  - the WebView2 bootstrapper download (§1, install time only, one-time)
  - a Hugging Face HTTPS request, and only if the AI model download was explicitly triggered by
    the user (`ai_download_model`) — confirm this by *not* triggering it in one full pass of this
    checklist, and confirming zero network activity occurs, then triggering it in a second pass
    and confirming the only activity is the two expected model file downloads
  - explicit user-initiated sync/pairing traffic (mDNS + TLS to another paired device), only if
    that flow is deliberately exercised
- [ ] Any other outbound connection — telemetry-shaped, unexpected DNS lookups, anything to a
      host that isn't `huggingface.co` or a local peer — is a real finding, not a false positive,
      and should stop this checklist and be investigated before release.

---

## What this checklist cannot substitute for

Running this on one clean VM snapshot is a single sample, not a guarantee across the full
Windows 10/11 version and locale matrix. Treat a full pass as strong evidence, not proof, the
same way this project's own `SECURITY_REMEDIATION_REPORT.md` treats every other check.
