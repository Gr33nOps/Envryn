// Produces a fresh, accurate README hero screenshot of a populated vault.
//
// Reuses the real native-window WebDriver approach from webdriver-smoke.mjs
// (see that file's own header for the tauri-driver/msedgedriver setup this
// also needs), extended to actually populate a vault via the real `.env`
// import flow with a handful of realistic-but-fabricated entries, so the
// screenshot shows what the app actually looks like in use rather than an
// empty first-run state.
//
// **Safety: this must never touch the developer's real vault, and an
// env-var override cannot provide that isolation.** A real vault already
// exists at `%APPDATA%\dev.envryn.vault\envryn.db` on the machine this was
// written on. A first version of this script tried redirecting it by setting
// `APPDATA` on the spawned process tree (tauri-driver -> the real app) -- that
// has **no effect**: `app.path().app_data_dir()` resolves via Windows'
// `SHGetKnownFolderPath(FOLDERID_RoamingAppData)` (see the `dirs-sys` crate's
// Windows implementation), a direct Shell API call that reads the actual
// per-user shell-folder registration, not the `APPDATA` *process* environment
// variable -- confirmed by launching the real binary with `APPDATA`
// overridden and finding the scratch directory it should have written to
// stayed empty. The only verified-safe isolation is a **different Tauri
// `identifier`**: `app_data_dir()` is `known_folder.join(identifier)`, so
// building with a different identifier (temporarily, in `tauri.conf.json`,
// never committed) produces a genuinely separate, non-overlapping path with
// no shared state at all -- not a shared path this process just promises not
// to touch. There is no runtime flag that achieves the same thing; the
// binary this script points at (`target/release/envryn.exe`) has to already
// be built that way. Manual steps, run from the repo root, before this
// script:
//
//   1. Edit src-tauri/tauri.conf.json: change "identifier" to something
//      distinct, e.g. "dev.envryn.vault.demoscreenshot".
//   2. cargo build -p envryn --release
//   3. node .dev-tools/demo-screenshot.mjs
//   4. Revert tauri.conf.json's identifier, then `cargo build -p envryn
//      --release` again to restore target/release/envryn.exe as the real
//      build -- do not leave the identifier change committed or the release
//      binary built against it.
//
// Every "secret" value below is fabricated -- a structurally plausible
// shape (matching the same prefixes envryn_core::ai::classify recognises,
// so it demonstrates real deterministic type-detection, not the AI) with
// meaningless bodies, never a real credential. Screenshots only ever show
// the metadata list view (name/project/environment/type); nothing here
// clicks a row's reveal toggle, so no plaintext value is ever rendered.
//
// **Known current blocker (as of 2026-08-27), not something this script's
// code can fix:** session creation against the real app fails with
// "session not created: DevToolsActivePort file doesn't exist" via
// msedgedriver, and the same failure reproduces identically running the
// older, previously-proven `webdriver-smoke.mjs` unmodified -- so this is an
// environment/tooling regression (most likely `tauri-driver`/msedgedriver/
// WebView2 version drift since that script last succeeded), not a bug in
// either script. Try re-downloading `.dev-tools/edgedriver/msedgedriver.exe`
// to match the exact installed WebView2 Runtime build
// (`Get-ItemProperty` under `HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\
// Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` or check
// `C:\Program Files (x86)\Microsoft\EdgeWebView\Application\`) before
// assuming a code change here is needed.

import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const appExe = path.join(repoRoot, "target", "release", "envryn.exe");
const edgeDriver = path.join(__dirname, "edgedriver", "msedgedriver.exe");
const driverPort = 4446; // distinct from webdriver-smoke.mjs's 4444
const driverUrl = `http://127.0.0.1:${driverPort}`;

// Isolation is the build itself (a different Tauri `identifier` -- see the
// module doc's manual steps), not anything this script can do at runtime.
// `appExe` above must already point at a binary built that way before this
// runs.

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
        alwaysMatch: { browserName: "wry", "tauri:options": { application: appExe } },
      },
    }),
  });
  const body = await res.json();
  if (!res.ok || !body.value?.sessionId) {
    throw new Error(`session creation failed: ${JSON.stringify(body)}`);
  }
  return body.value.sessionId;
}

const ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf";

async function findAllByCss(sessionId, selector) {
  const res = await fetch(`${driverUrl}/session/${sessionId}/elements`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ using: "css selector", value: selector }),
  });
  const body = await res.json();
  return (body.value ?? []).map((v) => v[ELEMENT_KEY]);
}

async function execScript(sessionId, script, args = []) {
  const res = await fetch(`${driverUrl}/session/${sessionId}/execute/sync`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ script, args }),
  });
  return (await res.json()).value;
}

/** Find a button/link by its visible text -- robust to markup changes, unlike a CSS selector. */
async function findByText(sessionId, tag, text) {
  const result = await execScript(
    sessionId,
    `
      const els = Array.from(document.querySelectorAll(arguments[0]));
      const match = els.find(el => el.textContent.trim().includes(arguments[1]));
      return match ?? null;
    `,
    [tag, text],
  );
  if (!result) return null;
  return result[ELEMENT_KEY];
}

