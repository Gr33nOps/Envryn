import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

const runtimeErrors = new WeakMap<Page, string[]>();

/**
 * Browser E2E deliberately mocks only Tauri's transport boundary. React,
 * routing, validation, responsive CSS, and every user interaction below are
 * production code. Native IPC/crypto remain covered by Rust integration tests
 * and the real-window WebDriver smoke suite.
 */
async function installTauriMock(page: Page) {
  await page.addInitScript(() => {
    let exists = false;
    let unlocked = false;
    let nextCallback = 1;
    const callbacks = new Map<number, (...args: unknown[]) => unknown>();

    const invoke = async (command: string, args?: Record<string, unknown>) => {
      switch (command) {
        case "vault_status":
          return {
            exists,
            unlocked,
            platform_protection_available: true,
            platform_protection_enabled: false,
            hello_gate_available: false,
            hello_gate_enabled: false,
          };
        case "vault_create":
          exists = true;
          unlocked = true;
          return null;
        case "vault_lock":
          unlocked = false;
          return null;
        case "secret_list":
        case "trusted_device_list":
        case "discovery_browse":
        case "conflict_list_all":
          return [];
        case "conflict_count":
          return 0;
        case "sync_listen_start":
          return 43123;
        case "sync_listen_stop":
          return null;
        case "settings_get":
          return { auto_lock_minutes: 5, clipboard_clear_seconds: 30, ai_enabled: false };
        case "device_identity":
          return { device_id: "browser-e2e", fingerprint: "00".repeat(32) };
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
          throw { code: "internal", message: `Unhandled E2E command: ${command}`, args };
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
        plugins: {
          event: {
            unregisterListener() {},
          },
        },
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
      value: {
        unregisterListener() {},
      },
    });
  });
}

test.beforeEach(async ({ page }) => {
  const errors: string[] = [];
  runtimeErrors.set(page, errors);
  page.on("pageerror", (error) => errors.push(`page: ${error.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(`console: ${message.text()}`);
  });
  await installTauriMock(page);
});

test.afterEach(async ({ page }) => {
  expect(runtimeErrors.get(page) ?? []).toEqual([]);
});

async function navigateFromVault(page: Page, mobile: boolean, destination: "Sync" | "Settings") {
  if (mobile) {
    await page.getByRole("button", { name: "More" }).click();
  }
  await page.getByRole("link", { name: destination, exact: true }).click();
  await expect(page.getByRole("heading", { name: destination, exact: true })).toBeVisible();
}

test("creates a vault through the real responsive UI", async ({ page }, testInfo) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Set up Envryn" })).toBeVisible();

  await page.getByLabel("Master password", { exact: true }).fill("first-password");
  await expect(page.getByRole("img", { name: /Estimated password strength/ })).toBeVisible();
  await page.getByLabel("Confirm master password").fill("different-password");
  await page.getByRole("button", { name: "Create vault" }).click();
  await expect(page.getByText("Those passwords do not match.")).toBeVisible();

  const password = "E2E correct horse battery staple 42!";
  await page.getByLabel("Master password", { exact: true }).fill(password);
  await page.getByLabel("Confirm master password").fill(password);
  await page.getByRole("button", { name: "Create vault" }).click();

  await expect(page).toHaveURL(/\/vault\/?$/);
  await expect(page.getByRole("heading", { name: "Secrets" })).toBeVisible();
  await expect(page.getByText("No secrets yet")).toBeVisible();

  const hasHorizontalOverflow = await page.evaluate(
    () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
  );
  expect(hasHorizontalOverflow).toBe(false);

  const mobile = testInfo.project.name === "android-chromium";
  if (mobile) {
    await expect(page.getByRole("navigation", { name: "Primary navigation" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Add", exact: true })).toBeVisible();
  } else {
    await expect(page.getByText("My vault")).toBeVisible();
  }

  await navigateFromVault(page, mobile, "Sync");
  await navigateFromVault(page, mobile, "Settings");
});

test("onboarding and join screens have no serious accessibility violations", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Set up Envryn" })).toBeVisible();

  const onboarding = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze();
  expect(
    onboarding.violations.filter((item) => ["serious", "critical"].includes(item.impact ?? "")),
  ).toEqual([]);

  await page.getByRole("button", { name: /Join an existing vault instead/ }).click();
  await expect(page.getByRole("heading", { name: "Join an existing vault" })).toBeVisible();

  const join = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze();
  expect(
    join.violations.filter((item) => ["serious", "critical"].includes(item.impact ?? "")),
  ).toEqual([]);
});
