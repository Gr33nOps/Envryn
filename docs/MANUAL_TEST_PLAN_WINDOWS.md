# Envryn Windows Manual Test Plan

This plan covers the installed Envryn desktop application on Windows. It is designed for release testing and beta testing. Mobile testing is intentionally out of scope.

## How to use this plan

Record the result of each case in the `Result` column:

- `PASS`: Actual result matches the expected result.
- `FAIL`: Actual result differs from the expected result.
- `BLOCKED`: The test cannot run because a prerequisite is unavailable.
- `N/A`: The test does not apply to this build or environment.

For every failure, save the test ID, exact steps, expected result, actual result, screenshot or recording, app version, Windows version, and relevant log file.

## Build and tester record

| Field                          | Value |
| ------------------------------ | ----- |
| App version                    | 0.1.9 |
| Commit or release tag          | `af5f591` plus the documented working-tree fixes |
| Installer filename             | `Envryn_0.1.9_x64_en-US.msi` and `Envryn_0.1.9_x64-setup.exe` |
| Windows edition and build      | Windows 10 Enterprise 10.0.19041 Sandbox; Windows 10 Pro 10.0.19045 host |
| Windows account type           | Disposable Sandbox account for installer tests |
| Display resolution and scaling | 1366 by 768 plus effective 125, 150, and 200 percent browser layouts |
| WebView2 version               | Offline runtime embedded; exact runtime version not recorded |
| Test date                      | 2026-08-30 |
| Tester                         | Automated QA harness with native and Sandbox verification |

## Priorities

- `P0`: Data safety, vault access, privacy, backup, or basic app operation. Any failure blocks release.
- `P1`: Important daily workflow. Any unresolved failure normally blocks release.
- `P2`: Secondary workflow, polish, or uncommon error case. Review before release.

## Safety and prerequisites

- Use fake credentials only. Never paste a real production secret into a test vault.
- Use a disposable Windows account, VM, or test PC for restore, corruption, upgrade, and interrupted-write tests.
- Back up any existing Envryn data before starting destructive tests.
- Prepare two Windows PCs or VMs on the same private LAN for pairing and sync tests. No phone is required.
- Test with Local AI disabled first. Run the Local AI section again after the model is downloaded and running.
- Keep Windows Task Manager, Snipping Tool, Clipboard History, and File Explorer available.
- For network tests, know how to temporarily disable Wi-Fi or Ethernet and how to change the Windows Firewall rule for Envryn.

## Safe test data

Use clearly fake values such as these:

| Item             | Test value                                                           |
| ---------------- | -------------------------------------------------------------------- |
| Primary password | `Envryn QA Vault 2026!`                                              |
| Changed password | `Envryn QA Vault Changed 2026!`                                      |
| Weak password    | `test`                                                               |
| Projects         | `Website`, `Desktop Client`, `Empty Project`                         |
| Environments     | Development, Staging, Production, Unassigned                         |
| Variable names   | `IGDB_CLIENT_ID`, `TMDB_API_KEY`, `VERCEL_CLI_TOKEN`, `DATABASE_URL` |
| Secret values    | `FAKE_VALUE_FOR_MANUAL_TESTING_ONLY` plus a unique number            |
| Tags             | `qa`, `billing`, `release-candidate`                                 |

## Test environment matrix

At minimum, run the P0 and P1 tests in the first row. Run the remaining rows before a public release when the environment is available.

| Environment                                       | Scope                                  | Result |
| ------------------------------------------------- | -------------------------------------- | ------ |
| Windows 11, current supported build, 100% scaling | Full suite                             | BLOCKED |
| Windows 11, 125% or 150% scaling                  | Visual, window, modal, accessibility   | BLOCKED; equivalent layouts pass |
| Windows 11, 200% scaling                          | Visual, keyboard, modal, accessibility | BLOCKED; equivalent layout passes |
| Windows 10, latest supported build                | P0, P1, installer, Windows integration | PASS for clean Sandbox scope |
| Standard Windows user                             | Install, launch, vault, backup, sync   | BLOCKED |
| Offline PC                                        | Vault, search, Local AI, privacy       | PASS for installer, core, and Local AI scope |
| Two Windows PCs or VMs on one LAN                 | Pairing and sync                       | BLOCKED; protocol tests pass |

## Release smoke test

Run this short sequence on the final installer before the full suite.

| Result | ID      | Priority | Test                                                                             | Expected result                                                                                 |
| ------ | ------- | -------- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
|        | SMK-001 | P0       | Install and launch the release build.                                            | Installation completes and Envryn opens without an error or blank window.                       |
|        | SMK-002 | P0       | Create a vault with a strong password, close Envryn, reopen it, and unlock.      | The vault is created once, remains locked after restart, and accepts only the correct password. |
|        | SMK-003 | P0       | Create `Website`, open it, and add one API Key secret.                           | The project opens and the secret appears inside it.                                             |
|        | SMK-004 | P0       | Edit every visible field of that secret, save, close the panel, and reopen it.   | Every edited field persists exactly.                                                            |
|        | SMK-005 | P0       | Reveal and copy the value, then wait for the configured clipboard timeout.       | Reveal works, copy is accurate, and the clipboard is cleared at the expected time.              |
|        | SMK-006 | P1       | Search for the secret by name, provider, tag, project, and environment.          | The correct result appears quickly for every query.                                             |
|        | SMK-007 | P1       | Create a Database, SSH, Secure Note, Environment Variable, and Custom secret.    | Each type shows its correct fields, saves, and displays an appropriate icon.                    |
|        | SMK-008 | P0       | Create an encrypted backup, then restore it into a disposable test vault.        | Restore succeeds and all test data is readable with the new password.                           |
|        | SMK-009 | P0       | Press `Win+L`, sign back in, and return to Envryn.                               | The vault is locked.                                                                            |
|        | SMK-010 | P0       | Press `Ctrl+L`, then unlock again.                                               | The vault locks immediately and unlocks without data loss.                                      |
|        | SMK-011 | P1       | Pair two Windows test installations and run Sync All.                            | Both PCs become trusted and contain the same test data.                                         |
|        | SMK-012 | P0       | Disconnect the network, restart Envryn, unlock, view, edit, and create a secret. | Core vault operations work offline and no cloud sign-in is requested.                           |

## 1. Installation, launch, and removal

