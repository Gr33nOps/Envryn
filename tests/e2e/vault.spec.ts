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
    let masterPassword = "";
    const projects: { id: string; name: string; created_ms: number }[] = [];
    type MockPayload = { kind: string; [key: string]: unknown };
    type MockSecret = {
      id: string;
      name: string;
      project: string;
      environment: string;
      payload: MockPayload;
      notes: string | null;
      tags: string[];
      provider: string | null;
      created_ms: number;
      updated_ms: number;
      rotated_ms: number | null;
    };
    const secrets: MockSecret[] = [];
    let settings = {
      auto_lock_minutes: 5,
      clipboard_clear_seconds: 30,
      theme: "system",
      ai_enabled: false,
    };
    let platformProtectionEnabled = false;
    let aiDownloaded = false;
    let aiRunning = false;
    const trustedDevices = [
      {
        device_id: "qa-laptop",
        fingerprint_hex: "ab".repeat(32),
        name: "QA Laptop",
        paired_ms: Date.now() - 86_400_000,
        last_sync_ms: Date.now() - 60_000,
      },
    ];
    const discoveredPeers = [
      {
        device_id: "qa-laptop",
        fingerprint_hex: "ab".repeat(32),
        addresses: ["192.0.2.25"],
        port: 43123,
      },
    ];
    const conflictRecord: MockSecret = {
      id: "conflicted-secret",
      name: "Conflicted API Key",
      project: "QA",
      environment: "Staging",
      payload: { kind: "ApiKey", value: "FAKE_CONFLICT_VALUE" },
      notes: null,
      tags: ["conflict"],
      provider: "Example",
      created_ms: Date.now() - 120_000,
      updated_ms: Date.now() - 60_000,
      rotated_ms: null,
    };
    const conflicts = [
      { conflict_id: "conflict-1", record: conflictRecord },
      {
        conflict_id: "conflict-2",
        record: { ...conflictRecord, id: "conflicted-secret-2", name: "Conflicted Token" },
      },
    ];
    const seedSecrets = (count: number) => {
      const projectNames = Array.from(
        { length: 50 },
        (_, index) => `Project ${String(index).padStart(2, "0")}`,
      );
      for (const [index, name] of projectNames.entries()) {
        if (!projects.some((project) => project.name === name)) {
          projects.push({
            id: `10000000-0000-4000-8000-${String(index + 1).padStart(12, "0")}`,
            name,
            created_ms: Date.now() - count + index,
          });
        }
      }
      for (let index = 0; index < count; index += 1) {
        secrets.push({
          id: `performance-secret-${index}`,
          name: `PERF_SECRET_${String(index).padStart(4, "0")}`,
          project: projectNames[index % projectNames.length]!,
          environment: ["Development", "Staging", "Production"][index % 3]!,
          payload: { kind: "ApiKey", value: `FAKE_PERFORMANCE_VALUE_${index}` },
          notes: `Unicode QA note ${index}: café 東京`,
          tags: ["qa", `batch-${index % 10}`],
          provider: "Performance QA",
          created_ms: Date.now() - count + index,
          updated_ms: Date.now() - count + index,
          rotated_ms: null,
        });
      }
    };
    const summary = (secret: MockSecret) => ({
      id: secret.id,
      name: secret.name,
      kind: secret.payload.kind,
      project: secret.project,
      environment: secret.environment,
      provider: secret.provider,
      tags: secret.tags,
      has_notes: Boolean(secret.notes),
      created_ms: secret.created_ms,
      updated_ms: secret.updated_ms,
      rotated_ms: secret.rotated_ms,
    });
    let nextCallback = 1;
    const callbacks = new Map<number, (...args: unknown[]) => unknown>();

    const invoke = async (command: string, args?: Record<string, unknown>) => {
      switch (command) {
        case "vault_status":
          return {
            exists,
            unlocked,
            platform_protection_available: true,
            platform_protection_enabled: platformProtectionEnabled,
            hello_gate_available: false,
            hello_gate_enabled: false,
          };
        case "vault_create":
          exists = true;
          unlocked = true;
          masterPassword = String(args?.["password"] ?? "");
          return null;
        case "vault_unlock":
          if (args?.["password"] !== masterPassword) {
            throw { code: "auth_failed", message: "Authentication failed" };
          }
          unlocked = true;
          return null;
        case "vault_unlock_with_platform":
          if (!platformProtectionEnabled) {
            throw {
              code: "platform_unavailable",
              message: "Windows account unlock is unavailable",
            };
          }
          unlocked = true;
          return null;
        case "vault_lock":
          unlocked = false;
          return null;
        case "trusted_device_list":
          return trustedDevices;
        case "trusted_device_rename": {
          const device = trustedDevices.find((item) => item.device_id === args?.["deviceId"]);
          if (!device) throw { code: "not_found", message: "Device not found" };
          device.name = String(args?.["name"] ?? "");
          return device;
        }
        case "trusted_device_revoke": {
          const index = trustedDevices.findIndex((item) => item.device_id === args?.["deviceId"]);
          if (index < 0) throw { code: "not_found", message: "Device not found" };
          trustedDevices.splice(index, 1);
          return null;
        }
        case "discovery_browse":
          return discoveredPeers;
        case "sync_now":
          if (String(args?.["address"] ?? "").endsWith(".99")) {
            throw { code: "network", message: "The device could not be reached" };
          }
          return { records_applied: 2, conflicts: conflicts.length };
        case "pairing_host_start":
          return {
            address: "192.0.2.10",
            port: 43123,
            code: "482731",
            device_id: "browser-e2e",
            fingerprint_display: "00:00:00:00",
          };
        case "pairing_cancel":
          return null;
        case "conflict_list_all":
          return conflicts;
        case "conflict_recover": {
          const index = conflicts.findIndex((item) => item.conflict_id === args?.["conflictId"]);
          if (index < 0) throw { code: "not_found", message: "Conflict not found" };
          const recovered = { ...conflicts[index]!.record, id: `secret-${secrets.length + 1}` };
          secrets.push(recovered);
          conflicts.splice(index, 1);
          return summary(recovered);
        }
        case "conflict_discard": {
          const index = conflicts.findIndex((item) => item.conflict_id === args?.["conflictId"]);
          if (index < 0) throw { code: "not_found", message: "Conflict not found" };
          conflicts.splice(index, 1);
          return null;
        }
        case "secret_list":
          return secrets.map(summary);
        case "secret_search": {
          const query = String(args?.["query"] ?? "").toLowerCase();
          return secrets
            .filter((secret) =>
              [
                secret.name,
                secret.project,
                secret.environment,
                secret.provider ?? "",
                secret.tags.join(" "),
              ]
                .join(" ")
                .toLowerCase()
                .includes(query),
            )
            .map(summary);
        }
        case "secret_reveal": {
          const secret = secrets.find((item) => item.id === args?.["id"]);
          if (!secret) throw { code: "not_found", message: "Secret not found" };
          return secret;
        }
        case "secret_create": {
          const input = args?.["input"] as Omit<
            MockSecret,
            "id" | "created_ms" | "updated_ms" | "rotated_ms"
          >;
          const now = Date.now();
          const secret: MockSecret = {
            ...input,
            id: `secret-${secrets.length + 1}`,
            notes: input.notes ?? null,
            provider: input.provider ?? null,
            created_ms: now,
            updated_ms: now,
            rotated_ms: null,
          };
          secrets.push(secret);
          return summary(secret);
        }
        case "secret_update": {
          const secret = secrets.find((item) => item.id === args?.["id"]);
          if (!secret) throw { code: "not_found", message: "Secret not found" };
          Object.assign(secret, args?.["update"] as Partial<MockSecret>, {
            updated_ms: Date.now(),
          });
          return summary(secret);
        }
        case "secret_delete": {
          const index = secrets.findIndex((item) => item.id === args?.["id"]);
          if (index < 0) throw { code: "not_found", message: "Secret not found" };
          secrets.splice(index, 1);
          return null;
        }
        case "secret_duplicates":
          return [];
        case "clipboard_copy":
          return null;
        case "project_list":
          return projects;
        case "project_create": {
          const project = {
            id: `00000000-0000-4000-8000-${String(projects.length + 1).padStart(12, "0")}`,
            name: String(args?.["name"] ?? ""),
            created_ms: Date.now(),
          };
          projects.push(project);
          return project;
        }
        case "project_rename": {
          const project = projects.find((item) => item.id === args?.["id"]);
          if (!project) throw { code: "not_found", message: "Project not found" };
          const oldName = project.name;
          project.name = String(args?.["name"] ?? "");
          for (const secret of secrets) {
            if (secret.project === oldName) secret.project = project.name;
          }
          return project;
        }
        case "conflict_count":
          return conflicts.length;
        case "sync_listen_start":
          return 43123;
        case "sync_listen_stop":
          return null;
        case "settings_get":
          return settings;
        case "settings_set":
          settings = { ...settings, ...(args?.["settings"] as typeof settings) };
          return settings;
        case "vault_enable_platform_protection":
          if (args?.["password"] !== masterPassword) {
            throw { code: "auth_failed", message: "Authentication failed" };
          }
          platformProtectionEnabled = true;
          return null;
        case "vault_disable_platform_protection":
          platformProtectionEnabled = false;
          return null;
        case "vault_change_password":
          if (args?.["currentPassword"] !== masterPassword) {
            throw { code: "auth_failed", message: "Authentication failed" };
          }
          masterPassword = String(args?.["newPassword"] ?? "");
          return null;
        case "backup_create":
          if (String(args?.["path"] ?? "").includes("cannot-write")) {
            throw { code: "io", message: "The backup location could not be written." };
          }
          return null;
        case "backup_restore":
          if (args?.["backupPassword"] === "wrong-password") {
            throw { code: "auth_failed", message: "Authentication failed" };
          }
          return { restored: 3 };
        case "device_identity":
          return { device_id: "browser-e2e", fingerprint: "00".repeat(32) };
        case "ai_status":
          return {
            enabled_in_settings: settings.ai_enabled,
            model_downloaded: aiDownloaded,
            model_name: "Local model",
            engine_running: aiRunning,
          };
        case "ai_download_model":
          aiDownloaded = true;
          return null;
        case "ai_start":
          aiRunning = true;
          return null;
        case "ai_stop":
          aiRunning = false;
          return null;
        case "classify_deterministic": {
          const name = String(args?.["name"] ?? "").toUpperCase();
          const value = String(args?.["value"] ?? "");
          if (name.includes("DATABASE") || value.startsWith("postgres://")) {
            return { kind: "Database", provider: "PostgreSQL", confidence: 0.99 };
          }
          if (name.includes("SSH") || value.includes("PRIVATE KEY")) {
            return { kind: "SshKey", provider: "OpenSSH", confidence: 0.99 };
          }
          if (name.includes("TOKEN")) {
            return { kind: "Token", provider: null, confidence: 0.9 };
          }
          if (name.includes("KEY")) {
            return { kind: "ApiKey", provider: null, confidence: 0.9 };
          }
          return null;
        }
        case "ai_classify_pasted_value":
          return { kind: "ApiKey", provider: "IGDB", confidence: 0.82 };
        case "ai_suggest_name":
          return { name: "IGDB API Key", confidence: 0.84 };
        case "ai_classify_env_names": {
          const names = (args?.["names"] as string[] | undefined) ?? [];
          return {
            names: names.map((name) => ({
              name,
              kind: name.includes("URL") ? "Webhook" : "Environment",
            })),
          };
        }
        case "ai_extract_structured_fields":
          return {
            fields: [
              { label: "Host", value: "db.example.test" },
              { label: "Port", value: "5432" },
              { label: "Username", value: "qa_user" },
            ],
          };
        case "ai_parse_search_intent":
          return {
            text: String(args?.["query"] ?? ""),
            kind: null,
            project: null,
            environment: null,
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

    Object.defineProperty(window, "__ENVRYN_E2E_STATE__", {
      configurable: true,
      value: { trustedDevices, discoveredPeers, conflicts, seedSecrets },
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

async function createDisposableVault(page: Page, password = "E2E disposable vault password 42!") {
  await page.goto("/");
  await page.getByLabel("Master password", { exact: true }).fill(password);
  await page.getByLabel("Confirm master password").fill(password);
  await page.getByRole("button", { name: "Create vault" }).click();
  await expect(page).toHaveURL(/\/vault\/?$/);
  await expect(page.getByRole("heading", { name: "Secrets" })).toBeVisible();
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
    await expect(page.getByLabel("Filter by secret category")).toBeVisible();
    await expect(page.locator(".desktop-category-tabs")).toBeHidden();
    const add = page.getByRole("button", { name: "Add", exact: true });
    await expect(add).toBeVisible();
    await add.click();
    const addDialog = page.getByRole("dialog");
    await expect(addDialog.getByRole("heading", { name: "Add a secret" })).toBeVisible();
    const addBox = await addDialog.boundingBox();
    expect(addBox).not.toBeNull();
    expect(Math.abs(addBox!.x)).toBeLessThanOrEqual(1);
    expect(Math.abs(addBox!.width - page.viewportSize()!.width)).toBeLessThanOrEqual(1);
    expect(Math.abs(addBox!.y + addBox!.height - page.viewportSize()!.height)).toBeLessThanOrEqual(
      2,
    );
    await addDialog.getByRole("button", { name: "Close" }).click();
  } else {
    await expect(page.getByText("My vault")).toBeVisible();
  }

  await navigateFromVault(page, mobile, "Sync");
  await navigateFromVault(page, mobile, "Settings");
});

test("creates and opens a real project, with a mobile-sized dialog", async ({ page }, testInfo) => {
  await page.goto("/");
  const password = "E2E project password 42!";
  await page.getByLabel("Master password", { exact: true }).fill(password);
  await page.getByLabel("Confirm master password").fill(password);
  await page.getByRole("button", { name: "Create vault" }).click();

  const mobile = testInfo.project.name === "android-chromium";
  await page.getByRole("link", { name: "Projects", exact: true }).click();
  await page.getByRole("button", { name: "New project" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  if (mobile) {
    await expect(dialog).toHaveCSS("transform", "none");
    const box = await dialog.boundingBox();
    const viewport = page.viewportSize();
    expect(box).not.toBeNull();
    expect(viewport).not.toBeNull();
    expect(box!.width).toBeLessThanOrEqual(viewport!.width);
    expect(Math.abs(box!.y + box!.height - viewport!.height)).toBeLessThanOrEqual(2);
  }

  await page.getByPlaceholder("e.g. Rescripto").fill("Mobile API");
  await page.getByRole("button", { name: "Create project" }).click();
  await expect(page).toHaveURL(/\/vault\/projects\/00000000-0000-4000-8000-000000000001/);
  await expect(page.getByText("Mobile API", { exact: true })).toBeVisible();
  await expect(page.getByText("No secrets in —")).toBeVisible();
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

test("creates, reveals, edits, persists, searches, and deletes a structured desktop secret", async ({
  page,
}) => {
  await page.goto("/");
  const password = "E2E desktop secret password 42!";
  await page.getByLabel("Master password", { exact: true }).fill(password);
  await page.getByLabel("Confirm master password").fill(password);
  await page.getByRole("button", { name: "Create vault" }).click();

  await page.getByRole("button", { name: "Add secret", exact: true }).click();
  const add = page.getByRole("dialog");
  await add.getByPlaceholder("e.g. OPENAI_API_KEY").fill("QA PostgreSQL");
  await add.getByLabel("What kind is it?").selectOption("Database");
  await add.getByLabel("Project").fill("Website");
  await add.getByLabel("Environment").selectOption("Production");
  await add.getByPlaceholder("Paste secret value").fill("FAKE_DATABASE_PASSWORD_001");
  await add.getByLabel("Host").fill("db.example.test");
  await add.getByLabel("Port").fill("5433");
  await add.getByLabel("Database").fill("envryn_qa");
  await add.getByLabel("Username").fill("qa_user");
  await add.getByLabel("Notes").fill("Desktop E2E structured field coverage");
  await add.getByLabel("Tags").fill("qa, release-candidate");
  await add.getByRole("button", { name: "Save secret" }).click();

  const row = page.getByRole("button", { name: "Open QA PostgreSQL details" });
  await expect(row).toBeVisible();
  await row.click();
  const panel = page.locator(".secret-panel");
  await expect(panel).toContainText("Production");
  await expect(panel).toContainText("PostgreSQL");
  await expect(panel).toContainText("release-candidate");
  await panel.getByRole("button", { name: "Reveal" }).click();
  await expect(panel).toContainText("qa_user@db.example.test:5433/envryn_qa");

  await panel.getByRole("button", { name: "Edit" }).click();
  const edit = page.getByRole("dialog");
  await edit.getByLabel("Environment").selectOption("Staging");
  await edit.getByLabel("Host").fill("db-staging.example.test");
  await edit.getByLabel("Port").fill("6432");
  await edit.getByRole("button", { name: "Save changes" }).click();

  await expect(panel).toContainText("Staging");
  await panel.getByRole("button", { name: "Edit" }).click();
  const reopened = page.getByRole("dialog");
  await expect(reopened.getByLabel("Environment")).toHaveValue("Staging");
  await expect(reopened.getByLabel("Host")).toHaveValue("db-staging.example.test");
  await expect(reopened.getByLabel("Port")).toHaveValue("6432");
  await reopened.getByRole("button", { name: "Cancel" }).click();

  await page.keyboard.press("Control+k");
  await page.getByPlaceholder("Search your vault, then press Enter").fill("PostgreSQL");
  await expect(page.getByText("QA PostgreSQL", { exact: true }).first()).toBeVisible();
  await page.keyboard.press("Escape");

  const vaultAudit = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze();
  expect(
    vaultAudit.violations.filter((item) => ["serious", "critical"].includes(item.impact ?? "")),
  ).toEqual([]);

  await panel.getByRole("button", { name: "Delete secret" }).click();
  const confirmation = page.getByRole("dialog", { name: "Delete QA PostgreSQL?" });
  await confirmation.getByRole("button", { name: "Delete secret" }).click();
  await expect(row).toHaveCount(0);
  await expect(page.getByText("No secrets yet")).toBeVisible();
});

test("persists desktop settings and exercises Windows unlock, local AI, and password changes", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Settings", exact: true })).toBeVisible();
  await expect(page.getByText("Version 0.1.9")).toBeVisible();

  await page.getByLabel("Auto-lock the vault").selectOption("15");
  await page.getByLabel("Clear clipboard after copying").selectOption("60");
  await page.getByRole("link", { name: "Backup", exact: true }).click();
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await expect(page.getByLabel("Auto-lock the vault")).toHaveValue("15");
  await expect(page.getByLabel("Clear clipboard after copying")).toHaveValue("60");

  await page.getByRole("button", { name: "Download", exact: true }).click();
  await expect(page.getByText("Ready", { exact: true })).toBeVisible();
  const aiSwitch = page.getByRole("switch", { name: "Enable local AI" });
  await aiSwitch.click();
  await expect(aiSwitch).toBeChecked();
  await expect(page.getByText("Running", { exact: true })).toBeVisible();
  await aiSwitch.click();
  await expect(aiSwitch).not.toBeChecked();

  const windowsSwitch = page.getByRole("switch", { name: "Unlock with this Windows account" });
  await windowsSwitch.click();
  const enableDialog = page.getByRole("dialog", { name: "Confirm your master password" });
  await enableDialog.getByLabel("Master password").fill("wrong-password");
  await enableDialog.getByRole("button", { name: "Enable" }).click();
  await expect(enableDialog.getByText("That password did not work.")).toBeVisible();
  await enableDialog.getByLabel("Master password").fill("E2E disposable vault password 42!");
  await enableDialog.getByRole("button", { name: "Enable" }).click();
  await expect(windowsSwitch).toBeChecked();
  await windowsSwitch.click();
  await expect(windowsSwitch).not.toBeChecked();

  await page.getByRole("button", { name: "Change", exact: true }).click();
  const changeDialog = page.getByRole("dialog", { name: "Change master password" });
  await changeDialog.getByLabel("Current password").fill("E2E disposable vault password 42!");
  await changeDialog.getByLabel("New password", { exact: true }).fill("short");
  await changeDialog.getByLabel("Confirm new password").fill("short");
  await changeDialog.getByRole("button", { name: "Change password" }).click();
  await expect(
    changeDialog.getByText("Your new password must be at least 8 characters."),
  ).toBeVisible();
  await changeDialog.getByLabel("New password", { exact: true }).fill("new-password-42!");
  await changeDialog.getByLabel("Confirm new password").fill("different-password");
  await changeDialog.getByRole("button", { name: "Change password" }).click();
  await expect(changeDialog.getByText("Those passwords do not match.")).toBeVisible();
  await changeDialog.getByLabel("Confirm new password").fill("new-password-42!");
  await changeDialog.getByLabel("Current password").fill("wrong-current-password");
  await changeDialog.getByRole("button", { name: "Change password" }).click();
  await expect(changeDialog.getByText("Your current password did not match.")).toBeVisible();
  await changeDialog.getByLabel("Current password").fill("E2E disposable vault password 42!");
  await changeDialog.getByRole("button", { name: "Change password" }).click();
  await expect(changeDialog).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Reset" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Delete", exact: true })).toBeDisabled();

  const audit = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze();
  expect(
    audit.violations.filter((item) => ["serious", "critical"].includes(item.impact ?? "")),
  ).toEqual([]);
});

test("validates, creates, and restores encrypted backups through the desktop UI", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.getByRole("link", { name: "Backup", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Backup", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Back up now" }).first().click();
  const createDialog = page.getByRole("dialog", { name: "Create encrypted backup" });
  await createDialog.getByRole("button", { name: "Create backup" }).click();
  await expect(createDialog.getByText("Choose where to save the backup file.")).toBeVisible();
  await createDialog.getByLabel("Save to").fill("C:\\QA\\envryn-backup.envrynbk");
  await createDialog.getByLabel("Backup password").fill("short");
  await createDialog.getByLabel("Confirm password").fill("short");
  await createDialog.getByRole("button", { name: "Create backup" }).click();
  await expect(
    createDialog.getByText("Your backup password must be at least 8 characters."),
  ).toBeVisible();
  await createDialog.getByLabel("Backup password").fill("backup-password-42!");
  await createDialog.getByLabel("Confirm password").fill("different-password");
  await createDialog.getByRole("button", { name: "Create backup" }).click();
  await expect(createDialog.getByText("Passwords do not match.")).toBeVisible();
  await createDialog.getByLabel("Confirm password").fill("backup-password-42!");
  await createDialog.getByLabel("Save to").fill("C:\\cannot-write\\envryn-backup.envrynbk");
  await createDialog.getByRole("button", { name: "Create backup" }).click();
  await expect(createDialog.getByText("The backup location could not be written.")).toBeVisible();
  await createDialog.getByLabel("Save to").fill("C:\\QA\\envryn-backup.envrynbk");
  await createDialog.getByRole("button", { name: "Create backup" }).click();
  await expect(createDialog).toHaveCount(0);
  await expect(page.getByText("Backup created")).toBeVisible();

  await page.getByRole("button", { name: "Restore", exact: true }).click();
  const restoreDialog = page.getByRole("dialog", { name: "Restore from backup" });
  await restoreDialog.getByRole("button", { name: "Restore vault" }).click();
  await expect(restoreDialog.getByText("Choose the backup file to restore.")).toBeVisible();
  await restoreDialog.getByLabel("Backup file").fill("C:\\QA\\envryn-backup.envrynbk");
  await restoreDialog.getByLabel("Backup password").fill("wrong-password");
  await restoreDialog.getByLabel("New master password").fill("restored-password-42!");
  await restoreDialog.getByLabel("Confirm new password").fill("restored-password-42!");
  await restoreDialog.getByRole("button", { name: "Restore vault" }).click();
  await expect(restoreDialog.getByText("That backup password did not work.")).toBeVisible();
  await restoreDialog.getByLabel("Backup password").fill("backup-password-42!");
  await restoreDialog.getByRole("button", { name: "Restore vault" }).click();
  await expect(page).toHaveURL(/\/vault\/?$/);
  await expect(page.getByText("Restored 3 secrets")).toBeVisible();
});

test("renames, inspects, and revokes a trusted desktop device", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.getByRole("link", { name: "Trusted devices", exact: true }).click();
  await expect(page.getByText("1 approved device")).toBeVisible();
  await page.getByRole("button", { name: /QA Laptop/ }).click();
  await expect(page.locator(".device-detail-panel")).toContainText("AB:AB:AB:AB");
  await page.getByRole("button", { name: "Rename" }).click();
  await page.getByLabel("Device name").fill("Renamed QA Laptop");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("Renamed QA Laptop", { exact: true }).first()).toBeVisible();

  await page.getByRole("button", { name: "Revoke device" }).click();
  const confirm = page.getByRole("dialog", { name: "Revoke Renamed QA Laptop?" });
  await confirm.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByText("1 approved device")).toBeVisible();
  await page.getByRole("button", { name: "Revoke device" }).click();
  await page
    .getByRole("dialog", { name: "Revoke Renamed QA Laptop?" })
    .getByRole("button", { name: "Revoke device" })
    .click();
  await expect(page.getByText("No devices paired yet. Pair one to start syncing.")).toBeVisible();
});

