import * as React from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { AlertTriangle, ArrowRight, Download, KeyRound, RotateCcw, Trash2 } from "lucide-react";
import { toast } from "sonner";
import {
  Button,
  Field,
  Input,
  Modal,
  Panel,
  SectionLabel,
  Select,
  SettingsRow,
  Switch,
} from "@/components/envryn/ui";
import * as ipc from "@/lib/ipc";
import {
  IpcError,
  settingsGet,
  settingsSet,
  vaultChangePassword,
  vaultDisablePlatformProtection,
  vaultEnablePlatformProtection,
  vaultStatus,
  type AppSettings,
} from "@/lib/ipc";

export const Route = createFileRoute("/vault/settings")({ component: Settings });

function Group({
  label,
  description,
  children,
}: Readonly<{
  label: string;
  description?: string;
  children: React.ReactNode;
}>) {
  return (
    <section className="space-y-1.5">
      <div>
        <SectionLabel>{label}</SectionLabel>
        {description && <p className="mt-1 text-[11px] text-subtle-foreground">{description}</p>}
      </div>
      <Panel>{children}</Panel>
    </section>
  );
}

/** Ask for the current master password before enabling platform protection --
 * see docs/CRYPTOGRAPHY.md: enabling an alternate unlock path deserves the
 * same friction as changing the primary one. */
function EnablePlatformProtectionModal({
  open,
  onOpenChange,
  onEnabled,
}: Readonly<{
  open: boolean;
  onOpenChange: (v: boolean) => void;
  onEnabled: () => void;
}>) {
  const [password, setPassword] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [loading, setLoading] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setPassword("");
      setError(null);
      setLoading(false);
    }
  }, [open]);

  async function confirm() {
    setError(null);
    setLoading(true);
    try {
      await vaultEnablePlatformProtection(password);
      onOpenChange(false);
      onEnabled();
      toast("This Windows account can now unlock the vault");
    } catch (err) {
      setError(
        err instanceof IpcError && err.code === "auth_failed"
          ? "That password did not work."
          : "That could not be enabled.",
      );
    } finally {
      setLoading(false);
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title="Confirm your master password"
      description="This lets Envryn unlock without your master password, tied to this Windows user account on this PC. Your master password still works too."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button variant="primary" loading={loading} onClick={() => void confirm()}>
            Enable
          </Button>
        </>
      }
    >
      <Field label="Master password" error={error ?? undefined}>
        <Input
          type="password"
          autoFocus
          value={password}
          invalid={Boolean(error)}
          onChange={(event) => {
            setPassword(event.target.value);
            setError(null);
          }}
        />
      </Field>
    </Modal>
  );
}

function ChangePasswordModal({
  open,
  onOpenChange,
}: Readonly<{
  open: boolean;
  onOpenChange: (v: boolean) => void;
}>) {
  const [current, setCurrent] = React.useState("");
  const [next, setNext] = React.useState("");
  const [confirmValue, setConfirmValue] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [loading, setLoading] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setCurrent("");
      setNext("");
      setConfirmValue("");
      setError(null);
      setLoading(false);
    }
  }, [open]);

  async function confirm() {
    setError(null);
    if (next.length < 8) {
      setError("Your new password must be at least 8 characters.");
      return;
    }
    if (next !== confirmValue) {
      setError("Those passwords do not match.");
      return;
    }
    setLoading(true);
    try {
      await vaultChangePassword(current, next);
      onOpenChange(false);
      toast("Master password changed");
    } catch (err) {
      setError(
        err instanceof IpcError && err.code === "auth_failed"
          ? "Your current password did not match."
          : "That could not be changed.",
      );
    } finally {
      setLoading(false);
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title="Change master password"
      description="This re-wraps your vault key instantly. Nothing needs to be re-encrypted."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button variant="primary" loading={loading} onClick={() => void confirm()}>
            Change password
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="Current password">
          <Input
            type="password"
            autoFocus
            value={current}
            onChange={(event) => setCurrent(event.target.value)}
          />
        </Field>
        <Field label="New password">
          <Input type="password" value={next} onChange={(event) => setNext(event.target.value)} />
        </Field>
        <Field label="Confirm new password" error={error ?? undefined}>
          <Input
            type="password"
            invalid={Boolean(error)}
            value={confirmValue}
            onChange={(event) => setConfirmValue(event.target.value)}
          />
        </Field>
      </div>
    </Modal>
  );
}