| Result | ID      | Priority | Test                                                                              | Expected result                                                                                    |
| ------ | ------- | -------- | --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
|        | INS-001 | P0       | Install on a PC where Envryn has never been installed.                            | Setup finishes without an unexplained warning and creates a working app entry.                     |
|        | INS-002 | P1       | Launch from the installer completion screen.                                      | One responsive Envryn window opens.                                                                |
|        | INS-003 | P1       | Launch from the Start menu.                                                       | Envryn opens with the correct name and icon.                                                       |
|        | INS-004 | P1       | Launch from a desktop shortcut if the installer offers one.                       | The shortcut opens the installed app.                                                              |
|        | INS-005 | P1       | Launch Envryn twice quickly.                                                      | The app does not corrupt data or leave unusable duplicate windows.                                 |
|        | INS-006 | P0       | Restart Windows and launch Envryn.                                                | The existing vault is detected and remains locked.                                                 |
|        | INS-007 | P1       | Install the same version over the existing version.                               | Installation completes and the vault remains intact.                                               |
|        | INS-008 | P0       | Upgrade from the previous public version.                                         | Settings, projects, secrets, device trust, and backups remain usable.                              |
|        | INS-009 | P2       | Cancel setup before installation finishes.                                        | Setup exits cleanly and does not leave a broken app entry.                                         |
|        | INS-010 | P1       | Uninstall on a disposable PC, then inspect installed-app entries and shortcuts.   | Program files and shortcuts are removed cleanly. User data behavior matches the uninstall message. |
|        | INS-011 | P0       | Reinstall after uninstall without manually deleting vault data.                   | Existing retained data is handled predictably and never silently overwritten.                      |
|        | INS-012 | P1       | Verify the executable, installer, Start menu entry, taskbar, and title bar icons. | All use the intended dark Envryn branding and remain sharp.                                        |

## 2. First run, vault creation, and unlock

| Result | ID      | Priority | Test                                                                              | Expected result                                                                        |
| ------ | ------- | -------- | --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
|        | VLT-001 | P0       | Start with no vault data.                                                         | The first-run screen offers vault creation and joining an existing vault.              |
|        | VLT-002 | P1       | Submit vault creation with empty password fields.                                 | Creation is blocked with clear validation.                                             |
|        | VLT-003 | P1       | Enter a weak password.                                                            | Strength feedback is understandable and unsafe submission is prevented where required. |
|        | VLT-004 | P1       | Enter different password and confirmation values.                                 | A mismatch is shown and no vault is created.                                           |
|        | VLT-005 | P1       | Toggle password visibility on both password fields.                               | Only the chosen field changes visibility and the value is not altered.                 |
|        | VLT-006 | P0       | Create a vault with the primary test password.                                    | Creation succeeds once and the unlocked vault opens.                                   |
|        | VLT-007 | P0       | Close and reopen the app after creation.                                          | The unlock screen appears instead of first-run setup.                                  |
|        | VLT-008 | P0       | Unlock with an incorrect password.                                                | Access is denied with a useful, non-sensitive error.                                   |
|        | VLT-009 | P0       | Unlock with the correct password.                                                 | The vault opens and all data is available.                                             |
|        | VLT-010 | P1       | Press Enter after typing the correct password.                                    | The form submits exactly once.                                                         |
|        | VLT-011 | P1       | Click Unlock repeatedly while unlock is in progress.                              | Only one operation runs and the UI remains stable.                                     |
|        | VLT-012 | P0       | Lock, then check that secret content is no longer visible behind the lock screen. | Names, values, notes, and prior search results are not exposed.                        |
|        | VLT-013 | P0       | Kill Envryn in Task Manager while unlocked, then reopen it.                       | The app starts locked and previously saved data is intact.                             |
|        | VLT-014 | P1       | Start Envryn without internet access.                                             | Vault creation and password unlock work without a network connection.                  |
|        | VLT-015 | P1       | Resize the first-run and unlock windows to minimum supported size.                | Controls remain visible, usable, and free of overlap.                                  |
|        | VLT-016 | P2       | Paste leading or trailing spaces into the password field.                         | Behavior is consistent and does not unexpectedly change a valid password.              |

## 3. Windows account unlock and password changes

Use a disposable Windows account for this section.

| Result | ID       | Priority | Test                                                                                    | Expected result                                                                        |
| ------ | -------- | -------- | --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
|        | AUTH-001 | P1       | In Settings, enable Windows account unlock and enter the wrong current master password. | Enabling fails and the option remains off.                                             |
|        | AUTH-002 | P0       | Enable Windows account unlock with the correct current master password.                 | Enabling succeeds and the saved setting is visible.                                    |
|        | AUTH-003 | P0       | Lock the vault and choose Windows account unlock.                                       | The vault unlocks only for the same Windows account.                                   |
|        | AUTH-004 | P0       | Restart Windows, launch Envryn, and use Windows account unlock.                         | Unlock works without exposing the master password.                                     |
|        | AUTH-005 | P1       | Disable Windows account unlock, lock, and inspect unlock choices.                       | Windows account unlock is no longer available or no longer succeeds.                   |
|        | AUTH-006 | P0       | Open Change vault password and enter the wrong current password.                        | The change is rejected and the old password still works.                               |
|        | AUTH-007 | P1       | Enter mismatched or weak new passwords.                                                 | The change is blocked with clear validation.                                           |
|        | AUTH-008 | P0       | Change to the changed test password, lock, and unlock.                                  | The new password works and the old password fails.                                     |
|        | AUTH-009 | P0       | After changing the password, reopen several secrets and create a backup.                | Existing data decrypts correctly and new encryption operations work.                   |
|        | AUTH-010 | P0       | If Windows account unlock was enabled before the password change, test it again.        | It behaves consistently or clearly requests re-enrollment. There is no silent lockout. |

## 4. Window controls, navigation, and shortcuts

| Result | ID      | Priority | Test                                                                                      | Expected result                                                               |
| ------ | ------- | -------- | ----------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
|        | WIN-001 | P1       | Drag the custom title bar across monitors.                                                | The window moves smoothly and does not start an unintended action.            |
|        | WIN-002 | P1       | Use minimize, maximize, restore, and close.                                               | Each control performs the standard Windows action once.                       |
|        | WIN-003 | P1       | Double-click the title bar.                                                               | Maximize and restore behave like a normal Windows window.                     |
|        | WIN-004 | P1       | Resize from every edge and corner.                                                        | The correct resize cursor appears and content remains usable.                 |
|        | WIN-005 | P2       | Snap the window left and right with Windows Snap.                                         | The app snaps and redraws correctly.                                          |
|        | WIN-006 | P1       | Navigate through All Secrets, categories, Projects, Devices, Sync, Backup, and Settings.  | The selected state, heading, content, and browser history remain correct.     |
|        | WIN-007 | P1       | Press `Ctrl+N` from several vault pages.                                                  | Add Secret opens once and returns to the prior page after cancel.             |
|        | WIN-008 | P1       | Press `Ctrl+K`, type a query, navigate with arrows, press Enter, then Escape.             | Search is keyboard-operable and opens the selected result.                    |
|        | WIN-009 | P0       | Press `Ctrl+L` from a secret panel and from a modal.                                      | The vault locks immediately and sensitive overlays disappear.                 |
|        | WIN-010 | P1       | Press Escape in search, Add Secret, Edit Secret, confirmation dialogs, and detail panels. | Only the topmost dismissible layer closes. No changes are saved accidentally. |
|        | WIN-011 | P2       | Navigate back and forward after opening projects, categories, and settings.               | Routes restore correctly without duplicated or stale overlays.                |
|        | WIN-012 | P1       | Leave the app open for 30 minutes while navigating periodically.                          | The app remains responsive and memory use does not grow continuously.         |