test("opens and cancels the desktop manual pairing session with local address details", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.getByRole("link", { name: "Trusted devices", exact: true }).click();
  await page.getByRole("button", { name: "Pair a device" }).click();
  const dialog = page.getByRole("dialog", { name: "Pair a device" });
  await expect(dialog.getByText("482731", { exact: true })).toBeVisible();
  await expect(dialog.getByText("192.0.2.10:43123", { exact: true })).toBeVisible();
  await expect(dialog.getByText("Waiting for the other device...")).toBeVisible();
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).toHaveCount(0);
});

test("syncs an online trusted device and resolves both conflict choices", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.getByRole("link", { name: "Sync", exact: true }).click();
  await expect(page.getByText("Online", { exact: true })).toBeVisible();
  await expect(page.getByText("2 pending review")).toBeVisible();
  await page.getByRole("button", { name: "Sync now" }).click();
  await expect(page.getByText(/Sync complete.*conflicting edits found/)).toBeVisible();
  await page.getByRole("button", { name: "Discard" }).first().click();
  await expect(page.getByText("1 pending review")).toBeVisible();
  await page.getByRole("button", { name: "Keep as new secret" }).click();
  await expect(page.getByText(/pending review/)).toHaveCount(0);
  await expect(page.getByText("Everything is up to date.")).toBeVisible();
});

