import { readdir, stat } from "node:fs/promises";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const dist = join(root, "apps", "ui", "dist");

const limits = {
  largestJavaScript: 400 * 1024,
  totalJavaScript: 750 * 1024,
  totalCss: 125 * 1024,
};

async function filesBelow(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map((entry) => {
      const path = join(directory, entry.name);
      return entry.isDirectory() ? filesBelow(path) : [path];
    }),
  );
  return nested.flat();
}

function kibibytes(bytes) {
  return `${(bytes / 1024).toFixed(1)} KiB`;
}

const files = await filesBelow(dist);
const measured = await Promise.all(
  files.map(async (path) => ({ path, bytes: (await stat(path)).size })),
);
const javascript = measured.filter(({ path }) => path.endsWith(".js"));
const css = measured.filter(({ path }) => path.endsWith(".css"));
const largestJavaScript = javascript.reduce(
  (largest, file) => (file.bytes > largest.bytes ? file : largest),
  { path: "", bytes: 0 },
);
const totalJavaScript = javascript.reduce((total, file) => total + file.bytes, 0);
const totalCss = css.reduce((total, file) => total + file.bytes, 0);

const checks = [
  {
    name: `largest JavaScript chunk (${relative(dist, largestJavaScript.path)})`,
    actual: largestJavaScript.bytes,
    limit: limits.largestJavaScript,
  },
  { name: "total JavaScript", actual: totalJavaScript, limit: limits.totalJavaScript },
  { name: "total CSS", actual: totalCss, limit: limits.totalCss },
];

let failed = false;
for (const check of checks) {
  const passed = check.actual <= check.limit;
  console.log(
    `${passed ? "PASS" : "FAIL"} ${check.name}: ${kibibytes(check.actual)} / ${kibibytes(check.limit)}`,
  );
  failed ||= !passed;
}

if (failed) {
  console.error("Bundle budget exceeded. Split or remove code before raising a limit.");
  process.exitCode = 1;
}