## 5. Secret creation and common fields

| Result | ID      | Priority | Test                                                                                     | Expected result                                                                         |
| ------ | ------- | -------- | ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
|        | SEC-001 | P0       | Open Add Secret from All Secrets.                                                        | The type selection or secret form opens correctly.                                      |
|        | SEC-002 | P1       | Try to save without the required name or required value.                                 | Save is blocked and the relevant field is identified.                                   |
|        | SEC-003 | P0       | Create a secret with name, type, project, environment, value, provider, notes, and tags. | It appears once with every field preserved.                                             |
|        | SEC-004 | P1       | Create a secret with a long name, long notes, and several tags.                          | Data saves without clipping, layout overlap, or silent truncation.                      |
|        | SEC-005 | P1       | Use punctuation, Unicode, spaces, and line breaks in supported fields.                   | Supported characters round-trip correctly.                                              |
|        | SEC-006 | P1       | Add comma-separated tags with extra spaces and mixed case.                               | Tags are parsed consistently and do not create blank entries.                           |
|        | SEC-007 | P1       | Select each environment: Development, Staging, Production, and Unassigned.               | The selected environment is saved and displayed correctly.                              |
|        | SEC-008 | P1       | Select each available project and also test no project where allowed.                    | Project membership matches the selection.                                               |
|        | SEC-009 | P0       | Reveal and hide a value several times.                                                   | The exact value appears only while revealed and never changes.                          |
|        | SEC-010 | P0       | Copy a value and paste it into a temporary plain-text field.                             | The copied text matches exactly. Remove the temporary text immediately.                 |
|        | SEC-011 | P1       | Open two different secrets in sequence.                                                  | The panel always shows the current secret and never leaks values from the previous one. |
|        | SEC-012 | P1       | Cancel a partially completed new secret.                                                 | No secret is created and entered data is discarded.                                     |
|        | SEC-013 | P1       | Double-click Save or press Enter repeatedly during save.                                 | Only one secret is created.                                                             |
|        | SEC-014 | P0       | Restart the app, unlock, and inspect all secrets created in this section.                | All saved values and metadata persist.                                                  |

## 6. Secret type coverage

For each row, create the secret, reveal or view all sensitive fields, edit every field, save, close the panel, reopen it, then restart Envryn and check it again.

| Result | ID       | Priority | Type                    | Fields and behavior to verify                                                                             |
| ------ | -------- | -------- | ----------------------- | --------------------------------------------------------------------------------------------------------- |
|        | TYPE-001 | P0       | API Key                 | Name, key value, provider, project, environment, notes, and tags persist.                                 |
|        | TYPE-002 | P0       | Token                   | Token value and all metadata persist.                                                                     |
|        | TYPE-003 | P0       | Environment Variable    | Variable name and value remain paired and searchable.                                                     |
|        | TYPE-004 | P0       | Database                | Host, port, database, username, password, and metadata persist independently.                             |
|        | TYPE-005 | P0       | SSH                     | Host, username, private key, passphrase, and metadata persist with line breaks intact.                    |
|        | TYPE-006 | P0       | OAuth                   | Client ID, client secret, and metadata persist independently.                                             |
|        | TYPE-007 | P1       | Webhook                 | Endpoint, signing secret, and metadata persist.                                                           |
|        | TYPE-008 | P0       | Secure Note             | Multi-line note body and metadata persist without converting the type.                                    |
|        | TYPE-009 | P0       | Custom                  | Multiple field names and values persist in the same order.                                                |
|        | TYPE-010 | P1       | Custom field controls   | Add, rename, reveal, hide, and remove custom fields. No other field changes unexpectedly.                 |
|        | TYPE-011 | P1       | Type change during edit | Change a test secret to another supported type. The UI explains or safely handles fields that do not map. |
|        | TYPE-012 | P1       | Empty optional fields   | Save each type with only required data. It remains valid and does not display fake metadata.              |

## 7. Secret editing and deletion

| Result | ID      | Priority | Test                                                                    | Expected result                                                      |
| ------ | ------- | -------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------- |
|        | EDT-001 | P0       | Edit only the environment and save.                                     | The new environment is shown immediately and after restart.          |
|        | EDT-002 | P0       | Edit only the project and save.                                         | The secret moves to the selected project and leaves the old project. |
|        | EDT-003 | P0       | Edit name, type-specific fields, provider, notes, and tags in one save. | Every changed field persists and unchanged fields remain unchanged.  |
|        | EDT-004 | P1       | Remove optional provider, notes, tags, and project values.              | Cleared values remain cleared after reopen and restart.              |
|        | EDT-005 | P1       | Cancel an edit after changing every field.                              | The original saved version remains unchanged.                        |
|        | EDT-006 | P1       | Close the edit modal with Escape and the close button.                  | Unsaved changes are not silently committed.                          |
|        | EDT-007 | P0       | Delete a secret, then cancel the confirmation.                          | The secret remains present and usable.                               |
|        | EDT-008 | P0       | Delete a secret and confirm.                                            | It disappears from lists, categories, projects, counts, and search.  |
|        | EDT-009 | P0       | Restart after deletion.                                                 | The deleted secret does not return.                                  |
|        | EDT-010 | P1       | Delete the currently open secret from its menu.                         | The panel closes cleanly and no stale detail remains.                |
|        | EDT-011 | P1       | Edit a secret while a filter is active so it no longer matches.         | Save succeeds and the list updates without showing stale data.       |
|        | EDT-012 | P1       | Edit a secret so that it moves into another category.                   | Category counts and lists update immediately.                        |

## 8. Secret list, categories, filters, sorting, and icons

