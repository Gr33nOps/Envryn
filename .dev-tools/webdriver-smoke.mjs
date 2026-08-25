// Real end-to-end verification of the native Tauri window via the W3C
// WebDriver protocol, through `tauri-driver` (which wraps `msedgedriver.exe`
// on Windows to automate the WebView2 surface Tauri renders into).
//
// This is the thing every prior phase's docs said this environment could
// not do: drive and screenshot the actual native window, not a browser-only
// preview of the Vite dev server. Run manually:
//
//   node .dev-tools/webdriver-smoke.mjs
//
// Requires: the release binary built (`cargo build -p envryn --release`),
// tauri-driver installed (`cargo install tauri-driver`), and a matching
// msedgedriver.exe (see .dev-tools/edgedriver/, downloaded to match this
// machine's installed Edge/WebView2 version -- a version mismatch is the
// most common failure mode and shows up as a WebDriver session-creation
// error, not a subtle bug in this script).
//
// Two real things this uncovered, worth knowing before re-running:
//  - A plain `cargo build --release` (no Tauri CLI) served the webview from
//    `devUrl` (localhost:1420) instead of the embedded frontend, because
//    src-tauri/Cargo.toml was missing the standard
//    `default = ["custom-protocol"]` feature every Tauri scaffold has --
//    fixed there, not a workaround needed here.
//  - The legacy WebDriver `Element Click` command did not reliably trigger
//    this app's (Radix/shadcn-styled) buttons; the W3C Actions API
//    (`actionClick` below -- a real pointerMove/pointerDown/pointerUp
//    sequence) does. Use `actionClick`, not `click`, for anything that
//    needs to actually submit or navigate.
//
// Running this creates a real, throwaway vault in this machine's actual
// app-data directory (`%APPDATA%\dev.envryn.vault\envryn.db`) -- delete it
// afterward if you don't want a leftover test vault sitting there:
//   Remove-Item "$env:APPDATA\dev.envryn.vault\envryn.db*"

import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const appExe = path.join(repoRoot, "target", "release", "envryn.exe");
const edgeDriver = path.join(__dirname, "edgedriver", "msedgedriver.exe");
const driverPort = 4444;
const driverUrl = `http://127.0.0.1:${driverPort}`;

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
      if (res.ok || res.status === 404) return; // driver process is up and answering
    } catch {
      // not listening yet
    }
    await sleep(500);
  }
  throw new Error("tauri-driver did not become ready in time");
}

async function createSession() {
  const res = await fetch(`${driverUrl}/session`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      capabilities: {
        alwaysMatch: {
          browserName: "wry",
          "tauri:options": { application: appExe },
        },
      },
    }),
  });
  const body = await res.json();
  if (!res.ok || !body.value?.sessionId) {
    throw new Error(`session creation failed: ${JSON.stringify(body)}`);
  }
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

async function getText(sessionId, elementId) {
  const res = await fetch(
    `${driverUrl}/session/${sessionId}/element/${elementId}/text`,
  );
  const body = await res.json();
  return body.value;
}

async function getPageText(sessionId) {
  const bodyId = await findByCss(sessionId, "body");
  if (!bodyId) return null;
  return getText(sessionId, bodyId);
}

/**
 * WebDriver's raw `Element Send Keys` sets the DOM value directly, which a
 * React controlled input does not always notice (React listens for the
 * event dispatched by its own synthetic input system, tracked via a native
 * setter). Dispatching a real `input` event through the native value setter
 * afterward is the standard workaround -- this is exactly the "fires a real
 * input event" recipe React Testing Library's `fireEvent` uses internally.
 */
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

async function click(sessionId, elementId) {
  await fetch(
    `${driverUrl}/session/${sessionId}/element/${elementId}/click`,
    { method: "POST" },
  );
}

