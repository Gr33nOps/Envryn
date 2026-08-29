#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const failures = [];

function read(path) {
  return readFileSync(join(root, path), "utf8");
}

function requireInvariant(condition, message) {
  if (!condition) failures.push(message);
}

const tauri = JSON.parse(read("src-tauri/tauri.conf.json"));
const csp = tauri.app?.security?.csp ?? {};
requireInvariant(csp["default-src"] === "'self'", "CSP default-src must remain self-only");
requireInvariant(csp["script-src"] === "'self'", "CSP must not permit remote or inline scripts");
requireInvariant(csp["object-src"] === "'none'", "CSP must block embedded objects");
requireInvariant(csp["frame-ancestors"] === "'none'", "CSP must block framing");
requireInvariant(
  tauri.app?.security?.freezePrototype === true,
  "prototype freezing must remain enabled",
);
requireInvariant(
  tauri.bundle?.android?.minSdkVersion >= 29,
  "Android must not install on platform versions that no longer receive security fixes",
);

const capabilities = JSON.parse(read("src-tauri/capabilities/default.json"));
const forbiddenPermissions = ["fs:", "shell:", "http:", "clipboard-manager:"];
for (const permission of capabilities.permissions ?? []) {
  requireInvariant(
    !forbiddenPermissions.some((prefix) => permission.startsWith(prefix)),
    `frontend capability must not include ${permission}`,
  );
}

const packageJson = JSON.parse(read("package.json"));
requireInvariant(
  packageJson.scripts?.["build:native-ui"]?.includes("patch:android-mdns"),
  "every native build must apply the Android platform hardening patch",
);

for (const workflow of readdirSync(join(root, ".github", "workflows"), {
  withFileTypes: true,
})) {
  if (!workflow.isFile() || !/\.ya?ml$/i.test(workflow.name)) continue;
  const source = read(`.github/workflows/${workflow.name}`);
  requireInvariant(
    !/(?:^|\n)\s*(?:-\s*)?(?:run:\s*)?npx(?:\s|$)/m.test(source),
    `${workflow.name} must execute npm-locked binaries directly instead of npx`,
  );
}

const patchSource = read(".dev-tools/patch-android-mdns.mjs");
for (const marker of [
  'android:allowBackup="false"',
  'android:fullBackupContent="false"',
  "WindowManager.LayoutParams.FLAG_SECURE",
  "CHANGE_WIFI_MULTICAST_STATE",
  'replace(/minSdk\\s*=\\s*24/, "minSdk = 29")',
]) {
  requireInvariant(patchSource.includes(marker), `Android patch is missing ${marker}`);
}

const clipboard = read(
  "crates/envryn-android-clipboard/android/src/main/java/SensitiveClipboardPlugin.kt",
);
requireInvariant(
  clipboard.includes("android.content.extra.IS_SENSITIVE"),
  "Android secret clipboard entries must be marked sensitive",
);
requireInvariant(
  clipboard.includes("clearPrimaryClip"),
  "Android clipboard bridge must support timed clearing",
);

const vaultShell = read("apps/ui/src/routes/vault/route.tsx");
requireInvariant(
  vaultShell.includes('document.addEventListener("visibilitychange"'),
  "Android vault must lock when the activity is backgrounded",
);

const generatedManifest = "src-tauri/gen/android/app/src/main/AndroidManifest.xml";
if (existsSync(join(root, generatedManifest))) {
  const manifest = read(generatedManifest);
  for (const marker of [
    'android:allowBackup="false"',
    'android:usesCleartextTraffic="${usesCleartextTraffic}"',
    "android.permission.CHANGE_WIFI_MULTICAST_STATE",
  ]) {
    requireInvariant(manifest.includes(marker), `generated Android manifest is missing ${marker}`);
  }
}

const generatedActivity =
  "src-tauri/gen/android/app/src/main/java/dev/envryn/vault/MainActivity.kt";
if (existsSync(join(root, generatedActivity))) {
  const activity = read(generatedActivity);
  requireInvariant(
    activity.includes("WindowManager.LayoutParams.FLAG_SECURE"),
    "generated Android activity must block capture",
  );
  requireInvariant(
    activity.includes("createMulticastLock"),
    "generated Android activity must acquire the mDNS multicast lock",
  );
}

const generatedGradle = "src-tauri/gen/android/app/build.gradle.kts";
if (existsSync(join(root, generatedGradle))) {
  requireInvariant(
    read(generatedGradle).includes("minSdk = 29"),
    "generated Android build must require Android 10 or newer",
  );
}

if (failures.length > 0) {
  console.error("Security invariant verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Security invariants verified.");