test("explains when no trusted desktop device can be synced", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.evaluate(() => {
    const state = (
      window as unknown as {
        __ENVRYN_E2E_STATE__: { trustedDevices: unknown[]; discoveredPeers: unknown[] };
      }
    ).__ENVRYN_E2E_STATE__;
    state.trustedDevices.splice(0);
    state.discoveredPeers.splice(0);
  });
  await page.getByRole("link", { name: "Sync", exact: true }).click();
  await expect(
    page.getByText("No trusted devices yet. Pair one from the Devices page."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Sync now" }).click();
  await expect(page.getByText("No trusted devices found on this network")).toBeVisible();
});

test("shows a desktop sync failure and succeeds after retry", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.evaluate(() => {
    const state = (
      window as unknown as {
        __ENVRYN_E2E_STATE__: { discoveredPeers: Array<{ addresses: string[] }> };
      }
    ).__ENVRYN_E2E_STATE__;
    state.discoveredPeers[0]!.addresses = ["192.0.2.99"];
  });
  await page.getByRole("link", { name: "Sync", exact: true }).click();
  await page.getByRole("button", { name: "Sync now" }).click();
  await expect(page.getByText("Sync could not complete for one or more devices.")).toBeVisible();
  await page.evaluate(() => {
    const state = (
      window as unknown as {
        __ENVRYN_E2E_STATE__: { discoveredPeers: Array<{ addresses: string[] }> };
      }
    ).__ENVRYN_E2E_STATE__;
    state.discoveredPeers[0]!.addresses = ["192.0.2.25"];
  });
  await page.getByRole("button", { name: "Retry" }).click();
  await expect(page.getByText("Everything is up to date.")).toBeVisible();
  await expect(page.getByText("Sync could not complete for one or more devices.")).toHaveCount(0);
});

