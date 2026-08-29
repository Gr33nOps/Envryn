#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const ignoredDirectories = new Set([".git", "node_modules", "target"]);

function collectMarkdown(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (!ignoredDirectories.has(entry.name)) {
        files.push(...collectMarkdown(path.join(directory, entry.name)));
      }
    } else if (entry.isFile() && entry.name.toLowerCase().endsWith(".md")) {
      files.push(path.relative(root, path.join(directory, entry.name)).replaceAll("\\", "/"));
    }
  }
  return files;
}

const markdownFiles = collectMarkdown(root).sort();

const failures = [];

function checkTarget(file, rawTarget) {
  let target = rawTarget.trim().replace(/^<|>$/g, "");
  target = target.split(/\s+["']/)[0];
  if (!target || target.startsWith("#") || /^[a-z][a-z0-9+.-]*:/i.test(target)) return;

  target = target.split("#")[0].split("?")[0];
  if (!target) return;

  let decoded;
  try {
    decoded = decodeURIComponent(target);
  } catch {
    failures.push(`${file}: invalid URL encoding in ${rawTarget}`);
    return;
  }

  const resolved = path.resolve(root, path.dirname(file), decoded);
  if (!existsSync(resolved)) failures.push(`${file}: missing local target ${rawTarget}`);
}

for (const file of markdownFiles) {
  const source = readFileSync(path.join(root, file), "utf8");
  if (source.includes(String.fromCodePoint(0x2014))) {
    failures.push(`${file}: contains an em dash`);
  }

  for (const match of source.matchAll(/!?\[[^\]]*\]\(([^)]+)\)/g)) {
    checkTarget(file, match[1]);
  }
  for (const match of source.matchAll(/<(?:img|a)\b[^>]*(?:src|href)=["']([^"']+)["']/gi)) {
    checkTarget(file, match[1]);
  }
}

if (failures.length > 0) {
  console.error("Documentation verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Documentation verified: ${markdownFiles.length} files, no em dashes or broken local links.`,
);
