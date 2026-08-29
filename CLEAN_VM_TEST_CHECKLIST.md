# Envryn - Clean-Machine Release Test Checklist

**Who this is for:** anyone with a Windows VM, following these steps exactly, in order - no
Envryn source code, no build tools, and no prior knowledge of the project required. Every command
below is either a Windows built-in or explicitly says what to install first.

**What "clean" means:** a Windows VM snapshot that has never had Envryn installed on it, and is
reverted to that same snapshot before *every* run of this checklist - not just the first time.
A machine that already has an old Envryn install, an old vault folder, or a prior run's leftover
files is no longer a valid test of first-install or leftover-file behavior.

**Why this can't be done on the developer's own machine:** that machine already has Rust,
Node.js, WebView2, and every build tool installed, plus prior Envryn installs and test data - the
opposite of what these checks need to be a real test.

**Target VM:** Windows 10 (version 1809 or later) or Windows 11, a standard (non-administrator)
user account, and - deliberately - no WebView2 Runtime pre-installed for the first pass (§1
covers testing both states).

---

## 0. Before you start

- [ ] Confirm the VM snapshot is clean (see "What 'clean' means" above).
- [ ] Get the two installer files onto the VM (copy them in, or download from wherever they were
      shared - see `BETA_RELEASE_CHECKLIST.md` for where a real beta build's artifacts and their
      checksums are published). You need both: the `.msi` and the `...-setup.exe` (NSIS).
- [ ] **Verify each installer's checksum before running anything.** Open PowerShell (Start menu →
      type `PowerShell` → Enter) and run, for each file:
      ```powershell
      Get-FileHash .\Envryn_x.y.z_x64_en-US.msi -Algorithm SHA256
      Get-FileHash .\Envryn_x.y.z_x64-setup.exe -Algorithm SHA256
      ```
      Compare the `Hash` value printed against the checksum published alongside the build you
      were given. If they don't match, stop - do not install a file whose checksum doesn't match.
- [ ] Start a network capture *before* running either installer, so nothing between download and
      first launch is missed. No download needed - Windows has a built-in packet monitor:
      ```powershell
      # Run PowerShell as Administrator for this. --pkt-size 0 logs full packets,
      # not just headers; you'll stop it later in §10.
      pktmon start --capture --pkt-size 0 --file-name "$env:USERPROFILE\Desktop\envryn-capture.etl"
      ```
      (If Wireshark is available and preferred instead, that works too - the point is *something*
      is capturing for the whole session, not the specific tool.)

## 1. Installation

Test the MSI and the NSIS installer as two **separate, fully clean** passes (revert the VM
snapshot between them) - they behave differently and both need their own clean-machine test.

- [ ] **Pass A - no WebView2 pre-installed:** confirm the VM genuinely has no WebView2 Runtime
      first (`Settings → Apps → Installed apps`, search "WebView2" - should show nothing). Run
      the installer. It should download and install WebView2 automatically before Envryn opens,
      not fail and not silently show a broken/blank window.
- [ ] **Pass B - WebView2 already installed:** install WebView2 Runtime first
      (https://developer.microsoft.com/microsoft-edge/webview2/ - "Evergreen Bootstrapper"), then
      run the Envryn installer. Confirm it does not reinstall, downgrade, or otherwise disturb the
      existing WebView2 install.
- [ ] **NSIS installer specifically:** confirm it does **not** show a Windows "Do you want to
      allow this app to make changes to your device?" (UAC) prompt, and confirm (Properties on
      the installed `.exe`, or just note the path the installer showed) that it installed under
      `%LOCALAPPDATA%\Envryn`, not `Program Files`.
- [ ] **MSI installer specifically:** confirm it **does** show the UAC prompt, and that it
      installed under `C:\Program Files\Envryn`.