| Result | ID      | Priority | Test                                                                                                       | Expected result                                                                   |
| ------ | ------- | -------- | ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
|        | LST-001 | P1       | Compare All Secrets count with the actual number of records.                                               | The count matches.                                                                |
|        | LST-002 | P1       | Open every category after creating one secret of each type.                                                | Every supported type is reachable through a relevant filter or category.          |
|        | LST-003 | P1       | Apply API and Tokens, Databases, SSH, Secure Notes, Environment Variables, and Custom Credentials filters. | Each filter includes only the intended records.                                   |
|        | LST-004 | P1       | Apply each environment filter.                                                                             | Only the selected environment is shown and Unassigned works correctly.            |
|        | LST-005 | P1       | Combine text, type, and environment filters.                                                               | Results satisfy all active filters.                                               |
|        | LST-006 | P1       | Clear filters one at a time and then clear all.                                                            | The list expands predictably and returns to the full set.                         |
|        | LST-007 | P1       | Sort newest first and oldest first.                                                                        | Order reverses correctly and remains stable for equal timestamps.                 |
|        | LST-008 | P1       | Create common provider examples such as IGDB, TMDB, Vercel CLI, PostgreSQL, GitHub, AWS, and Docker.       | Recognized services receive useful distinct icons where available.                |
|        | LST-009 | P2       | Create an unknown provider and type.                                                                       | A sensible generic fallback icon appears without a broken image.                  |
|        | LST-010 | P1       | Inspect imported and manually created secrets.                                                             | `IMPORTED` appears only when the record genuinely contains that user-visible tag. |
|        | LST-011 | P1       | Scroll a list with at least 100 secrets and open items near the bottom.                                    | Scrolling remains responsive and opens the correct record.                        |
|        | LST-012 | P2       | Use very long names, project names, and tags in the list.                                                  | Text truncates or wraps cleanly without covering actions.                         |
|        | LST-013 | P1       | Use a context menu on several secret types.                                                                | Actions target the selected record and dismiss normally.                          |
|        | LST-014 | P1       | Trigger an empty filter result.                                                                            | A clear empty state appears and offers a sensible recovery action.                |

## 9. Projects

| Result | ID      | Priority | Test                                                                 | Expected result                                                                       |
| ------ | ------- | -------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
|        | PRJ-001 | P0       | Click New Project, enter `Website`, and save.                        | A project is created and its project page opens. Add Secret does not open by mistake. |
|        | PRJ-002 | P1       | Try to create a blank or whitespace-only project.                    | Creation is blocked with clear validation.                                            |
|        | PRJ-003 | P1       | Try to create a project whose normalized name already exists.        | The app handles the duplicate predictably and does not create confusing duplicates.   |
|        | PRJ-004 | P0       | Create `Empty Project`, close and reopen Envryn.                     | The empty project still exists and opens with an empty state.                         |
|        | PRJ-005 | P0       | Add a secret from inside a project.                                  | The project is preselected and the new secret appears in that project.                |
|        | PRJ-006 | P1       | Add secrets in all four environments to one project.                 | Environment tabs and counts match the records.                                        |
|        | PRJ-007 | P1       | Search and sort within a project.                                    | Only that project's secrets are searched and sorted.                                  |
|        | PRJ-008 | P0       | Rename a project containing multiple secret types.                   | The project keeps its contents and every secret shows the new project name.           |
|        | PRJ-009 | P1       | Cancel a project rename.                                             | The original name and membership remain unchanged.                                    |
|        | PRJ-010 | P0       | Move a secret from one project to another through Edit Secret.       | Both project lists and counts update correctly.                                       |
|        | PRJ-011 | P1       | Remove a secret from a project if unassigned projects are supported. | The secret remains in All Secrets but leaves the project.                             |
|        | PRJ-012 | P1       | Open a legacy project inferred from existing secret metadata.        | It appears once and contains the expected records.                                    |
|        | PRJ-013 | P1       | Use long, Unicode, and punctuation-heavy project names.              | Supported names persist and routes still open correctly.                              |
|        | PRJ-014 | P1       | Lock the vault while a project is open, then unlock.                 | The app returns safely with correct project data or to a predictable default page.    |

## 10. Search and command palette

| Result | ID       | Priority | Test                                                                                  | Expected result                                                                                                |
| ------ | -------- | -------- | ------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
|        | SRCH-001 | P0       | Search an exact secret name.                                                          | The correct record is the top result without invoking AI.                                                      |
|        | SRCH-002 | P1       | Search a partial name and a minor typo.                                               | Useful fuzzy matches appear quickly.                                                                           |
|        | SRCH-003 | P1       | Search by provider, project, environment, type, tag, and non-sensitive notes text.    | Relevant records appear for every supported metadata field.                                                    |
|        | SRCH-004 | P0       | Search for the actual secret value.                                                   | Sensitive values are not exposed in result previews. Search behavior follows the documented privacy model.     |
|        | SRCH-005 | P1       | Enter a query with different capitalization and spacing.                              | Matching is consistent and not needlessly case-sensitive.                                                      |
|        | SRCH-006 | P1       | Navigate results using arrow keys, Enter, mouse, and Escape.                          | Selection and dismissal are correct.                                                                           |
|        | SRCH-007 | P1       | Search with Local AI disabled.                                                        | Normal search remains fast and fully usable.                                                                   |
|        | SRCH-008 | P1       | Search with Local AI enabled but the model stopped or unavailable.                    | Normal search still works and assisted errors do not break the palette.                                        |
|        | SRCH-009 | P1       | Use an intent query such as `production database for Website`, then choose Interpret. | Assisted search returns relevant constrained results or an honest no-match response.                           |
|        | SRCH-010 | P1       | Search for uncommon services: IGDB, TMDB, and Vercel CLI.                             | Results use available metadata and do not substitute an unrelated popular provider.                            |
|        | SRCH-011 | P1       | Enter a nonsense or ambiguous query.                                                  | The app avoids a confident incorrect answer and allows normal search recovery.                                 |
|        | SRCH-012 | P1       | Measure exact-name search with 1,000 test records.                                    | Visible results begin within 300 ms on the reference PC. Record actual time.                                   |
|        | SRCH-013 | P2       | Measure assisted search after model warm-up.                                          | The UI remains responsive, shows progress, and completes within the agreed release budget. Record actual time. |
|        | SRCH-014 | P1       | Delete or edit a search result, close search, then search again.                      | The index reflects the latest saved state with no stale result.                                                |

## 11. `.env` import

| Result | ID      | Priority | Test                                                                                                         | Expected result                                                                                           |
| ------ | ------- | -------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------- |
|        | IMP-001 | P1       | Open Import `.env`, then cancel without entering data.                                                       | No record is created.                                                                                     |
|        | IMP-002 | P1       | Submit an empty input or comments-only input.                                                                | Import is blocked with a useful message.                                                                  |
|        | IMP-003 | P0       | Paste valid lines containing IGDB, TMDB, Vercel CLI, database, and generic variables.                        | All valid variables appear in review with correct names and intact values.                                |
|        | IMP-004 | P1       | Include blank lines, comments, `export`, quoted values, spaces around `=`, embedded `=`, and an empty value. | Supported syntax parses correctly and malformed lines are explained or skipped safely.                    |
|        | IMP-005 | P1       | In review, reveal and hide values.                                                                           | Visibility changes only for the intended row.                                                             |
|        | IMP-006 | P1       | Exclude selected rows before import.                                                                         | Only checked rows are created.                                                                            |
|        | IMP-007 | P1       | Change detected secret types before import.                                                                  | Manual selections are preserved in created records.                                                       |
|        | IMP-008 | P1       | Choose project and environment, then import.                                                                 | Every created record has the selected project and environment.                                            |
|        | IMP-009 | P0       | Inspect the tags of imported records.                                                                        | The app does not add an `IMPORTED` tag unless the user explicitly chose it.                               |
|        | IMP-010 | P1       | Import with Local AI disabled.                                                                               | Deterministic classification still works and import completes offline.                                    |
|        | IMP-011 | P1       | Import with Local AI enabled and running.                                                                    | Uncommon names improve where the model is confident, while values remain local.                           |
|        | IMP-012 | P1       | Attempt import with no rows selected.                                                                        | No records are created and the UI asks for at least one selection.                                        |
|        | IMP-013 | P1       | Import 100 fake variables.                                                                                   | The review list remains usable, each selected item is imported once, and success count is accurate.       |
|        | IMP-014 | P2       | Force one record to fail during a disposable import if a safe test hook exists.                              | The app reports partial success and identifies failed variable names without duplicating successful rows. |

