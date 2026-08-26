// Live adversarial/runtime testing against the real compiled Tauri window,
// via the same W3C WebDriver approach webdriver-smoke.mjs already proved
// out. This script exists to answer a different question than that one:
// not "does the happy path render," but "does the app actually behave
// safely when a hostile or careless user does something wrong."
//
// Run manually (same prerequisites as webdriver-smoke.mjs):
//   node .dev-tools/adversarial-smoke.mjs
//
// This creates and destroys real throwaway vaults in this machine's actual
// app-data directory. It deletes any existing dev vault at start (loudly,
// see below) and leaves the directory in a clean state at the end.
//
// Findings are printed as PASS/FAIL lines and collected into a summary at
// the end; screenshots go to .dev-tools/adversarial-*.png as evidence.

import { spawn, execFileSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import path from "node:path";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const appExe = path.join(repoRoot, "target", "release", "envryn.exe");
const edgeDriver = path.join(__dirname, "edgedriver", "msedgedriver.exe");
const driverPort = 4444;
const driverUrl = `http://127.0.0.1:${driverPort}`;
const vaultDbPath = path.join(process.env.APPDATA, "dev.envryn.vault", "envryn.db");
const PASSWORD = "Adversarial-Test-Passw0rd!42";

// WebDriver W3C key codes (Unicode Private Use Area), per the spec's
// "Normalized Key Value" table -- these are the actual values the protocol
// requires, not arbitrary placeholders.
const CTRL_KEY = String.fromCodePoint(0xe009);
const ESCAPE_KEY = String.fromCodePoint(0xe00c);

const results = [];
function record(name, pass, detail) {
  results.push({ name, pass, detail });
  console.log(`${pass ? "PASS" : "FAIL"}: ${name}${detail ? " -- " + detail : ""}`);
}

function startTauriDriver() {
  const proc = spawn(
    "tauri-driver",
    ["--port", String(driverPort), "--native-driver", edgeDriver],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  proc.stdout.on("data", (d) => process.stdout.write(`[tauri-driver] ${d}`));
  proc.stderr.on("data", (d) => process.stderr.write(`[tauri-driver] ${d}`));
  return proc;
}

async function waitForDriverReady() {
  for (let i = 0; i < 30; i++) {
    try {
      const res = await fetch(`${driverUrl}/status`);
      if (res.ok || res.status === 404) return;
    } catch {}
    await sleep(500);
  }
  throw new Error("tauri-driver did not become ready in time");
}

async function createSession() {
  const res = await fetch(`${driverUrl}/session`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      capabilities: { alwaysMatch: { browserName: "wry", "tauri:options": { application: appExe } } },
    }),
  });
  const body = await res.json();
  if (!res.ok || !body.value?.sessionId) throw new Error(`session creation failed: ${JSON.stringify(body)}`);
  return body.value.sessionId;
}

async function findByCss(sessionId, selector) {
  const res = await fetch(`${driverUrl}/session/${sessionId}/element`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ using: "css selector", value: selector }),
  });
  const body = await res.json();
  return body.value?.["element-6066-11e4-a52e-4f735466cecf"] ?? null;
}

async function findAllByCss(sessionId, selector) {
  const res = await fetch(`${driverUrl}/session/${sessionId}/elements`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ using: "css selector", value: selector }),
  });
  const body = await res.json();
  return (body.value ?? []).map((v) => v["element-6066-11e4-a52e-4f735466cecf"]);
}

async function dispatchReactInputEvent(sessionId, elementId) {
  await fetch(`${driverUrl}/session/${sessionId}/execute/sync`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      script: `
        const el = arguments[0];
        const proto = Object.getPrototypeOf(el);
        const desc = Object.getOwnPropertyDescriptor(proto, "value");
        desc.set.call(el, el.value);
        el.dispatchEvent(new Event("input", { bubbles: true }));
      `,
      args: [{ "element-6066-11e4-a52e-4f735466cecf": elementId }],
    }),
  });
}

async function execScript(sessionId, script, args = []) {
  const res = await fetch(`${driverUrl}/session/${sessionId}/execute/sync`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ script, args }),
  });
  return (await res.json()).value;
}