test("imports reviewed env entries without adding an Imported tag", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.getByRole("button", { name: "Import .env" }).click();
  const dialog = page.getByRole("dialog", { name: "Import a .env file" });
  await dialog.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByText("No KEY=VALUE lines were found in that text.")).toBeVisible();
  await dialog
    .getByLabel(".env contents")
    .fill(
      "# disposable values only\nDATABASE_URL=postgres://qa.invalid/db\nIGDB_TOKEN=FAKE_IGDB_TOKEN\nPUBLIC_URL=https://example.invalid/hook",
    );
  await dialog.getByLabel("Project").fill("Imported QA");
  await dialog.getByLabel("Environment").selectOption("Staging");
  await dialog.getByRole("button", { name: "Continue" }).click();
  await expect(dialog.getByText("Review 3 variables before saving.")).toBeVisible();
  await expect(dialog.getByLabel("Type for DATABASE_URL")).toHaveValue("Database");
  await expect(dialog.getByLabel("Type for IGDB_TOKEN")).toHaveValue("Token");
  await dialog.getByLabel("Type for IGDB_TOKEN").selectOption("OAuth");
  await dialog.getByLabel("Import PUBLIC_URL").uncheck();

  const audit = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze();
  expect(
    audit.violations.filter((item) => ["serious", "critical"].includes(item.impact ?? "")),
  ).toEqual([]);

  await dialog.getByRole("button", { name: "Import 2 secrets" }).click();
  await expect(page.getByRole("button", { name: "Open DATABASE_URL details" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open IGDB_TOKEN details" })).toBeVisible();
  await expect(page.getByText("IMPORTED", { exact: true })).toHaveCount(0);
  await page.getByRole("link", { name: /^Databases/ }).click();
  await expect(page.getByRole("button", { name: "Open DATABASE_URL details" })).toBeVisible();
  await page.getByRole("link", { name: /^API & tokens/ }).click();
  await expect(page.getByRole("button", { name: "Open IGDB_TOKEN details" })).toBeVisible();
});

test("extracts, reviews, edits, and saves structured fields with local AI", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await page.getByRole("switch", { name: "Enable local AI" }).click();
  await expect(page.getByText("Running", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "All secrets", exact: true }).click();
  await page.getByRole("button", { name: "Extract fields" }).click();
  const dialog = page.getByRole("dialog", { name: "Extract fields from text" });
  await dialog.getByRole("button", { name: "Extract fields" }).click();
  await expect(dialog.getByText("Paste the text you want fields extracted from.")).toBeVisible();
  await dialog
    .getByLabel("Text to extract from")
    .fill("Host: db.example.test\nPort: 5432\nUsername: qa_user");
  await dialog.getByRole("button", { name: "Extract fields" }).click();
  await expect(dialog.getByText("Review the extracted fields before saving.")).toBeVisible();
  await dialog.getByLabel("Name").fill("Extracted Database Fields");
  await dialog.getByLabel("Project").fill("AI QA");
  await dialog.getByLabel("Environment").selectOption("Production");
  await dialog.getByPlaceholder("Label").first().fill("Server");
  await dialog.getByRole("button", { name: "Add field" }).click();
  await dialog.getByPlaceholder("Label").last().fill("Region");
  await dialog.getByPlaceholder("Value").last().fill("test-region-1");
  await dialog.getByRole("button", { name: "Remove field" }).nth(1).click();
  await dialog.getByRole("button", { name: "Save secret" }).click();
  const row = page.getByRole("button", { name: "Open Extracted Database Fields details" });
  await expect(row).toBeVisible();
  await row.click();
  const panel = page.locator(".secret-panel");
  await expect(panel).toContainText("Custom");
  await expect(panel).toContainText("Production");
  await panel.getByRole("button", { name: "Reveal" }).click();
  await expect(panel).toContainText("Server: db.example.test");
  await expect(panel).toContainText("Region: test-region-1");
});

