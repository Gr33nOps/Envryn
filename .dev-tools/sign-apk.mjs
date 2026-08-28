#!/usr/bin/env node
/**
 * Sign the Android release APK.
 *
 * **Why this exists as a committed script rather than a Gradle signingConfig:**
 * `src-tauri/gen/android/` is generated and `.gitignore`d, so anything written
 * into its Gradle files is lost the moment the project is regenerated. Signing
 * has to survive that, because an unsigned APK is not merely "untrusted" the
 * way an unsigned Windows installer is -- Android's package installer refuses
 * it outright ("package appears to be invalid" / "There was a problem parsing
 * the package"). v0.1.4-beta shipped an unsigned APK and was therefore
 * impossible to install at all; this script is the fix for that class of
 * mistake, not just that instance.
 *
 * Android signing costs nothing (unlike Windows Authenticode): the key is
 * self-generated, and Android only requires that a package carry *a* valid,
 * consistent signature -- not one chained to a public CA.
 *
 * **The keystore is the one irreplaceable artifact in this repo's release
 * process.** Android permanently binds an installed app's identity to its
 * signing key: an update signed with a different key cannot install over an
 * existing installation, and there is no recovery path. It therefore lives
 * outside the repository entirely (`~/.envryn/`), so it cannot be committed by
 * accident, and must be backed up separately.
 *
 * Usage:
 *   node .dev-tools/sign-apk.mjs [--in <unsigned-apk>] [--out <path>]
 *
 * Environment overrides (all optional):
 *   ENVRYN_KEYSTORE       path to the .jks            (default ~/.envryn/envryn-release.jks)
 *   ENVRYN_KEYSTORE_PASS  the store/key password      (default: read from ~/.envryn/keystore-password.txt)
 *   ENVRYN_KEY_ALIAS      key alias                   (default "envryn")
 *   ANDROID_HOME          Android SDK root            (required, for build-tools)
 */

import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, writeFileSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

function fail(message) {
  console.error(`sign-apk: ${message}`);
  process.exit(1);
}

const repoRoot = resolve(import.meta.dirname, "..");
const androidHome = process.env.ANDROID_HOME ?? process.env.ANDROID_SDK_ROOT;
if (!androidHome) fail("set ANDROID_HOME to your Android SDK directory");

/** Newest build-tools version present -- apksigner/zipalign live there. */
function buildToolsDir() {
  const root = join(androidHome, "build-tools");
  if (!existsSync(root)) fail(`no build-tools under ${root}`);
  const versions = readdirSync(root).sort();
  const newest = versions.at(-1);
  if (!newest) fail(`no build-tools versions installed under ${root}`);
  return join(root, newest);
}

const tools = buildToolsDir();
const zipalign = join(tools, process.platform === "win32" ? "zipalign.exe" : "zipalign");

// **Run apksigner's JAR directly rather than its `apksigner.bat` wrapper.**
// Node refuses to `execFileSync` a `.bat` without `shell: true` (blocked since
// Node 20 over the CVE-2024-27980 argument-injection class), and turning the
// shell on here would mean the keystore password passes through a command
// interpreter -- exactly what the `file:` password form below exists to avoid.
// The JAR is what the wrapper invokes anyway, so this is the same tool with
// one less layer.
const apksignerJar = join(tools, "lib", "apksigner.jar");
if (!existsSync(apksignerJar)) fail(`apksigner.jar not found at ${apksignerJar}`);
const javaHome = process.env.JAVA_HOME;
const java = javaHome
  ? join(javaHome, "bin", process.platform === "win32" ? "java.exe" : "java")
  : "java";

const keystore = process.env.ENVRYN_KEYSTORE ?? join(homedir(), ".envryn", "envryn-release.jks");
if (!existsSync(keystore)) {
  fail(
    `keystore not found at ${keystore}\n` +
      `  Create one with:\n` +
      `    keytool -genkeypair -keystore "${keystore}" -alias envryn \\\n` +
      `      -keyalg RSA -keysize 4096 -validity 10000\n` +
      `  Then BACK IT UP. Losing it means no future release can update an\n` +
      `  existing Android install -- Android has no key-rotation recovery.`,
  );
}