- [ ] Confirm a Start Menu entry named "Envryn" exists and launches the app.
- [ ] Confirm you see a Windows SmartScreen warning ("Windows protected your PC") the first time
      you run either installer. **This is expected and correct right now** - see
      `RELEASE_SIGNING.md`; the build is genuinely unsigned. Seeing this warning is a *pass* for
      this specific check, not a bug. (What it should say is covered by whatever label the actual
      release build used - see `BETA_RELEASE_CHECKLIST.md`.)

## 2. First launch

- [ ] The app opens directly to a "create your vault" screen - nothing else, since no vault
      exists yet on a clean machine.
- [ ] The window looks correct: dark theme, no stray white/default window border or title bar
      (a real bug this project fixed once already - this is the first time a truly clean WebView2
      profile has tested it), and the app resizes and can be dragged/moved normally.
- [ ] Create a vault: type a real password. Confirm a strength indicator appears and visibly
      changes as you type different passwords (weak → strong).
- [ ] Open File Explorer, type `%APPDATA%` in the address bar, press Enter. Confirm a
      `dev.envryn.vault` folder exists. **Note:** this folder is created within a few seconds of
      *any* launch, before you create a vault - the auto-lock background timer reads app settings
      (auto-lock minutes, clipboard-clear seconds) on every tick, and creating that folder is a
      side effect of checking for a settings file that may not exist yet. This is expected,
      deliberate behavior (see `INSTALLER_REVIEW.md`), not a sign anything vault-related has
      happened - the folder holds no secret data until you actually create a vault.

## 3. Vault lifecycle

- [ ] Add one secret of *each* available type (API Key, Token, Env Var, Note, Database, SSH,
      OAuth, Webhook, Custom). For each: save it, find it in the list, click to reveal its value,
      and confirm the revealed value matches exactly what you typed.
- [ ] Use the search box to find one of them by name. Edit it (change the value), save, and
      confirm the new value sticks. Delete one entirely and confirm it disappears from the list.
- [ ] Lock the vault (`Ctrl+L`, or the Lock button in the sidebar). Confirm every screen
      immediately stops showing any secret value or list - a locked vault should show nothing
      readable anywhere on screen.
- [ ] Unlock with the correct password. Confirm every secret you added is still there, unchanged.
- [ ] Lock again, then try unlocking with a **wrong** password on purpose. Confirm the error
      message is generic (something like "authentication failed"), not something that gives away
      *why* it failed differently for a wrong password versus any other problem.

## 4. Clipboard expiry

- [ ] Reveal a secret and click "copy". Note the app's stated auto-clear time (Settings screen,
      default 30 seconds). Paste into Notepad immediately to confirm the copy worked, then wait
      out the full countdown *without pasting again*, then try pasting into Notepad again -
      nothing should paste (clipboard was cleared).
- [ ] Copy a secret again, but this time, before the countdown finishes, copy something unrelated
      instead (select and copy any text from a browser or Notepad). Wait past when the original
      countdown would have finished. Confirm the *new* clipboard content is still there - Envryn
      must not clear a clipboard it no longer put there itself.
- [ ] Copy a secret, then press `Win+V` to open Windows' Clipboard History. Confirm the secret
      value does not appear in that history list.

## 5. Backup / restore

- [ ] In Envryn, create a backup and save it somewhere findable (e.g. Desktop), with a backup
      password you'll remember. Confirm a file is created.
- [ ] Open that backup file in a plain text/hex viewer to eyeball it for plaintext leakage -
      PowerShell has a built-in hex dump, no extra tool needed:
      ```powershell
      Format-Hex .\your-backup-file | more
      ```
      Skim the right-hand ASCII column: none of your secret values, names, or notes should be
      readable there.
- [ ] Restore that backup into a brand-new vault (create a new vault first if needed, with a
      *different* master password than either the original vault or the backup password). Confirm
      every secret comes back correctly, and that the new vault only accepts its own new
      password - not the original vault's password, not the backup's password.
- [ ] Try restoring the same backup file with the *wrong* backup password on purpose. Confirm it
      fails cleanly with an error, and that nothing partial got imported.

## 6. Lock / unlock and idle behavior