## 12. Local AI, suggestions, and structured extraction

| Result | ID     | Priority | Test                                                                                   | Expected result                                                                                |
| ------ | ------ | -------- | -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
|        | AI-001 | P1       | Open Settings on a fresh installation.                                                 | Local AI is opt-in and its state is explained clearly.                                         |
|        | AI-002 | P1       | Enable Local AI without a model present.                                               | The app offers or begins the model setup flow with clear size and status information.          |
|        | AI-003 | P1       | Interrupt model download by disconnecting the network.                                 | The UI reports failure safely and can retry without corrupting the app.                        |
|        | AI-004 | P1       | Complete the model download and start Local AI.                                        | Status reaches a ready or running state without requiring a cloud account.                     |
|        | AI-005 | P1       | Restart Envryn after Local AI setup.                                                   | The enabled state and model availability are handled predictably.                              |
|        | AI-006 | P1       | Disable Local AI.                                                                      | AI actions stop using the model while normal vault workflows keep working.                     |
|        | AI-007 | P1       | Use Suggest Name and Suggest Type with `IGDB_CLIENT_ID`.                               | Suggestions reflect IGDB and Client ID or present a cautious fallback.                         |
|        | AI-008 | P1       | Repeat with `TMDB_API_KEY` and `VERCEL_CLI_TOKEN`.                                     | Suggestions recognize the terminology and avoid unrelated providers.                           |
|        | AI-009 | P1       | Use a completely unknown variable name and fake value.                                 | The app uses a generic, editable suggestion instead of false certainty.                        |
|        | AI-010 | P1       | Provide conflicting name, provider, and metadata clues.                                | The suggestion is cautious and does not overwrite entered fields automatically.                |
|        | AI-011 | P1       | Confirm that accepting a suggestion changes only the intended field.                   | Other form values remain untouched and final Save is still required.                           |
|        | AI-012 | P1       | Open Extract fields with Local AI disabled.                                            | The app explains that Local AI must be enabled and does not lose the pasted text unexpectedly. |
|        | AI-013 | P1       | Extract a fake connection string or labeled configuration block.                       | Useful labeled fields appear in review and can be edited before saving.                        |
|        | AI-014 | P1       | Extract empty, unlabeled, and nonsensical text.                                        | Clear validation or no-fields feedback appears without inventing sensitive data.               |
|        | AI-015 | P1       | Add, rename, and remove extracted fields, select a project and environment, then save. | A Custom secret is created exactly as reviewed.                                                |
|        | AI-016 | P0       | Run Local AI actions while the internet is disconnected.                               | Once the model is installed, inference works locally and no sign-in is requested.              |
|        | AI-017 | P1       | Monitor Task Manager during idle, first inference, repeated inference, and disable.    | CPU and memory use are explainable, the UI stays responsive, and resources settle after work.  |
|        | AI-018 | P2       | Repeat the same suggestion five times.                                                 | Results are reasonably stable and never damage manually entered data.                          |

## 13. Backup and restore

Run this section only with a disposable test vault or after making a separate verified backup.

| Result | ID      | Priority | Test                                                                                                 | Expected result                                                                                                    |
| ------ | ------- | -------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
|        | BAK-001 | P1       | Open Backup from the sidebar and from Settings.                                                      | Both routes open the same working backup page.                                                                     |
|        | BAK-002 | P1       | Start backup and cancel the file chooser.                                                            | No backup is created and the vault remains unchanged.                                                              |
|        | BAK-003 | P1       | Try an empty, weak, or mismatched backup password.                                                   | Backup is blocked with clear validation.                                                                           |
|        | BAK-004 | P0       | Create a backup with a valid path and password.                                                      | A non-empty backup file is created and success is reported.                                                        |
|        | BAK-005 | P0       | Search the backup file with a text or hex viewer for fake secret names and values.                   | Plaintext names, values, and notes are not readable.                                                               |
|        | BAK-006 | P1       | Create backups to a path containing spaces and Unicode characters.                                   | Backup succeeds or gives a clear supported-path error.                                                             |
|        | BAK-007 | P1       | Attempt to overwrite an existing backup file.                                                        | The app confirms or handles overwrite predictably and never silently damages the only backup.                      |
|        | BAK-008 | P1       | Start restore and cancel the file chooser or restore dialog.                                         | The current vault remains unchanged.                                                                               |
|        | BAK-009 | P0       | Restore a valid backup with the wrong backup password.                                               | Restore fails without changing the current vault.                                                                  |
|        | BAK-010 | P0       | Restore a random, truncated, or modified file.                                                       | Restore fails safely and the current vault remains unlockable.                                                     |
|        | BAK-011 | P1       | Use a weak or mismatched new vault password during restore.                                          | Restore is blocked before replacing data.                                                                          |
|        | BAK-012 | P0       | Restore a valid backup with correct credentials and a new vault password.                            | Success count is accurate and the restored vault opens with the new password.                                      |
|        | BAK-013 | P0       | Verify every project, environment, secret type, custom field, tag, note, and provider after restore. | Restored content matches the source backup exactly.                                                                |
|        | BAK-014 | P0       | Restart Envryn and unlock with the restore password, then try the pre-restore password.              | The new password works, the old one fails, and restored data persists.                                             |
|        | BAK-015 | P1       | Inspect the app data directory after restore on the disposable PC.                                   | The old vault is preserved or replaced according to the documented recovery design, with no ambiguous active copy. |
|        | BAK-016 | P1       | Attempt backup to an unwritable or full destination on a disposable environment.                     | The app reports the write failure and leaves existing files and vault data intact.                                 |

## 14. Trusted devices and PC-to-PC pairing

Use PC A with an existing test vault and a fresh installation on PC B. Keep both on the same private LAN.

