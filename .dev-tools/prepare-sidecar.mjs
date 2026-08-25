#!/usr/bin/env node
// Builds `envryn-ai-worker` in release mode and copies it into
// `src-tauri/binaries/` under the name Tauri's bundler expects for an
// `externalBin` sidecar (`<name>-<host-triple>[.exe]`, see
// `src-tauri/tauri.conf.json`'s `bundle.externalBin`).
//
// Run automatically by `cargo tauri build` via `beforeBuildCommand` --
// `tauri-build`'s own `copy_binaries` step (invoked from `build.rs`) only
// copies a sidecar that already exists under this exact name; it does not
// build one. Without this step, `src-tauri/binaries/` would need a
// manually-built binary checked into version control, which is exactly the
// kind of stale-artifact risk `.gitignore` deliberately avoids here -- see
// `ai::worker_binary_path`'s doc comment for how the app resolves the
// sidecar at runtime once it *is* in place.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

const hostLine = execFileSync("rustc", ["-vV"], { encoding: "utf8" })
  .split("\n")
  .find((line) => line.startsWith("host:"));
const hostTriple = hostLine?.split(":")[1]?.trim();
if (!hostTriple) {
  throw new Error("could not determine the host target triple from `rustc -vV`");
}

execFileSync("cargo", ["build", "-p", "envryn-ai-worker", "--release"], {
  stdio: "inherit",
  cwd: repoRoot,
});

const exeSuffix = process.platform === "win32" ? ".exe" : "";
const source = join(repoRoot, "target", "release", `envryn-ai-worker${exeSuffix}`);
const destDir = join(repoRoot, "src-tauri", "binaries");
mkdirSync(destDir, { recursive: true });
const dest = join(destDir, `envryn-ai-worker-${hostTriple}${exeSuffix}`);
copyFileSync(source, dest);
console.log(`Sidecar ready: ${dest}`);