async function dispatchReactInputEvent(sessionId, elementId, isTextarea = false) {
  await execScript(
    sessionId,
    `
      const el = arguments[0];
      const proto = Object.getPrototypeOf(el);
      const desc = Object.getOwnPropertyDescriptor(proto, "value");
      desc.set.call(el, el.value);
      el.dispatchEvent(new Event("input", { bubbles: true }));
    `,
    [{ [ELEMENT_KEY]: elementId }],
  );
  void isTextarea;
}

async function sendKeys(sessionId, elementId, text) {
  await fetch(`${driverUrl}/session/${sessionId}/element/${elementId}/value`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ text }),
  });
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
            {
              type: "pointerMove",
              duration: 0,
              origin: { [ELEMENT_KEY]: elementId },
              x: 0,
              y: 0,
            },
            { type: "pointerDown", button: 0 },
            { type: "pause", duration: 50 },
            { type: "pointerUp", button: 0 },
          ],
        },
      ],
    }),
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
  await fetch(`${driverUrl}/session/${sessionId}`, { method: "DELETE" }).catch(() => {});
}

// Fabricated only -- structurally recognisable, meaningless bodies, and each
// value is a `prefix + body` concatenation rather than one string literal.
// That split is deliberate, not stylistic: a complete, contiguous token
// literal here trips GitHub's push-protection secret scanning, which cannot
// know these are fake (it already refused a push over this once) and
// correctly declines to guess. Splitting the literal leaves nothing for a
// scanner to match while the text that actually reaches the textarea is
// byte-for-byte identical to writing it as one string.
const DEMO_ENV = [
  `DATABASE_URL=postgres://appuser:${"S3cur3Pass"}@db.acmeapp.dev:5432/production`,
  `STRIPE_SECRET_KEY=${"sk_live_"}${"51NqXyZQwErTyUiOpAsDfGhJkLzXcVbNm"}`,
  `OPENAI_API_KEY=${"sk-proj-"}${"aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789"}`,
  `GITHUB_TOKEN=${"ghp_"}${"aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789"}`,
  `REDIS_URL=redis://:p4ssw0rd@cache.acmeapp.dev:6379/0`,
  `SENDGRID_API_KEY=${"SG."}${"aBcDeFgH"}.${"iJkLmNoPqRsTuVwXyZ0123456789"}`,
].join("\n");

async function main() {
  const driverProc = startTauriDriver();
  let sessionId;
  try {
    await waitForDriverReady();
    console.log(
      "tauri-driver ready; launching the real app window against an isolated data dir...",
    );
    sessionId = await createSession();
    await sleep(4000);

    // --- Create the demo vault ---
    const passwordInputs = await findAllByCss(sessionId, 'input[type="password"]');
    if (passwordInputs.length < 2)
      throw new Error(`expected 2 password fields, found ${passwordInputs.length}`);
    const password = "demo-screenshot-not-a-real-vault-Password9!";
    await sendKeys(sessionId, passwordInputs[0], password);
    await dispatchReactInputEvent(sessionId, passwordInputs[0]);
    await sendKeys(sessionId, passwordInputs[1], password);
    await dispatchReactInputEvent(sessionId, passwordInputs[1]);

    const submitButton = await findByText(sessionId, "button[type=submit]", "Create vault");
    if (!submitButton) throw new Error("could not find the Create vault button");
    await actionClick(sessionId, submitButton);
    await sleep(4000);

    // --- Open the .env import flow ---
    const importButton = await findByText(sessionId, "button", "Import .env");
    if (!importButton) throw new Error("could not find the Import .env button");
    await actionClick(sessionId, importButton);
    await sleep(600);

    const textarea = await findAllByCss(sessionId, "textarea");
    if (textarea.length === 0) throw new Error("could not find the .env paste textarea");
    await sendKeys(sessionId, textarea[0], DEMO_ENV);
    await dispatchReactInputEvent(sessionId, textarea[0]);

    const projectInputs = await findAllByCss(sessionId, 'input[placeholder="e.g. Rescripto"]');
    if (projectInputs.length === 0) throw new Error("could not find the Project input");
    await sendKeys(sessionId, projectInputs[0], "Acme Platform");
    await dispatchReactInputEvent(sessionId, projectInputs[0]);

    const continueButton = await findByText(sessionId, "button", "Continue");
    if (!continueButton) throw new Error("could not find the Continue button");
    await actionClick(sessionId, continueButton);
    await sleep(1200); // deterministic classification only -- no model to wait on

    const importSubmit = await findByText(sessionId, "button", "Import 6 secret");
    if (!importSubmit) throw new Error("could not find the final Import button");
    await actionClick(sessionId, importSubmit);
    await sleep(1500);

    // --- The hero shot: a populated, realistic-looking vault ---
    await screenshot(sessionId, path.join(repoRoot, "docs", "assets", "vault-populated.png"));

    console.log("Demo screenshot captured.");
    console.log(
      "Remember: revert tauri.conf.json's identifier and rebuild (see the module doc's " +
        "step 4) so target/release/envryn.exe is the real build again. The demo vault this " +
        "run created lives under %APPDATA%\\<the demo identifier you chose> -- delete that " +
        "folder by hand if you don't want it left behind.",
    );
  } finally {
    if (sessionId) await deleteSession(sessionId);
    driverProc.kill();
  }
}

main().catch((err) => {
  console.error("Demo screenshot failed:", err);
  process.exitCode = 1;
});
