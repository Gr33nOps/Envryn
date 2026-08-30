#!/usr/bin/env node
/**
 * Copy Envryn's branded Android launcher icons into the generated Android
 * project, overwriting whatever `cargo tauri android init` put there.
 *
 * **Why this exists.** `cargo tauri android init` scaffolds
 * `src-tauri/gen/android/` from Tauri's own mobile template, which ships its
 * own placeholder launcher icon (an orange/teal "8" mark) -- it does not pull
 * from `src-tauri/icons/android/`, the branded set `tauri icon` already
 * generated there, even though that directory existed before `android init`
 * ran. The result, confirmed by diffing every density: every generated
 * `ic_launcher*.png` in `gen/android` was still Tauri's placeholder, not
 * Envryn's hexagon mark -- the app installed and ran correctly, it just
 * looked like an unbranded template on the home screen and app switcher.
 *
 * `src-tauri/gen/android/` is generated and `.gitignore`d, so fixing the
 * files there once does not survive a fresh clone or a re-init -- this
 * script is the fix that does, run before each Android build the same way
 * `.dev-tools/prepare-sidecar.mjs` and `.dev-tools/sign-apk.mjs` are.
 *
 * Usage:
 *   node .dev-tools/sync-android-icons.mjs
 */

import { existsSync, copyFileSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

function fail(message) {
  console.error(`sync-android-icons: ${message}`);
  process.exit(1);
}

const repoRoot = resolve(import.meta.dirname, "..");
const srcRoot = join(repoRoot, "src-tauri/icons/android");
const genRoot = join(repoRoot, "src-tauri/gen/android/app/src/main/res");

if (!existsSync(srcRoot)) fail(`no source icons at ${srcRoot} -- run "cargo tauri icon"`);
if (!existsSync(genRoot)) {
  console.log(
    `sync-android-icons: no generated project at ${genRoot} yet -- nothing to sync ` +
      `(run "cargo tauri android init" first, then re-run this).`,
  );
  process.exit(0);
}

let copied = 0;
for (const resourceDir of readdirSync(srcRoot)) {
  const srcDir = join(srcRoot, resourceDir);
  if (!statSync(srcDir).isDirectory()) continue;
  const genDir = join(genRoot, resourceDir);
  // Adaptive icon XML and its color resource may not exist in Tauri's
  // generated template. Create the resource directory so modern Android
  // launchers use Envryn's foreground and dark background instead of
  // silently falling back to the template or a transparent legacy bitmap.
  mkdirSync(genDir, { recursive: true });
  for (const file of readdirSync(srcDir)) {
    const from = join(srcDir, file);
    if (!statSync(from).isFile()) continue;
    copyFileSync(from, join(genDir, file));
    copied += 1;
  }
}

if (copied === 0) {
  fail("copied nothing -- source and generated directory layouts no longer match, check by hand");
}
console.log(`sync-android-icons: copied ${copied} branded icon file(s) into gen/android`);
