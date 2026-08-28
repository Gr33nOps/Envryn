#!/usr/bin/env node
/**
 * Patch the generated Android project with the platform integration Tauri's
 * generic template cannot supply: mDNS discovery plus privacy hardening.
 *
 * **Why this exists.** `crates/envryn-core/src/sync/discovery.rs` (via the
 * `mdns-sd` crate) sends and receives real UDP multicast on
 * `224.0.0.251:5353`. Android drops *incoming* multicast packets for an app
 * by default unless the app holds a `WifiManager.MulticastLock` -- `mdns-sd`
 * has no Android-aware code to acquire one itself (confirmed by inspecting
 * its source: it is a plain, cross-platform socket library with no JNI calls
 * into the Android framework at all). Without this patch, Sync's "no trusted
 * device found on this network" is not a bug in the discovery *logic* --
 * pairing (a direct, manually-entered TCP connection, no multicast involved)
 * works fine on the same build, proving the network stack itself is not
 * broken -- it is specifically that Android silently drops the multicast
 * packets before the Rust code ever sees them.
 *
 * `cargo tauri android init` scaffolds `MainActivity.kt` and
 * `AndroidManifest.xml` fresh; both live in `src-tauri/gen/android/`, which
 * is `.gitignore`d, so a fix made there directly does not survive a fresh
 * clone or a re-init -- this script is what makes it durable, the same
 * reason `.dev-tools/sync-android-icons.mjs` exists for the launcher icon.
 * Idempotent: safe to run whether or not the project has already been
 * patched (checks for its own marker string first).
 *
 * Usage (after `cargo tauri android init`, before `cargo tauri android
 * build`):
 *   node .dev-tools/patch-android-mdns.mjs
 */

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

function fail(message) {
  console.error(`patch-android-mdns: ${message}`);
  process.exit(1);
}

const repoRoot = resolve(import.meta.dirname, "..");
const genAndroid = join(repoRoot, "src-tauri/gen/android");
const manifestPath = join(genAndroid, "app/src/main/AndroidManifest.xml");
const activityPath = join(genAndroid, "app/src/main/java/dev/envryn/vault/MainActivity.kt");
const buildGradlePath = join(genAndroid, "app/build.gradle.kts");

if (!existsSync(genAndroid)) {
  console.log(
    `patch-android-mdns: no generated project at ${genAndroid} yet -- nothing to patch ` +
      `(run "cargo tauri android init" first, then re-run this).`,
  );
  process.exit(0);
}

// --- AndroidManifest.xml: the three permissions the multicast lock needs ---
if (!existsSync(manifestPath)) fail(`no AndroidManifest.xml at ${manifestPath}`);
let manifest = readFileSync(manifestPath, "utf8");
const MARKER = "CHANGE_WIFI_MULTICAST_STATE";
if (manifest.includes(MARKER)) {
  console.log("patch-android-mdns: AndroidManifest.xml already has the multicast permissions");
} else {
  const injected = manifest.replace(
    '<uses-permission android:name="android.permission.INTERNET" />',
    [
      '<uses-permission android:name="android.permission.INTERNET" />',
      "    <!-- Required to hold a WifiManager.MulticastLock (see MainActivity.kt's",
      "         onCreate). Without it, Android silently drops incoming mDNS",
      "         multicast packets and device-sync discovery finds nothing. -->",
      '    <uses-permission android:name="android.permission.ACCESS_WIFI_STATE" />',
      '    <uses-permission android:name="android.permission.CHANGE_WIFI_MULTICAST_STATE" />',
      '    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />',
    ].join("\n"),
  );
  if (injected === manifest) {
    fail(
      "AndroidManifest.xml does not contain the expected INTERNET permission line to anchor " +
        "on -- the generated template may have changed; patch by hand and update this script.",
    );
  }
  manifest = injected;
  writeFileSync(manifestPath, manifest);
  console.log("patch-android-mdns: added multicast permissions to AndroidManifest.xml");
}

// The encrypted vault belongs in Android's app-private sandbox and must never
// be copied into an ADB/cloud/device-transfer backup by the operating system.
if (!manifest.includes('android:allowBackup="false"')) {
  const hardened = manifest.replace(
    "<application\n",
    '<application\n        android:allowBackup="false"\n        android:fullBackupContent="false"\n',
  );
  if (hardened === manifest) {
    fail("AndroidManifest.xml has no expected <application> anchor for backup hardening");
  }
  manifest = hardened;
  writeFileSync(manifestPath, manifest);
  console.log("patch-android-mdns: disabled Android backup and device-transfer extraction");
}

// Android 7-9 no longer receive platform security fixes. Envryn holds vault
// material and should not claim compatibility with an unpatchable OS, even
// though Tauri's generic template still defaults to API 24.
if (!existsSync(buildGradlePath)) fail(`no app/build.gradle.kts at ${buildGradlePath}`);
let buildGradle = readFileSync(buildGradlePath, "utf8");
if (!buildGradle.includes("minSdk = 29")) {
  const hardened = buildGradle.replace(/minSdk\s*=\s*24/, "minSdk = 29");
  if (hardened === buildGradle) {
    fail("app/build.gradle.kts has no expected minSdk = 24 anchor");
  }
  buildGradle = hardened;
  writeFileSync(buildGradlePath, buildGradle);
  console.log("patch-android-mdns: raised minimum Android version to API 29 (Android 10)");
}

// --- MainActivity.kt: acquire the lock at app startup, release on destroy ---
if (!existsSync(activityPath)) fail(`no MainActivity.kt at ${activityPath}`);
let activity = readFileSync(activityPath, "utf8");
if (activity.includes("MulticastLock")) {
  console.log("patch-android-mdns: MainActivity.kt already acquires the multicast lock");
} else {
  const patched = `package dev.envryn.vault

import android.content.Context
import android.net.wifi.WifiManager
import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private var multicastLock: WifiManager.MulticastLock? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // See .dev-tools/patch-android-mdns.mjs's module doc for why this exists:
    // without it, mDNS-based sync discovery silently finds nothing on Android.
    val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
    multicastLock = wifiManager?.createMulticastLock("dev.envryn.vault.mdns")?.apply {
      setReferenceCounted(true)
      acquire()
    }
  }

  override fun onDestroy() {
    multicastLock?.let { if (it.isHeld) it.release() }
    super.onDestroy()
  }
}
`;
  writeFileSync(activityPath, patched);
  console.log("patch-android-mdns: MainActivity.kt now acquires a WifiManager.MulticastLock");
}

// FLAG_SECURE blocks screenshots, screen recording, and recent-app previews
// while Envryn's activity is visible. Apply it independently from the mDNS
// patch so upgrades from an already-patched generated project gain it too.
activity = readFileSync(activityPath, "utf8");
if (!activity.includes("WindowManager.LayoutParams.FLAG_SECURE")) {
  let hardened = activity;
  if (!hardened.includes("import android.view.WindowManager")) {
    hardened = hardened.replace(
      "import android.os.Bundle",
      "import android.os.Bundle\nimport android.view.WindowManager",
    );
  }
  hardened = hardened.replace(
    "super.onCreate(savedInstanceState)",
    [
      "super.onCreate(savedInstanceState)",
      "    window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)",
    ].join("\n"),
  );
  if (hardened === activity) {
    fail("MainActivity.kt has no expected onCreate anchor for FLAG_SECURE");
  }
  writeFileSync(activityPath, hardened);
  console.log("patch-android-mdns: enabled screenshot and recent-app preview protection");
}