/** Local AI: off by default, runs entirely on this device. Every control
 * here mirrors src-tauri/src/ai.rs's own fail-closed behaviour -- turning
 * the switch off is a real "no," not just a UI hint. */
function AiSettingsGroup({
  settings,
  updateSettings,
}: Readonly<{
  settings: AppSettings | null;
  updateSettings: (patch: Partial<AppSettings>) => Promise<void>;
}>) {
  const [status, setStatus] = React.useState<ipc.AiStatus | null>(null);
  const [downloading, setDownloading] = React.useState(false);
  const [downloadProgress, setDownloadProgress] = React.useState<ipc.AiDownloadProgress | null>(
    null,
  );
  const [starting, setStarting] = React.useState(false);

  const refreshStatus = React.useCallback(() => {
    ipc
      .aiStatus()
      .then(setStatus)
      .catch(() => {
        // AI status just stays unknown; every other setting on this page
        // still works.
      });
  }, []);

  React.useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  async function downloadModel() {
    setDownloading(true);
    setDownloadProgress(null);
    const unlisten = await ipc.listenAiDownloadProgress(setDownloadProgress);
    try {
      await ipc.aiDownloadModel();
      toast("Local AI model downloaded");
    } catch (err) {
      toast(err instanceof IpcError ? err.message : "Could not download the model.");
    } finally {
      unlisten();
      setDownloading(false);
      setDownloadProgress(null);
      refreshStatus();
    }
  }

  function downloadButtonLabel(): string {
    if (!downloading) return "Download";
    if (!downloadProgress || downloadProgress.total_bytes === 0) return "Starting…";
    const pct = Math.floor(
      (downloadProgress.bytes_downloaded / downloadProgress.total_bytes) * 100,
    );
    const label = downloadProgress.file_name === "tokenizer.json" ? "Tokenizer" : "Model";
    return `${label} ${pct}%`;
  }

  async function toggleAi(enable: boolean) {
    await updateSettings({ ai_enabled: enable });
    if (enable) {
      setStarting(true);
      try {
        await ipc.aiStart();
        toast("Local AI is running");
      } catch (err) {
        toast(err instanceof IpcError ? err.message : "Could not start local AI.");
      } finally {
        setStarting(false);
        refreshStatus();
      }
    } else {
      await ipc.aiStop().catch(() => {});
      refreshStatus();
    }
  }

  return (
    <Group
      label="Local AI"
      description="Optional. Runs entirely on this device -- the only network access is the one-time model download below, never during normal use."
    >
      <SettingsRow
        label="Enable local AI"
        description="Credential classification, naming suggestions, and natural-language search. Off by default."
        control={
          <Switch
            checked={settings?.ai_enabled ?? false}
            onCheckedChange={(checked) => !starting && void toggleAi(checked)}
          />
        }
      />
      <SettingsRow
        label="Model"
        description={
          status?.model_downloaded
            ? status.model_name
            : downloading
              ? "Downloading -- about 1 GB, this can take several minutes on an ordinary connection."
              : "Not downloaded yet (about 1 GB, one-time)"
        }
        control={
          status?.model_downloaded ? (
            <span className="text-[11.5px] text-success">Ready</span>
          ) : (
            <Button size="sm" loading={downloading} onClick={() => void downloadModel()}>
              <Download />
              {downloadButtonLabel()}
            </Button>
          )
        }
      />
      {settings?.ai_enabled && (
        <SettingsRow
          label="Status"
          control={
            <span className="text-[11.5px] text-muted-foreground">
              {status?.engine_running ? "Running" : "Not running"}
            </span>
          }
        />
      )}
    </Group>
  );
}

const AUTO_LOCK_OPTIONS = [1, 5, 15, 30, 60];
const CLIPBOARD_OPTIONS = [10, 30, 60, 120];