async function actionClick(sessionId, elementId) {
  await fetch(`${driverUrl}/session/${sessionId}/actions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      actions: [
        {
          type: "pointer",
          id: "mouse1",
          parameters: { pointerType: "mouse" },
          actions: [
            { type: "pointerMove", duration: 0, origin: { "element-6066-11e4-a52e-4f735466cecf": elementId }, x: 0, y: 0 },
            { type: "pointerDown", button: 0 },
            { type: "pause", duration: 50 },
            { type: "pointerUp", button: 0 },
          ],
        },
      ],
    }),
  });
}

async function keyCombo(sessionId, keys) {
  // keys: array of W3C key values to press together then release in reverse order.
  await fetch(`${driverUrl}/session/${sessionId}/actions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      actions: [
        {
          type: "key",
          id: "keyboard1",
          actions: [
            ...keys.map((k) => ({ type: "keyDown", value: k })),
            ...[...keys].reverse().map((k) => ({ type: "keyUp", value: k })),
          ],
        },
      ],
    }),
  });
}

async function sendKeys(sessionId, elementId, text) {
  await fetch(`${driverUrl}/session/${sessionId}/element/${elementId}/value`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ text }),
  });
}

async function screenshot(sessionId, filename) {
  const res = await fetch(`${driverUrl}/session/${sessionId}/screenshot`);
  const body = await res.json();
  await fs.writeFile(path.join(__dirname, filename), Buffer.from(body.value, "base64"));
}

async function getPageText(sessionId) {
  const bodyId = await findByCss(sessionId, "body");
  if (!bodyId) return "";
  const res = await fetch(`${driverUrl}/session/${sessionId}/element/${bodyId}/text`);
  return (await res.json()).value ?? "";
}

async function deleteSession(sessionId) {
  await fetch(`${driverUrl}/session/${sessionId}`, { method: "DELETE" }).catch(() => {});
}

function getClipboardText() {
  try {
    return execFileSync("powershell", ["-NoProfile", "-Command", "Get-Clipboard -Raw"], {
      encoding: "utf16le",
    }).trim();
  } catch {
    return "";
  }
}

async function fillPasswordFields(sessionId, password) {
  const inputs = await findAllByCss(sessionId, 'input[type="password"]');
  for (const el of inputs) {
    await sendKeys(sessionId, el, password);
    await dispatchReactInputEvent(sessionId, el);
  }
  return inputs;
}