| Result | ID      | Priority | Test                                                                                        | Expected result                                                             |
| ------ | ------- | -------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
|        | DEV-001 | P1       | Open Trusted Devices with no paired devices.                                                | A clear empty state and Pair a device action appear.                        |
|        | DEV-002 | P0       | Start Pair a device on PC A.                                                                | A pairing code plus local address and port appear.                          |
|        | DEV-003 | P0       | On fresh PC B, choose Join existing vault and enter PC A's address, port, and pairing code. | Both PCs advance to a matching verification code stage.                     |
|        | DEV-004 | P0       | Compare the short authentication string on both PCs.                                        | The displayed codes match exactly before trust is confirmed.                |
|        | DEV-005 | P0       | Enter a wrong current master password on PC A when confirming.                              | Pairing fails and PC B is not trusted.                                      |
|        | DEV-006 | P0       | Repeat with the correct password and matching code.                                         | Pairing succeeds and PC B receives access to the test vault.                |
|        | DEV-007 | P0       | Lock and unlock PC B with its chosen password.                                              | PC B opens the paired vault and displays the expected data.                 |
|        | DEV-008 | P1       | Cancel pairing from PC A while waiting.                                                     | The listener stops and the shown code can no longer be used.                |
|        | DEV-009 | P1       | Enter an incorrect address, port, or pairing code on PC B.                                  | Pairing fails with a clear retry path and creates no trusted entry.         |
|        | DEV-010 | P1       | Let a pairing session sit until timeout, if a timeout exists.                               | Expired details cannot establish trust and retry generates a fresh session. |
|        | DEV-011 | P1       | Block Envryn in Windows Firewall during pairing.                                            | The app reports a reachable network error and remains recoverable.          |
|        | DEV-012 | P1       | Open a trusted device's details.                                                            | Name, status, last activity, and fingerprint are shown consistently.        |
|        | DEV-013 | P1       | Rename PC B from PC A.                                                                      | The new name persists after page refresh and app restart.                   |
|        | DEV-014 | P1       | Copy the device fingerprint.                                                                | The exact fingerprint is copied and a confirmation appears.                 |
|        | DEV-015 | P0       | Start revoke, then cancel.                                                                  | Trust remains unchanged.                                                    |
|        | DEV-016 | P0       | Revoke PC B and confirm.                                                                    | PC B is removed from Trusted Devices and can no longer sync.                |
|        | DEV-017 | P0       | Attempt to sync from revoked PC B.                                                          | Sync is rejected and the app does not silently recreate trust.              |
|        | DEV-018 | P1       | Pair PC B again after revocation.                                                           | A complete fresh verification is required and succeeds only after approval. |

## 15. PC-to-PC synchronization

Complete the pairing section first. Label unique test records with `PC-A` and `PC-B` so their origin is obvious.

| Result | ID      | Priority | Test                                                                                  | Expected result                                                                                        |
| ------ | ------- | -------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
|        | SYN-001 | P0       | Open Sync on both PCs while both are online.                                          | Each trusted peer is discovered and shown with a meaningful state.                                     |
|        | SYN-002 | P0       | Create a secret on PC A, run Sync All, and inspect PC B.                              | The secret arrives once with all fields intact.                                                        |
|        | SYN-003 | P0       | Create a secret on PC B, sync, and inspect PC A.                                      | Bidirectional creation works.                                                                          |
|        | SYN-004 | P0       | Edit name, environment, project, type fields, notes, and tags on PC A, then sync.     | Every edit reaches PC B and survives restart.                                                          |
|        | SYN-005 | P0       | Delete a disposable secret on PC B, then sync.                                        | The deletion propagates according to the product's deletion policy and does not reappear unexpectedly. |
|        | SYN-006 | P0       | Create a new empty project on PC A and sync.                                          | The project appears on PC B even before it contains a secret.                                          |
|        | SYN-007 | P0       | Rename a populated project and sync.                                                  | The new name and all memberships match on both PCs.                                                    |
|        | SYN-008 | P0       | Sync all nine secret types, including multi-line SSH keys and multiple Custom fields. | Payloads and metadata match exactly on both PCs.                                                       |
|        | SYN-009 | P1       | Copy a synced secret value on PC B.                                                   | The decrypted value is exact and clipboard protection still applies locally.                           |
|        | SYN-010 | P1       | Take PC B offline, create records on both PCs, reconnect, and run Sync All.           | Offline changes merge without losing unrelated records.                                                |
|        | SYN-011 | P0       | Edit the same secret differently on both PCs while offline, reconnect, and sync.      | A deterministic conflict outcome or visible conflict record appears. No change disappears silently.    |
|        | SYN-012 | P0       | Resolve or discard the conflict using each available action.                          | The chosen result is consistent on both PCs after another sync.                                        |
|        | SYN-013 | P1       | Start sync when the trusted peer is powered off.                                      | The app reports offline or unavailable, stays responsive, and offers retry.                            |
|        | SYN-014 | P1       | Start sync with no trusted devices.                                                   | A clear `no trusted device` state links or directs the user to pairing.                                |
|        | SYN-015 | P0       | Pair a device, return directly to Sync, and run sync without restarting either app.   | The new trusted device is recognized immediately.                                                      |
|        | SYN-016 | P1       | Disconnect the network during an active large sync.                                   | Sync fails safely, reports status, and can resume or retry without duplicates.                         |
|        | SYN-017 | P1       | Run Sync All repeatedly when both vaults already match.                               | Each run is idempotent and does not duplicate or alter records.                                        |
|        | SYN-018 | P1       | Close and reopen both apps after a completed sync.                                    | Synced data and trusted-device state persist.                                                          |
|        | SYN-019 | P1       | Change PC A's IP address on the LAN, then reopen Sync.                                | Discovery finds the trusted peer again or provides a clear recovery path.                              |
|        | SYN-020 | P1       | Test when Windows marks the LAN Public, then Private.                                 | Firewall-related failure is explained and normal operation resumes after allowed access.               |
|        | SYN-021 | P1       | Sync 1,000 fake records and record duration, CPU, and memory.                         | Progress remains visible, the app stays responsive, and final counts match.                            |
|        | SYN-022 | P0       | Compare a sample from every secret type field by field after the large sync.          | There is no truncation, encoding change, plaintext exposure, or metadata loss.                         |
|        | SYN-023 | P0       | Revoke PC B during or immediately before a sync attempt.                              | The revoked device cannot complete a new authenticated sync.                                           |
|        | SYN-024 | P1       | Change the vault password on one device and follow the supported sync flow.           | The app clearly handles re-authentication or key update without corrupting either vault.               |

## 16. Settings and persistence