test("creates every supported desktop secret type and exposes every category", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  test.setTimeout(60_000);
  await createDisposableVault(page);
  const cases: Array<{
    type: string;
    name: string;
    fill?: (dialog: ReturnType<Page["getByRole"]>) => Promise<void>;
  }> = [
    {
      type: "API Key",
      name: "QA API Key",
      fill: async (dialog) => dialog.getByLabel("Provider").fill("TMDB"),
    },
    {
      type: "Environment",
      name: "QA Environment",
      fill: async (dialog) => dialog.getByLabel("Variable Name").fill("QA_VARIABLE"),
    },
    { type: "Token", name: "QA Token" },
    {
      type: "Database",
      name: "QA Database",
      fill: async (dialog) => {
        await dialog.getByLabel("Host").fill("db.example.test");
        await dialog.getByLabel("Port").fill("5432");
        await dialog.getByLabel("Database").fill("qa_db");
        await dialog.getByLabel("Username").fill("qa_user");
      },
    },
    {
      type: "SSH",
      name: "QA SSH",
      fill: async (dialog) => {
        await dialog.getByLabel("Host").fill("ssh.example.test");
        await dialog.getByLabel("Username").fill("qa_user");
        await dialog.getByLabel("Passphrase").fill("fake-passphrase");
      },
    },
    {
      type: "OAuth",
      name: "QA OAuth",
      fill: async (dialog) => dialog.getByLabel("Client ID").fill("fake-client-id"),
    },
    {
      type: "Webhook",
      name: "QA Webhook",
      fill: async (dialog) => dialog.getByLabel("Endpoint").fill("https://example.invalid/hook"),
    },
    { type: "Note", name: "QA Secure Note" },
    { type: "Custom", name: "QA Custom" },
  ];

  for (const [index, item] of cases.entries()) {
    await page.getByRole("button", { name: "Add secret", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Add a secret" });
    await dialog.getByPlaceholder("e.g. OPENAI_API_KEY").fill(item.name);
    await dialog.getByLabel("What kind is it?").selectOption(item.type);
    await dialog.getByLabel("Project").fill("Type Matrix");
    await dialog
      .getByLabel("Environment")
      .selectOption(index % 2 === 0 ? "Development" : "Production");
    if (item.type === "Note") {
      await dialog.getByLabel("Note body").fill("Disposable private note text");
    } else if (item.type === "Custom") {
      await dialog.getByLabel("Custom field 1 name").fill("Account ID");
      await dialog.getByLabel("Custom field 1 value").fill("fake-account-id");
    } else {
      await dialog
        .getByPlaceholder("Paste secret value")
        .fill(`FAKE_${item.type.toUpperCase().replace(" ", "_")}_VALUE`);
    }
    await item.fill?.(dialog);
    await dialog.getByLabel("Notes").fill(`Notes for ${item.type}`);
    await dialog.getByLabel("Tags").fill(`qa, ${item.type.toLowerCase().replace(" ", "-")}`);
    await dialog.getByRole("button", { name: "Save secret" }).click();
    await expect(page.getByRole("button", { name: `Open ${item.name} details` })).toBeVisible();
  }

  await expect(page.getByRole("button", { name: /^Open QA / })).toHaveCount(9);
  const categoryChecks = [
    ["API & tokens", "QA API Key"],
    ["Databases", "QA Database"],
    ["SSH", "QA SSH"],
    ["Secure notes", "QA Secure Note"],
    ["Environments", "QA Environment"],
    ["Custom", "QA Custom"],
  ] as const;
  for (const [category, secret] of categoryChecks) {
    await page.getByRole("link", { name: new RegExp(`^${category}`) }).click();
    await expect(page.getByRole("button", { name: `Open ${secret} details` })).toBeVisible();
  }
});

test("uses local AI as a fallback for an uncommon provider name and type", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await page.getByRole("switch", { name: "Enable local AI" }).click();
  await expect(page.getByText("Running", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "All secrets", exact: true }).click();
  await page.getByRole("button", { name: "Add secret", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Add a secret" });
  await dialog
    .getByPlaceholder("Paste secret value")
    .fill("rare-service-value-with-no-known-prefix");
  await dialog.getByRole("button", { name: "Suggest type" }).click();
  await expect(dialog.getByLabel("What kind is it?")).toHaveValue("API Key");
  await expect(dialog.getByLabel("Provider")).toHaveValue("IGDB");
  await dialog.getByRole("button", { name: "Suggest name" }).click();
  await expect(dialog.getByPlaceholder("e.g. OPENAI_API_KEY")).toHaveValue("IGDB API Key");
});

test("renames a project and keeps its secrets attached to the stable project", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);
  await page.getByRole("link", { name: "Projects", exact: true }).click();
  await page.getByRole("button", { name: "New project" }).click();
  await page.getByPlaceholder("e.g. Rescripto").fill("Original Project");
  await page.getByRole("button", { name: "Create project" }).click();

  await page.getByRole("button", { name: "Add secret", exact: true }).first().click();
  const add = page.getByRole("dialog", { name: "Add a secret" });
  await add.getByPlaceholder("e.g. OPENAI_API_KEY").fill("Project API Key");
  await add.getByLabel("Environment").selectOption("Development");
  await add.getByPlaceholder("Paste secret value").fill("FAKE_PROJECT_API_KEY");
  await add.getByRole("button", { name: "Save secret" }).click();

  await page.getByRole("button", { name: "Rename project" }).click();
  const titleInput = page.getByRole("heading", { level: 1 }).locator("input");
  await titleInput.fill("Cancelled Rename");
  await page.getByRole("button", { name: "Cancel rename" }).click();
  await expect(page.getByRole("heading", { name: /Original Project/ })).toBeVisible();
  await page.getByRole("button", { name: "Rename project" }).click();
  await page.getByRole("heading", { level: 1 }).locator("input").fill("Renamed Project");
  await page.getByRole("button", { name: "Save name" }).click();
  await expect(page.getByRole("heading", { name: /Renamed Project/ })).toBeVisible();
  await expect(page).toHaveURL(/00000000-0000-4000-8000-000000000001/);
  await expect(page.getByRole("button", { name: "Open Project API Key details" })).toBeVisible();
});

