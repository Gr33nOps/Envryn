import { expect, test, type Page } from "@playwright/test";
import path from "node:path";

const now = Date.now();

const secrets = [
  {
    id: "demo-stripe",
    name: "STRIPE_SECRET_KEY",
    kind: "ApiKey",
    project: "Acme Platform",
    environment: "Production",
    provider: "Stripe",
    tags: ["billing"],
    has_notes: false,
    created_ms: now - 20 * 86_400_000,
    updated_ms: now - 8 * 60_000,
    rotated_ms: now - 14 * 86_400_000,
  },
  {
    id: "demo-database",
    name: "DATABASE_URL",
    kind: "Database",
    project: "Acme Platform",
    environment: "Production",
    provider: "PostgreSQL",
    tags: ["primary"],
    has_notes: true,
    created_ms: now - 30 * 86_400_000,
    updated_ms: now - 2 * 3_600_000,
    rotated_ms: null,
  },
  {
    id: "demo-github",
    name: "GITHUB_TOKEN",
    kind: "Token",
    project: "Developer Tools",
    environment: "Development",
    provider: "GitHub",
    tags: ["automation"],
    has_notes: false,
    created_ms: now - 12 * 86_400_000,
    updated_ms: now - 86_400_000,
    rotated_ms: null,
  },
  {
    id: "demo-openai",
    name: "OPENAI_API_KEY",
    kind: "ApiKey",
    project: "Developer Tools",
    environment: "Staging",
    provider: "OpenAI",
    tags: ["local-dev"],
    has_notes: false,
    created_ms: now - 8 * 86_400_000,
    updated_ms: now - 2 * 86_400_000,
    rotated_ms: null,
  },
  {
    id: "demo-redis",
    name: "REDIS_URL",
    kind: "EnvVar",
    project: "Acme Platform",
    environment: "Staging",
    provider: "Redis",
    tags: ["cache"],
    has_notes: false,
    created_ms: now - 6 * 86_400_000,
    updated_ms: now - 3 * 86_400_000,
    rotated_ms: null,
  },
];

async function installDemoRuntime(page: Page) {
  await page.addInitScript(
    ({ demoSecrets, timestamp }) => {
      let nextCallback = 1;
      const callbacks = new Map<number, (...args: unknown[]) => unknown>();

      const invoke = async (command: string) => {
        switch (command) {
          case "vault_status":
            return {
              exists: true,
              unlocked: true,
              platform_protection_available: true,
              platform_protection_enabled: true,
              hello_gate_available: true,
              hello_gate_enabled: false,
            };
          case "secret_list":
            return demoSecrets;
          case "trusted_device_list":
            return [
              {
                device_id: "pixel-7-demo",
                fingerprint_hex: "a1".repeat(32),
                name: "Pixel 7",
                paired_ms: timestamp - 12 * 86_400_000,
                last_sync_ms: timestamp - 4 * 60_000,
              },
            ];
          case "discovery_browse":
            return [
              {
                device_id: "pixel-7-demo",
                fingerprint_hex: "a1".repeat(32),
                addresses: ["192.168.1.24"],
                port: 43123,
              },
            ];
          case "conflict_list_all":
            return [];
          case "conflict_count":
            return 0;
          case "sync_listen_start":
            return 43123;
          case "sync_listen_stop":
          case "vault_lock":
            return null;
          case "settings_get":
            return { auto_lock_minutes: 5, clipboard_clear_seconds: 30, ai_enabled: false };
          case "device_identity":
            return { device_id: "windows-demo", fingerprint: "b2".repeat(32) };
          case "ai_status":
            return {
              enabled_in_settings: false,
              model_downloaded: false,
              model_name: "Local model",
              engine_running: false,
            };
          case "plugin:window|is_maximized":
            return false;
          case "plugin:event|listen":
            return 1;
          default:
            if (command.startsWith("plugin:")) return null;
            throw new Error(`Unhandled screenshot command: ${command}`);
        }
      };

      Object.defineProperty(window, "__TAURI_INTERNALS__", {
        configurable: true,
        value: {
          invoke,
          metadata: {
            currentWindow: { label: "main" },
            currentWebview: { label: "main" },
          },
          plugins: { event: { unregisterListener() {} } },
          transformCallback(callback: (...args: unknown[]) => unknown, once = false) {
            const id = nextCallback++;
            callbacks.set(id, (...args: unknown[]) => {
              const result = callback(...args);
              if (once) callbacks.delete(id);
              return result;
            });
            return id;
          },
          unregisterCallback(id: number) {
            callbacks.delete(id);
          },
          runCallback(id: number, ...args: unknown[]) {
            return callbacks.get(id)?.(...args);
          },
        },
      });

      Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
        configurable: true,
        value: { unregisterListener() {} },
      });
    },
    { demoSecrets: secrets, timestamp: now },
  );
}

test("capture README product screenshots", async ({ page }, testInfo) => {
  const runtimeErrors: string[] = [];
  page.on("pageerror", (error) => runtimeErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") runtimeErrors.push(message.text());
  });

  await installDemoRuntime(page);
  await page.goto("/vault");
  await expect(page.getByRole("heading", { name: "Secrets" })).toBeVisible();
  await expect(page.getByText("STRIPE_SECRET_KEY")).toBeVisible();

  const output = path.resolve("docs", "assets", "screenshots");
  if (testInfo.project.name === "desktop") {
    await page.screenshot({ path: path.join(output, "desktop-vault.png"), fullPage: true });
  } else {
    await page.screenshot({ path: path.join(output, "mobile-vault.png"), fullPage: true });
    await page.getByRole("button", { name: "More" }).click();
    await page.getByRole("link", { name: "Sync", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Sync", exact: true })).toBeVisible();
    await expect(page.getByText("Pixel 7")).toBeVisible();
    await page.screenshot({ path: path.join(output, "mobile-sync.png"), fullPage: true });
  }

  expect(runtimeErrors).toEqual([]);
});