let password = process.env.ENVRYN_KEYSTORE_PASS;
if (!password) {
  const passwordFile = join(homedir(), ".envryn", "keystore-password.txt");
  if (!existsSync(passwordFile)) {
    fail(`set ENVRYN_KEYSTORE_PASS, or put the password in ${passwordFile}`);
  }
  password = readFileSync(passwordFile, "utf8").trim();
}
const alias = process.env.ENVRYN_KEY_ALIAS ?? "envryn";

const inputIndex = process.argv.indexOf("--in");
const unsigned =
  inputIndex !== -1 && process.argv[inputIndex + 1]
    ? resolve(process.argv[inputIndex + 1])
    : join(
        repoRoot,
        "src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk",
      );
if (!existsSync(unsigned)) {
  fail(`no unsigned APK at ${unsigned}\n  Run: cargo tauri android build --apk --ci`);
}

const version = JSON.parse(
  readFileSync(join(repoRoot, "src-tauri/tauri.conf.json"), "utf8"),
).version;

const outIndex = process.argv.indexOf("--out");
const out =
  outIndex !== -1 && process.argv[outIndex + 1]
    ? resolve(process.argv[outIndex + 1])
    : join(repoRoot, "target/release", `Envryn_${version}_android-universal.apk`);

const scratch = mkdtempSync(join(tmpdir(), "envryn-apk-"));
const aligned = join(scratch, "aligned.apk");

// zipalign BEFORE signing, not after: apksigner's v2/v3 signatures cover the
// whole file, so realigning afterwards would invalidate them. `-p` aligns
// uncompressed .so files to the page boundary, which is what lets Android map
// the native libraries directly out of the APK instead of extracting them.
console.log("zipalign...");
execFileSync(zipalign, ["-p", "-f", "4", unsigned, aligned], { stdio: "inherit" });

// Hand apksigner the password through a file rather than `pass:<literal>`.
// A password in argv is readable by any other process on the machine for as
// long as the signer runs (`Get-CimInstance Win32_Process` shows full command
// lines), and it lands in build logs -- which is precisely how it leaked into
// this repo's own CI scratch output the first time this script ran.
// Two separate files, not one shared file: apksigner reads each `file:`
// password as the *next line* of the stream it is given, so pointing both
// --ks-pass and --key-pass at one single-line file makes the second read hit
// EOF ("end of file reached"). Separate files make the intent explicit rather
// than depending on line ordering within one.
const storePassFile = join(scratch, "store.pass");
const keyPassFile = join(scratch, "key.pass");
writeFileSync(storePassFile, `${password}\n`, { mode: 0o600 });
writeFileSync(keyPassFile, `${password}\n`, { mode: 0o600 });

console.log("apksigner sign...");
try {
  execFileSync(
    java,
    [
      "-jar",
      apksignerJar,
      "sign",
      "--ks",
      keystore,
      "--ks-key-alias",
      alias,
      "--ks-pass",
      `file:${storePassFile}`,
      "--key-pass",
      `file:${keyPassFile}`,
      // minSdk is 29, so v1 (JAR) signing is not required. Request both v2
      // and v3; current apksigner may omit the older v2 block when every
      // supported OS can verify the stronger v3 scheme.
      "--v2-signing-enabled",
      "true",
      "--v3-signing-enabled",
      "true",
      "--out",
      out,
      aligned,
    ],
    { stdio: "inherit" },
  );
} finally {
  rmSync(scratch, { recursive: true, force: true });
}

console.log("apksigner verify...");
const verified = execFileSync(java, ["-jar", apksignerJar, "verify", "--verbose", out], {
  encoding: "utf8",
});
console.log(verified);

// An unsigned APK is uninstallable on Android, so "the signer ran" is not
// good enough -- the artifact itself has to prove it verifies before this
// script will call it releasable.
if (!verified.includes("Verifies")) {
  fail("the signed APK did not verify -- refusing to treat this as a releasable artifact");
}
if (!/v3 scheme \(APK Signature Scheme v3\): true/.test(verified)) {
  fail("v3 signing is missing -- refusing to publish a weaker Android signature");
}

console.log(`\nSigned APK ready: ${out}`);