function Settings() {
  const [settings, setSettings] = React.useState<AppSettings | null>(null);
  const [platformAvailable, setPlatformAvailable] = React.useState(false);
  const [platformEnabled, setPlatformEnabled] = React.useState(false);
  const [enabling, setEnabling] = React.useState(false);
  const [changingPassword, setChangingPassword] = React.useState(false);

  React.useEffect(() => {
    settingsGet()
      .then(setSettings)
      .catch(() => toast("Could not load settings"));
    vaultStatus()
      .then((status) => {
        setPlatformAvailable(status.platform_protection_available);
        setPlatformEnabled(status.platform_protection_enabled);
      })
      .catch(() => {
        // Platform protection availability just stays unknown/off; every
        // other row on this page still works.
      });
  }, []);

  async function updateSettings(patch: Partial<AppSettings>) {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    try {
      await settingsSet(next);
    } catch {
      toast("That setting could not be saved");
    }
  }

  async function togglePlatformProtection(enable: boolean) {
    if (enable) {
      setEnabling(true);
      return;
    }
    try {
      await vaultDisablePlatformProtection();
      setPlatformEnabled(false);
      toast("This Windows account can no longer unlock the vault on its own");
    } catch {
      toast("That could not be disabled");
    }
  }

  return (
    <div className="min-h-full bg-background">
      <div className="content-wrap content-wrap--narrow">
        <header className="page-hero">
          <div>
            <p className="breadcrumb">
              Vault <span>/</span> Settings
            </p>
            <h1 className="mt-3 text-[22px] font-semibold tracking-[-0.035em]">Settings</h1>
            <p className="mt-1.5 text-[12.5px] text-muted-foreground">
              Choose how Envryn locks, syncs, and protects your vault.
            </p>
          </div>
        </header>

        <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_260px]">
          <div className="space-y-5">
            <Group
              label="Security"
              description="These controls affect when someone can see your secret values."
            >
              <SettingsRow
                label="Auto-lock the vault"
                description="Lock after this long with no keyboard or mouse activity anywhere on this PC."
                control={
                  <Select
                    value={String(settings?.auto_lock_minutes ?? 5)}
                    onChange={(event) =>
                      void updateSettings({ auto_lock_minutes: Number(event.target.value) })
                    }
                    className="w-[130px]"
                  >
                    {AUTO_LOCK_OPTIONS.map((minutes) => (
                      <option key={minutes} value={minutes}>
                        {minutes} minute{minutes === 1 ? "" : "s"}
                      </option>
                    ))}
                  </Select>
                }
              />
              <SettingsRow
                label="Clear clipboard after copying"
                description="Remove secret values from the clipboard after this long."
                control={
                  <Select
                    value={String(settings?.clipboard_clear_seconds ?? 30)}
                    onChange={(event) =>
                      void updateSettings({ clipboard_clear_seconds: Number(event.target.value) })
                    }
                    className="w-[130px]"
                  >
                    {CLIPBOARD_OPTIONS.map((seconds) => (
                      <option key={seconds} value={seconds}>
                        {seconds} seconds
                      </option>
                    ))}
                  </Select>
                }
              />
              {platformAvailable && (
                <SettingsRow
                  label="Unlock with this Windows account"
                  description="Skip the master password on this PC. Protected by Windows, tied to this user account. Your master password always keeps working."
                  control={
                    <div className="flex items-center gap-2">
                      <span className="text-[11.5px] text-muted-foreground">
                        {platformEnabled ? "On" : "Off"}
                      </span>
                      <Switch
                        checked={platformEnabled}
                        onCheckedChange={(checked) => void togglePlatformProtection(checked)}
                      />
                    </div>
                  }
                />
              )}
            </Group>

            <Group
              label="Devices and sync"
              description="Pair a device and sync directly over your local network -- no account, no cloud."
            >
              <SettingsRow
                label="Trusted devices"
                description="Pair a new device, or rename and revoke existing ones."
                control={
                  <Link to="/vault/devices">
                    <Button size="sm">
                      Manage devices <ArrowRight />
                    </Button>
                  </Link>
                }
              />
              <SettingsRow
                label="Sync details"
                description="See connected devices, activity, and conflicts."
                control={
                  <Link to="/vault/sync">
                    <Button size="sm">
                      View sync details <ArrowRight />
                    </Button>
                  </Link>
                }
              />
            </Group>

            <AiSettingsGroup settings={settings} updateSettings={updateSettings} />

            <Group
              label="Backup"
              description="Create a protected copy before moving to another PC or changing vault settings."
            >
              <SettingsRow
                label="Encrypted backup"
                description="Create or restore an encrypted backup file."
                control={
                  <Link to="/vault/backup">
                    <Button size="sm">
                      View backup details <ArrowRight />
                    </Button>
                  </Link>
                }
              />
            </Group>

            <Group label="Keyboard shortcuts" description="Shortcuts for the most common actions.">
              <SettingsRow label="Search the vault" control={<span className="kbd">Ctrl K</span>} />
              <SettingsRow label="Add a secret" control={<span className="kbd">Ctrl N</span>} />
              <SettingsRow label="Lock the vault" control={<span className="kbd">Ctrl L</span>} />
            </Group>

            <Group label="About">
              <SettingsRow
                label="Envryn"
                control={<span className="text-[12px] text-muted-foreground">Version 0.1.6</span>}
              />
              <SettingsRow
                label="Security documentation"
                control={
                  <Button size="sm" onClick={() => toast("Security documentation is coming soon")}>
                    View
                  </Button>
                }
              />
            </Group>

            <section className="space-y-1.5">
              <div>
                <SectionLabel>Danger zone</SectionLabel>
                <p className="mt-1 text-[11px] text-subtle-foreground">
                  These actions can change or permanently remove your vault.
                </p>
              </div>
              <Panel className="border-destructive/35">
                <SettingsRow
                  label="Change vault password"
                  description="Update the password used to unlock this vault."
                  control={
                    <Button size="sm" onClick={() => setChangingPassword(true)}>
                      <KeyRound />
                      Change
                    </Button>
                  }
                />
                <SettingsRow
                  label="Export vault"
                  description="Save an encrypted copy of your vault data to a file."
                  control={
                    <Link to="/vault/backup">
                      <Button size="sm">
                        <Download />
                        Export
                      </Button>
                    </Link>
                  }
                />
                <SettingsRow
                  label="Reset vault"
                  description="Remove the current vault and start again. Not yet available -- use Delete vault, then create a new one, once this ships."
                  control={
                    <Button size="sm" variant="danger" disabled title="Coming soon">
                      <RotateCcw />
                      Reset
                    </Button>
                  }
                />
                <SettingsRow
                  label="Delete vault"
                  description="Permanently delete this vault and all of its secrets. Not yet available."
                  control={
                    <Button size="sm" variant="danger" disabled title="Coming soon">
                      <Trash2 />
                      Delete
                    </Button>
                  }
                />
              </Panel>
            </section>
          </div>
          <aside className="hidden lg:block">
            <div className="sticky top-5 rounded-lg border border-border bg-surface p-4">
              <div className="flex items-start gap-2.5">
                <AlertTriangle className="mt-0.5 size-4 shrink-0 text-warning" />
                <div>
                  <p className="text-[12px] font-medium">A note about sync</p>
                  <p className="mt-1.5 text-[11.5px] leading-relaxed text-muted-foreground">
                    Devices sync directly over your local network after you pair them -- there is no
                    account and no cloud in between.
                  </p>
                  <Link
                    to="/vault/devices"
                    className="mt-3 inline-flex items-center gap-1 text-[11.5px] text-primary hover:text-foreground"
                  >
                    Review devices <ArrowRight className="size-3.5" />
                  </Link>
                </div>
              </div>
            </div>
          </aside>
        </div>
      </div>

      <EnablePlatformProtectionModal
        open={enabling}
        onOpenChange={setEnabling}
        onEnabled={() => setPlatformEnabled(true)}
      />
      <ChangePasswordModal open={changingPassword} onOpenChange={setChangingPassword} />
    </div>
  );
}
