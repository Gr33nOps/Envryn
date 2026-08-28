#!/usr/bin/env node

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

function fail(message) {
  console.error(`mobsf-apk-scan: ${message}`);
  process.exit(1);
}

const apk = resolve(process.argv[2] ?? "");
const apiKey = process.env.MOBSF_API_KEY;
const baseUrl = (process.env.MOBSF_URL ?? "http://127.0.0.1:8000").replace(/\/$/, "");

if (!existsSync(apk)) fail(`APK not found: ${apk}`);
if (!apiKey) fail("MOBSF_API_KEY is required");

async function request(path, body) {
  let lastError;
  for (let attempt = 0; attempt < 15; attempt += 1) {
    try {
      const response = await fetch(`${baseUrl}${path}`, {
        method: "POST",
        headers: { "X-Mobsf-Api-Key": apiKey },
        body,
      });
      const text = await response.text();
      return { response, text };
    } catch (error) {
      lastError = error;
      await new Promise((resolvePromise) => setTimeout(resolvePromise, 2_000));
    }
  }
  throw lastError;
}

async function post(path, body) {
  const { response, text } = await request(path, body);
  if (!response.ok) fail(`${path} returned HTTP ${response.status}: ${text.slice(0, 500)}`);
  return JSON.parse(text);
}

const upload = new FormData();
upload.append("file", new Blob([readFileSync(apk)]), basename(apk));
const uploaded = await post("/api/v1/upload", upload);

const scan = new URLSearchParams({
  hash: uploaded.hash,
  scan_type: uploaded.scan_type,
  file_name: uploaded.file_name,
  re_scan: "1",
});
// The scan endpoint is synchronous and returns the completed JSON report.
// Keeping this response is also more robust than immediately asking the
// separate report endpoint while MobSF is still committing a first scan.
const report = await post("/api/v1/scan", scan);
const outputDir = resolve("target", "security");
mkdirSync(outputDir, { recursive: true });
const output = resolve(outputDir, `mobsf-${uploaded.hash}.json`);
writeFileSync(output, `${JSON.stringify(report, null, 2)}\n`);

const certificate = report.certificate_analysis?.certificate_info ?? "available in report";
console.log(`MobSF report saved: ${output}`);
console.log(`Package: ${report.package_name ?? "unknown"}`);
console.log(`Security score: ${report.security_score ?? "not reported"}`);
console.log(
  `Certificate analysis: ${typeof certificate === "string" ? certificate : "available in report"}`,
);