test("supports desktop keyboard add, search, lock, failed unlock, and successful unlock", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  const password = "E2E keyboard password 42!";
  await createDisposableVault(page, password);
  await page.keyboard.press("Control+n");
  const add = page.getByRole("dialog", { name: "Add a secret" });
  await expect(add).toBeVisible();
  await add.getByPlaceholder("e.g. OPENAI_API_KEY").fill("Keyboard Secret");
  await add.getByLabel("Project").fill("Keyboard QA");
  await add.getByPlaceholder("Paste secret value").fill("FAKE_KEYBOARD_SECRET");
  await add.getByRole("button", { name: "Save secret" }).click();

  await page.keyboard.press("Control+k");
  const search = page.getByPlaceholder("Search your vault, then press Enter");
  await search.fill("keyboard qa");
  await expect(page.getByText("Keyboard Secret", { exact: true }).first()).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.locator(".secret-panel")).toContainText("Keyboard Secret");
  await page.keyboard.press("Escape");
  await expect(page.locator(".secret-panel")).toHaveCount(0);

  await page.keyboard.press("Control+l");
  await expect(page.getByRole("heading", { name: "Unlock Envryn" })).toBeVisible();
  await page.getByLabel("Master password").fill("wrong-password");
  await page.getByRole("button", { name: "Unlock vault" }).click();
  await expect(page.getByText("That password did not work. Please try again.")).toBeVisible();
  await page.getByLabel("Master password").fill(password);
  await page.getByRole("button", { name: "Unlock vault" }).click();
  await expect(page.getByRole("heading", { name: "Secrets" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open Keyboard Secret details" })).toBeVisible();
});

test("keeps 1,000 desktop records and 50 projects searchable and usable", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  test.setTimeout(60_000);
  const isolatedPerformanceRun = testInfo.config.workers === 1;
  await page.goto("/");
  await page.evaluate(() => {
    const state = (
      window as unknown as {
        __ENVRYN_E2E_STATE__: { seedSecrets: (count: number) => void };
      }
    ).__ENVRYN_E2E_STATE__;
    state.seedSecrets(1_000);
  });
  await page.getByLabel("Master password", { exact: true }).fill("E2E performance password 42!");
  await page.getByLabel("Confirm master password").fill("E2E performance password 42!");
  await page.evaluate(() => {
    const started = performance.now();
    (window as unknown as { __ENVRYN_LIST_READY_MS__: number | null }).__ENVRYN_LIST_READY_MS__ =
      null;
    const observer = new MutationObserver(() => {
      if (document.body.textContent?.includes("PERF_SECRET_0999")) {
        (
          window as unknown as { __ENVRYN_LIST_READY_MS__: number | null }
        ).__ENVRYN_LIST_READY_MS__ = performance.now() - started;
        observer.disconnect();
      }
    });
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });
  });
  await page.getByRole("button", { name: "Create vault" }).click();
  await expect(page.getByRole("button", { name: "Open PERF_SECRET_0999 details" })).toBeVisible();
  const listReadyMs = await page.evaluate(
    () =>
      (window as unknown as { __ENVRYN_LIST_READY_MS__: number | null }).__ENVRYN_LIST_READY_MS__ ??
      Number.POSITIVE_INFINITY,
  );
  expect(listReadyMs).toBeLessThan(isolatedPerformanceRun ? 3_000 : 4_000);
  await expect(page.getByRole("button", { name: /^Open PERF_SECRET_/ })).toHaveCount(1_000);

  await page.keyboard.press("Control+k");
  const search = page.getByPlaceholder("Search your vault, then press Enter");
  await page.evaluate(() => {
    const started = performance.now();
    const dialog = document.querySelector('[role="dialog"]');
    (
      window as unknown as { __ENVRYN_SEARCH_READY_MS__: number | null }
    ).__ENVRYN_SEARCH_READY_MS__ = null;
    const observer = new MutationObserver(() => {
      if (dialog?.textContent?.includes("PERF_SECRET_0777")) {
        (
          window as unknown as { __ENVRYN_SEARCH_READY_MS__: number | null }
        ).__ENVRYN_SEARCH_READY_MS__ = performance.now() - started;
        observer.disconnect();
      }
    });
    if (dialog) observer.observe(dialog, { childList: true, subtree: true, characterData: true });
  });
  await search.fill("PERF_SECRET_0777");
  await expect(
    page.getByRole("dialog", { name: "Search" }).getByRole("button", { name: /^PERF_SECRET_0777/ }),
  ).toBeVisible();
  const searchReadyMs = await page.evaluate(
    () =>
      (window as unknown as { __ENVRYN_SEARCH_READY_MS__: number | null })
        .__ENVRYN_SEARCH_READY_MS__ ?? Number.POSITIVE_INFINITY,
  );
  expect(searchReadyMs).toBeLessThan(isolatedPerformanceRun ? 300 : 500);
  await page.keyboard.press("Escape");

  await page.getByRole("link", { name: /^Projects/ }).click();
  await expect(page.getByText("Project 27", { exact: true })).toBeVisible();
  const projectStarted = Date.now();
  await page.getByText("Project 27", { exact: true }).click();
  await expect(page.getByRole("button", { name: "Open PERF_SECRET_0777 details" })).toBeVisible();
  const projectReadyMs = Date.now() - projectStarted;
  expect(projectReadyMs).toBeLessThan(1_000);
  testInfo.annotations.push({
    type: "performance",
    description: JSON.stringify({ listReadyMs, searchReadyMs, projectReadyMs }),
  });
});