async function main() {
  console.log("=== Phase 0: clean slate ===");
  if (existsSync(vaultDbPath)) {
    await fs.rm(vaultDbPath, { force: true });
    await fs.rm(vaultDbPath + "-wal", { force: true }).catch(() => {});
    await fs.rm(vaultDbPath + "-shm", { force: true }).catch(() => {});
    console.log(`Deleted pre-existing dev vault at ${vaultDbPath}`);
  }

  let driverProc = startTauriDriver();
  let sessionId;

  try {
    await waitForDriverReady();
    sessionId = await createSession();
    console.log("Session created (vault creation screen).");
    await sleep(3000);

    // --- Test: create the baseline vault ---
    const created = await fillPasswordFields(sessionId, PASSWORD);
    record("baseline: create screen has 2 password fields", created.length === 2, `found ${created.length}`);
    const submitBtn = await findByCss(sessionId, 'button[type="submit"]');
    await actionClick(sessionId, submitBtn);
    await sleep(4000);
    await screenshot(sessionId, "adversarial-01-vault-created.png");
    let url = await execScript(sessionId, "return location.pathname;");
    record("baseline: vault creation lands on /vault", url === "/vault", `got ${url}`);

    // --- Test: create a secret we can use as a canary for the "lock hides everything" test ---
    const canaryValue = "CANARY-SECRET-VALUE-should-never-appear-while-locked-9f3e";
    // Open the add-secret flow via keyboard shortcut (Ctrl+N), which every
    // route wires up identically, rather than hunting for a specific button.
    await keyCombo(sessionId, [CTRL_KEY, "n"]);
    await sleep(800);
    const nameInput = await findByCss(sessionId, 'input[placeholder="e.g. OPENAI_API_KEY"]');
    const valueInput = await findByCss(sessionId, 'input[type="password"]');
    if (nameInput && valueInput) {
      await sendKeys(sessionId, nameInput, "Adversarial Canary");
      await dispatchReactInputEvent(sessionId, nameInput);
      await sendKeys(sessionId, valueInput, canaryValue);
      await dispatchReactInputEvent(sessionId, valueInput);
      await screenshot(sessionId, "adversarial-02-canary-secret-form.png");
      const saveBtns = await findAllByCss(sessionId, "button");
      let saved = false;
      for (const b of saveBtns) {
        const text = await execScript(sessionId, "return arguments[0].textContent;", [
          { "element-6066-11e4-a52e-4f735466cecf": b },
        ]);
        if (text && text.includes("Save secret")) {
          await actionClick(sessionId, b);
          saved = true;
          break;
        }
      }
      await sleep(1500);
      record("canary secret: save form submitted", saved);
    } else {
      record("canary secret: could not open add-secret form", false, "Ctrl+N shortcut did not open the expected fields");
    }

    // --- Test: lock -> attempt access ---
    await keyCombo(sessionId, [CTRL_KEY, "l"]);
    await sleep(1500);
    await screenshot(sessionId, "adversarial-03-after-lock.png");
    const afterLockUrl = await execScript(sessionId, "return location.pathname;");
    const afterLockText = await getPageText(sessionId);
    record("lock: navigated away from /vault", afterLockUrl !== "/vault", `got ${afterLockUrl}`);
    record(
      "lock: canary secret value not present in DOM while locked",
      !afterLockText.includes(canaryValue),
      afterLockText.includes(canaryValue) ? "CANARY VALUE FOUND IN DOM -- real leak" : "not found, as expected",
    );

    // --- Test: wrong master password ---
    const wrongPwField = await findByCss(sessionId, 'input[aria-label="Master password"]');
    if (wrongPwField) {
      await sendKeys(sessionId, wrongPwField, "definitely-the-wrong-password");
      await dispatchReactInputEvent(sessionId, wrongPwField);
      const unlockSubmit = await findByCss(sessionId, 'button[type="submit"]');
      await actionClick(sessionId, unlockSubmit);
      await sleep(1500);
      await screenshot(sessionId, "adversarial-04-wrong-password.png");
      const wrongPwUrl = await execScript(sessionId, "return location.pathname;");
      const wrongPwText = await getPageText(sessionId);
      record("wrong password: still not on /vault", wrongPwUrl !== "/vault", `got ${wrongPwUrl}`);
      record(
        "wrong password: shows an error, not silent failure",
        /did not work|try again|incorrect|failed/i.test(wrongPwText),
        wrongPwText.slice(0, 200),
      );
      record("wrong password: canary secret value not exposed", !wrongPwText.includes(canaryValue));
    } else {
      record("wrong password test", false, "could not find the unlock password field");
    }

    // --- Test: correct unlock after a wrong attempt ---
    const rightPwField = await findByCss(sessionId, 'input[aria-label="Master password"]');
    await sendKeys(sessionId, rightPwField, PASSWORD);
    await dispatchReactInputEvent(sessionId, rightPwField);
    const unlockSubmit2 = await findByCss(sessionId, 'button[type="submit"]');
    await actionClick(sessionId, unlockSubmit2);
    await sleep(2500);
    const rightPwUrl = await execScript(sessionId, "return location.pathname;");
    record("correct password after a wrong attempt: unlocks normally", rightPwUrl === "/vault", `got ${rightPwUrl}`);
    await screenshot(sessionId, "adversarial-05-unlocked-again.png");

    // --- Test: large/unicode secret value ---
    await keyCombo(sessionId, [CTRL_KEY, "n"]);
    await sleep(800);
    const bigNameInput = await findByCss(sessionId, 'input[placeholder="e.g. OPENAI_API_KEY"]');
    const bigValueInput = await findByCss(sessionId, 'input[type="password"]');
    const unicodePayload = "\u{1F510}日本語密码Ключ" + "A".repeat(20000) + "END";
    let bigSaveWorked = false;
    if (bigNameInput && bigValueInput) {
      await sendKeys(sessionId, bigNameInput, "Adversarial Big Unicode Secret");
      await dispatchReactInputEvent(sessionId, bigNameInput);
      // Set the huge value directly via JS (WebDriver send-keys is far too
      // slow character-by-character for a 20k+ char string) and fire the
      // same input event React needs to notice it.
      await execScript(
        sessionId,
        `
        const el = arguments[0];
        const proto = Object.getPrototypeOf(el);
        const desc = Object.getOwnPropertyDescriptor(proto, "value");
        desc.set.call(el, arguments[1]);
        el.dispatchEvent(new Event("input", { bubbles: true }));
        `,
        [{ "element-6066-11e4-a52e-4f735466cecf": bigValueInput }, unicodePayload],
      );
      const saveBtns2 = await findAllByCss(sessionId, "button");
      for (const b of saveBtns2) {
        const text = await execScript(sessionId, "return arguments[0].textContent;", [
          { "element-6066-11e4-a52e-4f735466cecf": b },
        ]);
        if (text && text.includes("Save secret")) {
          await actionClick(sessionId, b);
          bigSaveWorked = true;
          break;
        }
      }
      await sleep(1500);
    }
    await screenshot(sessionId, "adversarial-06-after-large-unicode-save.png");
    const afterBigSaveCrashed = await execScript(sessionId, "return document.body ? document.body.innerHTML.length : -1;");
    record(
      "large/unicode secret: app did not crash (DOM still renders)",
      typeof afterBigSaveCrashed === "number" && afterBigSaveCrashed > 0,
      `body innerHTML length: ${afterBigSaveCrashed}`,
    );
    record("large/unicode secret: save flow completed without throwing", bigSaveWorked);

    // --- Test: malformed .env import ---
    // Make sure we're unlocked (previous steps end unlocked, but re-check defensively).
    const preImportUrl = await execScript(sessionId, "return location.pathname;");
    if (preImportUrl !== "/vault") {
      const pwField3 = await findByCss(sessionId, 'input[aria-label="Master password"]');
      if (pwField3) {
        await sendKeys(sessionId, pwField3, PASSWORD);
        await dispatchReactInputEvent(sessionId, pwField3);
        const submit3 = await findByCss(sessionId, 'button[type="submit"]');
        await actionClick(sessionId, submit3);
        await sleep(2000);
      }
    }
    const importTrigger = await execScript(sessionId, `
      const els = Array.from(document.querySelectorAll('button, a'));
      const match = els.find(e => /import/i.test(e.textContent || ""));
      if (match) { match.click(); return true; }
      return false;
    `);
    await sleep(800);
    if (importTrigger) {
      const textarea = await findByCss(sessionId, "textarea");
      if (textarea) {
        const malformed = "not-a-valid-line\n====\n\n# comment only\nNOVALUEHERE\n" + "=".repeat(500);
        await execScript(
          sessionId,
          `
          const el = arguments[0];
          const proto = Object.getPrototypeOf(el);
          const desc = Object.getOwnPropertyDescriptor(proto, "value");
          desc.set.call(el, arguments[1]);
          el.dispatchEvent(new Event("input", { bubbles: true }));
          `,
          [{ "element-6066-11e4-a52e-4f735466cecf": textarea }, malformed],
        );
        const continueBtn = await execScript(sessionId, `
          const els = Array.from(document.querySelectorAll('button'));
          const match = els.find(e => /continue/i.test(e.textContent || ""));
          if (match) { match.click(); return true; }
          return false;
        `);
        await sleep(1000);
        await screenshot(sessionId, "adversarial-07-malformed-env-import.png");
        const stillAlive = await execScript(sessionId, "return document.body ? document.body.innerHTML.length : -1;");
        record(
          "malformed .env import: app did not crash",
          typeof stillAlive === "number" && stillAlive > 0,
          `continue clicked: ${continueBtn}, body length: ${stillAlive}`,
        );
      } else {
        record("malformed .env import test", false, "import modal opened but no textarea found");
      }
    } else {
      record("malformed .env import test", false, "no Import trigger found on this page -- not exercised");
    }

    // --- Test: clipboard copy + expiry ---
    await keyCombo(sessionId, [ESCAPE_KEY]);
    await sleep(500);
    const copyBtn = await execScript(sessionId, `
      const els = Array.from(document.querySelectorAll('button[aria-label="Copy secret"]'));
      if (els.length) { els[0].click(); return true; }
      return false;
    `);
    await sleep(1000);
    if (copyBtn) {
      const clipboardImmediately = getClipboardText();
      record(
        "clipboard: copy populates the real Windows clipboard",
        clipboardImmediately.length > 0,
        `clipboard length: ${clipboardImmediately.length}`,
      );
      console.log("Waiting 32s for the default clipboard-clear timer...");
      await sleep(32000);
      const clipboardAfterWait = getClipboardText();
      record(
        "clipboard: clears itself after the configured delay",
        clipboardAfterWait !== clipboardImmediately || clipboardAfterWait.length === 0,
        `before: ${clipboardImmediately.length} chars, after: ${clipboardAfterWait.length} chars`,
      );
    } else {
      record("clipboard copy test", false, "could not find a 'Copy secret' button to click");
    }

    await screenshot(sessionId, "adversarial-08-final-state.png");
  } finally {
    if (sessionId) await deleteSession(sessionId);
    driverProc.kill();
    await sleep(1000);
  }

  // --- Test: restart while unlocked never resurrects an unlocked session ---
  console.log("=== Phase: restart-while-unlocked ===");
  driverProc = startTauriDriver();
  try {
    await waitForDriverReady();
    sessionId = await createSession();
    await sleep(3000);
    const freshUrl = await execScript(sessionId, "return location.pathname;");
    const freshText = await getPageText(sessionId);
    await screenshot(sessionId, "adversarial-09-fresh-launch-after-restart.png");
    record(
      "restart: a fresh launch requires the password again (no persisted unlock)",
      freshUrl !== "/vault",
      `got ${freshUrl}`,
    );
    record("restart: canary secret not visible on fresh launch", !freshText.includes("CANARY-SECRET-VALUE"));
  } finally {
    if (sessionId) await deleteSession(sessionId);
    driverProc.kill();
    await sleep(1000);
  }

  // --- Test: tampered/corrupted vault file ---
  console.log("=== Phase: tampered vault file ===");
  const dbExists = existsSync(vaultDbPath);
  if (dbExists) {
    const original = await fs.readFile(vaultDbPath);
    const corrupted = Buffer.from(original);
    // Flip bytes in the middle third of the file -- avoids only touching the
    // SQLite header (which has its own separate, less interesting check) and
    // instead corrupts actual page content.
    const start = Math.floor(corrupted.length / 3);
    const end = Math.floor((corrupted.length * 2) / 3);
    for (let i = start; i < end; i += 7) corrupted[i] = corrupted[i] ^ 0xff;
    await fs.writeFile(vaultDbPath, corrupted);

    driverProc = startTauriDriver();
    try {
      await waitForDriverReady();
      sessionId = await createSession();
      await sleep(3000);
      await screenshot(sessionId, "adversarial-10-tampered-vault-launch.png");
      const tamperedPwField = await findByCss(sessionId, 'input[aria-label="Master password"]');
      if (tamperedPwField) {
        await sendKeys(sessionId, tamperedPwField, PASSWORD);
        await dispatchReactInputEvent(sessionId, tamperedPwField);
        const tamperedSubmit = await findByCss(sessionId, 'button[type="submit"]');
        await actionClick(sessionId, tamperedSubmit);
        await sleep(2000);
        const tamperedResultUrl = await execScript(sessionId, "return location.pathname;");
        await screenshot(sessionId, "adversarial-11-tampered-vault-unlock-attempt.png");
        const cleanRefusal = tamperedResultUrl !== "/vault";
        record(
          "tampered vault: corrupted data does not unlock, fails cleanly (no crash, no silent success)",
          cleanRefusal,
          `url after attempted unlock: ${tamperedResultUrl}`,
        );
        const stillResponsive = await execScript(sessionId, "return document.body ? document.body.innerHTML.length : -1;");
        record(
          "tampered vault: app window is still responsive after the failure (did not crash)",
          typeof stillResponsive === "number" && stillResponsive > 0,
        );
      } else {
        record(
          "tampered vault test",
          false,
          "app did not show an unlock/password screen at all for the corrupted file -- inspect screenshot",
        );
      }
    } finally {
      if (sessionId) await deleteSession(sessionId);
      driverProc.kill();
      await sleep(1000);
    }
  } else {
    record("tampered vault test", false, "no vault.db file existed to tamper with");
  }

  // --- Cleanup ---
  await fs.rm(vaultDbPath, { force: true }).catch(() => {});
  await fs.rm(vaultDbPath + "-wal", { force: true }).catch(() => {});
  await fs.rm(vaultDbPath + "-shm", { force: true }).catch(() => {});
  console.log(`Cleaned up dev vault at ${vaultDbPath}`);

  console.log("\n=== SUMMARY ===");
  for (const r of results) console.log(`${r.pass ? "PASS" : "FAIL"}: ${r.name}`);
  const failed = results.filter((r) => !r.pass);
  console.log(`\n${results.length - failed.length}/${results.length} passed.`);
  if (failed.length > 0) {
    console.log("FAILURES:");
    for (const f of failed) console.log(`  - ${f.name}: ${f.detail ?? ""}`);
    process.exitCode = 1;
  }
}

main().catch((err) => {
  console.error("Adversarial smoke test crashed:", err);
  process.exitCode = 1;
});