- [ ] Leave the app open and unlocked, and don't touch the mouse or keyboard until the configured
      auto-lock time passes (check Settings for the exact value). Confirm it locks itself with no
      further action from you.
- [ ] With the vault unlocked, press `Win+L` to lock the Windows session itself (not just the
      app). Log back into Windows and check Envryn - confirm it locked itself too. This is the
      first real, physical-hardware test of that behavior; note the result either way.
- [ ] If Windows Hello is available on the VM and you enable Envryn's platform-unlock option:
      confirm that unlocking this way genuinely requires the Windows Hello prompt to succeed -
      it shouldn't be possible to skip straight to an unlocked vault.

## 7. Restart behavior

- [ ] With the vault unlocked, fully close Envryn (not just lock it) and reopen it. Confirm it
      comes back **locked** - Envryn should never remember "stay unlocked" across a real restart
      of the app.
- [ ] Restart the whole VM. Reopen Envryn afterward and confirm your vault and all its secrets
      are still there and unlock normally.

## 8. Uninstall

- [ ] Uninstall via `Settings → Apps → Installed apps → Envryn → Uninstall` (works for both
      installer types). Confirm it completes with no error message.
- [ ] Confirm the Start Menu entry is gone.
- [ ] **This is the important one - check it deliberately, don't skip it:** open File Explorer,
      go to `%APPDATA%`, and confirm the `dev.envryn.vault` folder (and your vault database
      inside it) **is still there** after uninstalling. This is expected, correct behavior - an
      uninstall should never silently destroy your secrets - see `INSTALLER_REVIEW.md` and the
      README's "Uninstalling and removing your data" section. If the folder is instead gone,
      that is a real, serious bug: report it, don't treat it as a pass.

## 9. Leftover files

Right after uninstalling (§8), still on the same VM:

- [ ] Confirm `C:\Program Files\Envryn` (MSI installs) or `%LOCALAPPDATA%\Envryn` (NSIS installs)
      no longer exists.
- [ ] Confirm `%APPDATA%\dev.envryn.vault` (your vault data) still exists - this is a repeat of
      §8's check; confirming it twice, once right after uninstall and once here, catches a
      cleanup step that runs with a delay.
- [ ] Optionally check `%LOCALAPPDATA%\dev.envryn.vault` too, if present - this holds the
      embedded browser engine's own cache files, not your secrets, but note whether it survived
      uninstall as well (it's expected to, same reasoning as the vault folder).

## 10. Unexpected network connections

- [ ] Stop the packet capture from §0:
      ```powershell
      pktmon stop
      pktmon etl2pcap "$env:USERPROFILE\Desktop\envryn-capture.etl" -o "$env:USERPROFILE\Desktop\envryn-capture.pcapng"
      ```
      Open the resulting `.pcapng` file - Wireshark is the easiest way to browse it if available;
      if not, `pktmon` also has a `format` command to dump it as text for a rougher read-through.
- [ ] Do **one full pass** of this checklist *without* ever turning on the local-AI feature in
      Settings, and confirm the capture shows **zero** network activity from Envryn the entire
      time (install, use, uninstall).
- [ ] Separately, do a pass where you *do* turn on and use the AI feature (which downloads a
      model on first use) and confirm the capture shows connections to `huggingface.co` and
      nowhere else during that download - no other host, no repeated/background connections after
      it completes.
- [ ] If you test device sync/pairing, confirm capture activity only appears while that flow is
      actively being used, to a local peer address on your own network, never anywhere else.
- [ ] Any other connection you see - anything not explained by one of the three cases above - is
      a real finding. Stop and report it; don't wave it off as noise.

---

## What this checklist cannot substitute for

One full pass, on one VM snapshot, is a single sample - not a guarantee across every Windows
10/11 build, locale, and hardware combination. Treat a clean pass as strong evidence the release
is ready, not absolute proof, the same way `SECURITY_REMEDIATION_REPORT.md` treats every other
check in this project.