test("keeps desktop layouts usable across release resolution and scaling equivalents", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "desktop-chromium", "Windows desktop flow");
  await createDisposableVault(page);

  const sizes = [
    { width: 1366, height: 768 },
    { width: 1093, height: 614 },
    { width: 900, height: 600 },
    { width: 683, height: 384 },
  ];
  for (const size of sizes) {
    await page.setViewportSize(size);
    await expect(page.getByRole("heading", { name: "Secrets" })).toBeVisible();
    const horizontalOverflow = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    );
    expect(horizontalOverflow).toBe(false);

    const addButton =
      size.width < 768
        ? page.getByRole("button", { name: "Add", exact: true })
        : page.getByRole("button", { name: "Add secret", exact: true });
    await addButton.click();
    const dialog = page.getByRole("dialog", { name: "Add a secret" });
    const box = await dialog.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.y).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(size.width);
    expect(box!.y + box!.height).toBeLessThanOrEqual(size.height + 2);
    await dialog.getByRole("button", { name: "Cancel" }).click();
  }

  await page.setViewportSize({ width: 900, height: 600 });
  await page.getByRole("button", { name: "Add secret", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Add a secret" });
  const longName = "VERY_LONG_ENVIRONMENT_CREDENTIAL_NAME_THAT_MUST_NOT_COVER_ACTION_BUTTONS_2026";
  await dialog.getByPlaceholder("e.g. OPENAI_API_KEY").fill(longName);
  await dialog
    .getByLabel("Project")
    .fill("A very long project name used for Windows text layout QA");
  await dialog.getByPlaceholder("Paste secret value").fill("FAKE_LONG_LAYOUT_VALUE");
  await dialog.getByRole("button", { name: "Save secret" }).click();
  const row = page.getByRole("button", { name: `Open ${longName} details` });
  await expect(row).toBeVisible();
  await row.focus();
  await expect(page.getByRole("button", { name: "Copy secret" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Reveal details" })).toBeVisible();
  await expect(page.getByRole("button", { name: "More actions" })).toBeVisible();

  await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
  await expect(page.getByRole("heading", { name: "Secrets" })).toBeVisible();
  const accessibility = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze();
  expect(
    accessibility.violations.filter((item) => ["serious", "critical"].includes(item.impact ?? "")),
  ).toEqual([]);
});