| Result | ID      | Priority | Test                                                                            | Expected result                                                                             |
| ------ | ------- | -------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
|        | SET-001 | P1       | Set auto-lock to 1, 5, 15, 30, and 60 minutes in turn.                          | Each choice saves and remains selected after restart.                                       |
|        | SET-002 | P0       | With 1 minute selected, avoid keyboard and mouse input system-wide.             | The vault locks after about one minute of real Windows idle time.                           |
|        | SET-003 | P1       | Keep using another Windows application while Envryn is in the background.       | System activity prevents idle locking until the configured idle period is actually reached. |
|        | SET-004 | P1       | Set clipboard clearing to 10, 30, 60, and 120 seconds in turn.                  | Each choice persists after restart.                                                         |
|        | SET-005 | P0       | For each clipboard timeout, copy a fake secret and measure clearing.            | Clipboard clearing occurs close to the selected value.                                      |
|        | SET-006 | P0       | Copy a fake secret, then replace the clipboard with normal text before timeout. | Envryn does not erase the user's newer clipboard content.                                   |
|        | SET-007 | P1       | Follow Trusted Devices, Sync Details, Backup, and Export Vault links.           | Each opens the intended page.                                                               |
|        | SET-008 | P2       | Click View Security Documentation.                                              | The current build shows the intended `coming soon` notice and remains stable.               |
|        | SET-009 | P2       | Inspect Reset Vault and Delete Vault.                                           | Currently unavailable actions are visibly disabled and cannot be triggered accidentally.    |
|        | SET-010 | P1       | Change settings, lock, unlock, and restart.                                     | Settings persist and take effect after every transition.                                    |
|        | SET-011 | P1       | Verify the About version against the installer and GitHub release version.      | Version information matches exactly.                                                        |
|        | SET-012 | P1       | Verify displayed shortcut labels and execute each shortcut.                     | Labels match actual behavior.                                                               |

## 17. Windows privacy and security behavior

These checks validate visible behavior. They do not replace code review, automated security testing, or an independent security assessment.

| Result | ID      | Priority | Test                                                                                                                                  | Expected result                                                                                                                     |
| ------ | ------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
|        | PRV-001 | P0       | Open a revealed secret and try to capture the Envryn window with Snipping Tool, Print Screen, and a common meeting screen-share tool. | Protected content is blocked or hidden according to the app's documented capture-protection behavior. Record tool-specific results. |
|        | PRV-002 | P0       | Copy a fake secret, then open Clipboard History with `Win+V`.                                                                         | The secret is not added to Windows clipboard history where the OS supports exclusion.                                               |
|        | PRV-003 | P0       | Copy a fake secret on a PC with Windows cloud clipboard enabled.                                                                      | Envryn requests exclusion from roaming or history and the value is not shown on another Windows device.                             |
|        | PRV-004 | P0       | Lock the vault while a value is revealed.                                                                                             | Sensitive content disappears immediately.                                                                                           |
|        | PRV-005 | P0       | Press `Win+L`, sign back in, and return to Envryn.                                                                                    | The vault is locked even if the configured idle timeout has not elapsed.                                                            |
|        | PRV-006 | P0       | Put Windows to sleep or hibernate while unlocked, then resume.                                                                        | The vault returns locked.                                                                                                           |
|        | PRV-007 | P0       | Switch Windows users and return.                                                                                                      | The vault is locked and no secret is visible in previews.                                                                           |
|        | PRV-008 | P0       | Inspect recent files, Windows Search, notification text, and taskbar hover previews after normal use.                                 | Secret names and values are not unnecessarily exposed.                                                                              |
|        | PRV-009 | P0       | Use Envryn fully offline while monitoring active network connections with Windows Resource Monitor.                                   | Core operations do not require remote services. Local pairing traffic stays on the local network.                                   |
|        | PRV-010 | P0       | Search the app data directory for the exact fake secret value while the vault is locked.                                              | The value is not stored as readable plaintext.                                                                                      |
|        | PRV-011 | P0       | Trigger wrong-password, corrupt-backup, pairing, AI, and sync errors.                                                                 | Errors do not print secret values, master passwords, encryption keys, or full sensitive payloads.                                   |
|        | PRV-012 | P1       | Copy a device fingerprint and wait past the secret clipboard timeout.                                                                 | Non-secret copy behavior is predictable and does not weaken secret clipboard handling.                                              |

## 18. Accessibility, display, and usability

| Result | ID       | Priority | Test                                                                                                     | Expected result                                                                                                                               |
| ------ | -------- | -------- | -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
|        | A11Y-001 | P1       | Complete vault creation, unlock, create, edit, delete, search, backup, and settings using keyboard only. | Every interactive control is reachable, visible on focus, and usable in a logical order.                                                      |
|        | A11Y-002 | P1       | Use Tab and Shift+Tab inside every modal and confirmation dialog.                                        | Focus stays inside the open dialog and returns to the triggering control on close.                                                            |
|        | A11Y-003 | P1       | Test buttons and menu items with Enter and Space where appropriate.                                      | Keyboard activation matches standard Windows behavior.                                                                                        |
|        | A11Y-004 | P1       | Run Windows Narrator through first run, unlock, secret form, list, and settings.                         | Controls have useful names, roles, states, and validation announcements. Secret values are not read unless intentionally revealed or focused. |
|        | A11Y-005 | P1       | Zoom or scale Windows to 125%, 150%, and 200%.                                                           | Text, forms, dialogs, menus, and tables remain readable without horizontal loss of required actions.                                          |
|        | A11Y-006 | P1       | Test 1366x768 and a high-resolution monitor.                                                             | Primary actions remain visible and layouts do not overlap.                                                                                    |
|        | A11Y-007 | P2       | Test Windows High Contrast themes.                                                                       | Essential text, focus, borders, selection, errors, and icons remain distinguishable.                                                          |
|        | A11Y-008 | P2       | Enable Reduce motion in Windows.                                                                         | The app remains comfortable and does not depend on animation to communicate state.                                                            |
|        | A11Y-009 | P1       | Check text and control contrast in normal, hover, selected, disabled, error, and focus states.           | Important text and controls meet WCAG 2.1 AA contrast expectations.                                                                           |
|        | A11Y-010 | P1       | Use long names and Windows text scaling.                                                                 | Text does not cover reveal, copy, menu, Save, or Cancel controls.                                                                             |
|        | A11Y-011 | P1       | Trigger every validation and error state.                                                                | Errors are placed near the cause, readable, and not communicated by color alone.                                                              |
|        | A11Y-012 | P2       | Test with left-handed mouse settings and touchpad navigation.                                            | Core desktop actions remain practical and do not require precision clicks.                                                                    |

## 19. Reliability, recovery, and limits

Use disposable data for tests that interrupt the app or alter files.