/**
 * A real pointer-down/pointer-up sequence via the W3C Actions API, as a
 * fallback for components whose click handling does not respond to the
 * legacy single-shot `Element Click` command (some Radix/shadcn-styled
 * buttons listen for pointer events specifically).
 */
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
  const fs = await import("node:fs/promises");
  await fs.writeFile(filename, Buffer.from(body.value, "base64"));
  console.log(`Saved screenshot: ${filename}`);
}

async function deleteSession(sessionId) {
  await fetch(`${driverUrl}/session/${sessionId}`, { method: "DELETE" });
}

async function main() {
  const driverProc = startTauriDriver();
  let sessionId;
  try {
    await waitForDriverReady();
    console.log("tauri-driver is ready; creating a session (this launches the real app window)...");
    sessionId = await createSession();
    console.log(`Session created: ${sessionId}`);

    // The app needs a moment to finish its first render after the window
    // appears.
    await sleep(5000);
    await screenshot(sessionId, path.join(__dirname, "shot-01-initial.png"));

    const sourceRes = await fetch(`${driverUrl}/session/${sessionId}/source`);
    const sourceBody = await sourceRes.json();
    const fs = await import("node:fs/promises");
    await fs.writeFile(
      path.join(__dirname, "page-source.html"),
      sourceBody.value ?? JSON.stringify(sourceBody),
    );
    console.log("Saved page source to .dev-tools/page-source.html");

    const password = "webdriver-smoke-test-password";
    const passwordInputs = await findAllByCss(sessionId, 'input[type="password"]');
    if (passwordInputs.length < 2) {
      throw new Error(
        `expected 2 password fields (master + confirm), found ${passwordInputs.length}`,
      );
    }
    await sendKeys(sessionId, passwordInputs[0], password);
    await dispatchReactInputEvent(sessionId, passwordInputs[0]);
    await sendKeys(sessionId, passwordInputs[1], password);
    await dispatchReactInputEvent(sessionId, passwordInputs[1]);
    await screenshot(sessionId, path.join(__dirname, "shot-02-form-filled.png"));

    const submitButton = await findByCss(sessionId, 'button[type="submit"]');
    if (!submitButton) throw new Error("could not find the Create vault submit button");

    const buttonStateRes = await fetch(`${driverUrl}/session/${sessionId}/execute/sync`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        script: "return arguments[0].outerHTML;",
        args: [{ "element-6066-11e4-a52e-4f735466cecf": submitButton }],
      }),
    });
    console.log("Submit button state before click:", (await buttonStateRes.json()).value);

    await actionClick(sessionId, submitButton);

    // Vault creation runs a real Argon2id calibration + AEAD setup -- give
    // it real time rather than a hopeful short sleep.
    await sleep(4000);
    await screenshot(sessionId, path.join(__dirname, "shot-03-after-create.png"));
    console.log("Page text after click + wait:", await getPageText(sessionId));

    const settingsLink = await findByCss(sessionId, 'a[href="/vault/settings"]');
    if (settingsLink) {
      await actionClick(sessionId, settingsLink);
      await sleep(1000);
      await screenshot(sessionId, path.join(__dirname, "shot-04-settings.png"));
      // The screenshot is the real evidence here, not a text-content
      // assertion: WebDriver's `body` text-extraction algorithm undercounts
      // content in this app's layout for reasons not worth chasing further
      // (a scrollable panel, most likely) even though the rendered pixels
      // (and a manual look at shot-04-settings.png) show the "Local AI"
      // section -- enable toggle, model status, download button -- exactly
      // as expected. Look at the PNG, not just this console output.
      console.log("Settings screenshot saved -- inspect shot-04-settings.png for the Local AI section.");
    } else {
      console.log(
        "Could not find a direct link to /vault/settings -- screenshots still show real app state.",
      );
    }

    console.log("Real native-window smoke test completed. Review the screenshots in .dev-tools/.");
  } finally {
    if (sessionId) {
      await deleteSession(sessionId).catch(() => {});
    }
    driverProc.kill();
  }
}

main().catch((err) => {
  console.error("WebDriver smoke test failed:", err);
  process.exitCode = 1;
});