| Result | ID      | Priority | Test                                                                              | Expected result                                                                                     |
| ------ | ------- | -------- | --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
|        | REL-001 | P0       | End the process immediately after a successful secret save, then reopen.          | The saved record is complete and the vault opens normally.                                          |
|        | REL-002 | P1       | End the process while an unsaved form is open.                                    | Previously saved data remains intact and no partial record appears.                                 |
|        | REL-003 | P0       | End the process during backup on a disposable destination.                        | The vault remains intact and an incomplete backup is not presented as valid.                        |
|        | REL-004 | P0       | End the process during restore on a disposable installation.                      | On restart, the app recovers to a valid old or restored vault and never an ambiguous corrupt state. |
|        | REL-005 | P1       | End the process during sync, then restart both PCs and sync again.                | Retry converges without duplicates or data loss.                                                    |
|        | REL-006 | P1       | Fill the destination disk or use a read-only folder during backup.                | The error is clear and no valid existing backup is damaged.                                         |
|        | REL-007 | P1       | Use 1,000 secrets across 50 projects.                                             | Unlock, list, filter, project open, and normal search remain usable. Record timings.                |
|        | REL-008 | P1       | Create very large notes, SSH keys, and custom field sets within expected limits.  | Save and reopen are correct, or a clear size limit is enforced.                                     |
|        | REL-009 | P1       | Rapidly open and close search and modals 50 times.                                | No frozen overlay, lost focus, duplicate action, or sustained memory growth appears.                |
|        | REL-010 | P1       | Change Windows time zone and clock, then create and edit records.                 | Sorting and displayed times remain understandable and records remain valid.                         |
|        | REL-011 | P1       | Disconnect and reconnect monitors while Envryn is open.                           | The window remains reachable and content redraws correctly.                                         |
|        | REL-012 | P1       | Remove or rename a backup file after choosing it but before restore confirmation. | Restore fails safely with no change to the current vault.                                           |
|        | REL-013 | P1       | Leave the vault locked overnight, then unlock.                                    | Unlock works and there is no stale loading or expired UI state.                                     |
|        | REL-014 | P2       | Run Envryn under a standard user with a non-ASCII Windows username.               | App data, backup, Local AI, and launch paths work or fail with a clear message.                     |

## 20. Release package and documentation acceptance

| Result | ID      | Priority | Test                                                                                                 | Expected result                                                                |
| ------ | ------- | -------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
|        | RLS-001 | P0       | Download the installer from the public GitHub release, not from a local build folder.                | The asset downloads completely and matches the published filename and version. |
|        | RLS-002 | P0       | Verify the published checksum using Windows PowerShell.                                              | The local installer checksum matches the release checksum exactly.             |
|        | RLS-003 | P1       | Compare release notes with the actual build.                                                         | Listed fixes and features are present and unsupported claims are absent.       |
|        | RLS-004 | P1       | Follow README installation and first-run instructions as a new user.                                 | Every instruction is accurate, ordered, and sufficient.                        |
|        | RLS-005 | P1       | Open every README and release link.                                                                  | Links resolve to the intended current pages or assets.                         |
|        | RLS-006 | P1       | Compare README screenshots with the installed release.                                               | Branding and major UI shown are current and not misleading.                    |
|        | RLS-007 | P1       | Verify LICENSE, SECURITY, privacy, support, contributing, and code-of-conduct documents are visible. | Documents are readable, current, and use working contact paths.                |
|        | RLS-008 | P1       | Check GitHub release tag, package version, app About version, and installer version.                 | All version identifiers agree.                                                 |
|        | RLS-009 | P0       | Confirm required GitHub checks for the release commit.                                               | All required CI, test, quality, and packaging checks pass.                     |
|        | RLS-010 | P1       | Install the final public asset on a second clean Windows environment.                                | The public artifact behaves the same as the approved candidate.                |

## Performance measurements

Record the reference PC specifications and use the same vault for comparisons between releases.

| Measurement                               |                                 Target | Actual | Result |
| ----------------------------------------- | -------------------------------------: | -----: | ------ |
| Cold launch to unlock screen              |      3 seconds or less on reference PC |        |        |
| Unlock with 1,000 records                 |      3 seconds or less on reference PC | 1,463.7 ms | PASS |
| Exact or fuzzy local search first results |                         300 ms or less | 169.9 ms | PASS |
| Open a normal secret panel                |                         200 ms or less |        |        |
| Save a normal secret                      |                       1 second or less |        |        |
| Apply list filter or sort                 |                         300 ms or less |        |        |
| Open a 1,000-record project               |                       1 second or less | 103 ms | PASS |
| Idle memory after 10 minutes              |                   No continuous growth |        |        |
| Local AI repeated query                   |                   Record after warm-up |        |        |
| LAN sync of 1,000 records                 | Record and compare to previous release | 13.37 seconds over real mutual TLS in-process | PASS for protocol scope |

These are release targets for the reference environment, not universal hardware guarantees. A consistent regression of 20 percent or more should be investigated.

## Exit criteria

A Windows release candidate is ready only when:

- Every applicable P0 test passes.
- Every applicable P1 test passes, or a documented product decision accepts the remaining issue.
- There is no known data loss, vault corruption, plaintext-at-rest, authentication bypass, or unauthorized sync issue.
- Backup and restore pass using the final installed build.
- PC-to-PC pairing, bidirectional sync, offline merge, conflict handling, restart persistence, and revocation pass.
- Create Project and edit all secret fields pass in the final build.
- Windows lock, sleep or resume, clipboard timeout, clipboard replacement protection, and capture protection are verified.
- The full secret type matrix passes after restart.
- Normal search remains useful with Local AI disabled.
- The final GitHub release asset passes the smoke test on a clean PC.
- All failures have an owner, severity, reproduction steps, and release decision.

## Defect report template

```text
Title:
Test ID:
Severity: Blocker / Critical / Major / Minor
App version and release tag:
Windows edition and build:
Display scaling:
Fresh install or upgraded install:
Vault state: New / Existing / Paired / Restored
Local AI state: Disabled / Downloading / Running / Error
Network state: Online / Offline / Same-LAN peer

Preconditions:
1.

Steps to reproduce:
1.
2.
3.

Expected result:

Actual result:

Frequency: Always / Often / Once
Regression from version:
Screenshot or recording:
Log location and timestamp:
Additional notes:
```

## Manual coverage gaps that automation cannot fully replace

Keep automated unit, integration, and end-to-end tests running, but do not rely on them alone for these areas:

- The real Windows installer, upgrade, uninstall, Start menu, icon, and WebView2 environment.
- DPAPI-backed Windows account unlock under real Windows accounts.
- Windows session lock, sleep, hibernate, user switching, and system-wide idle detection.
- Clipboard History, cloud clipboard exclusion, timing, and preservation of newer clipboard content.
- Screen-capture behavior across Windows capture and conferencing tools.
- Real LAN discovery, firewall profiles, IP changes, pairing verification, revocation, and two-PC convergence.
- Real Local AI model download, startup, resource use, latency, offline inference, and suggestion quality.
- Actual file chooser, path permissions, disk-full behavior, encrypted backup inspection, and restore recovery.
- Keyboard, Narrator, high contrast, DPI scaling, multi-monitor behavior, and visual quality.
