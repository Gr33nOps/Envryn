import * as React from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { AlertTriangle, ArrowRight, Download, KeyRound, RotateCcw, Trash2 } from "lucide-react";
import { toast } from "sonner";
import {
  Button,
  ConfirmDialog,
  Panel,
  SectionLabel,
  Select,
  SettingsRow,
  Switch,
} from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/settings")({ component: Settings });

function Group({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
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

function Settings() {
  const [requireAuth, setRequireAuth] = React.useState(true);
  const [lockWithWindows, setLockWithWindows] = React.useState(true);
  const [discovery, setDiscovery] = React.useState(true);
  const [danger, setDanger] = React.useState<"reset" | "delete" | null>(null);

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
                description="Lock after you stop using Envryn."
                control={
                  <Select defaultValue="5" className="w-[130px]">
                    <option value="1">1 minute</option>
                    <option value="5">5 minutes</option>
                    <option value="15">15 minutes</option>
                    <option value="0">Never</option>
                  </Select>
                }
              />
              <SettingsRow
                label="Require authentication to reveal secrets"
                description="Use your master password or Windows Hello each time a value is revealed."
                control={
                  <div className="flex items-center gap-2">
                    <span className="text-[11.5px] text-muted-foreground">
                      {requireAuth ? "On" : "Off"}
                    </span>
                    <Switch checked={requireAuth} onCheckedChange={setRequireAuth} />
                  </div>
                }
              />
              <SettingsRow
                label="Clear clipboard after copying"
                description="Remove secret values from the clipboard after this long."
                control={
                  <Select defaultValue="30" className="w-[130px]">
                    <option value="10">10 seconds</option>
                    <option value="30">30 seconds</option>
                    <option value="60">60 seconds</option>
                  </Select>
                }
              />
              <SettingsRow
                label="Lock when Windows locks"
                description="Protect the vault whenever your Windows session is locked."
                control={
                  <div className="flex items-center gap-2">
                    <span className="text-[11.5px] text-muted-foreground">
                      {lockWithWindows ? "On" : "Off"}
                    </span>
                    <Switch checked={lockWithWindows} onCheckedChange={setLockWithWindows} />
                  </div>
                }
              />
            </Group>

            <Group
              label="Devices and sync"
              description="Envryn uses your local network to connect devices you have approved."
            >
              <SettingsRow
                label="Allow local device discovery"
                description="Lets trusted devices find this PC on your LAN. It does not share secret values with unknown devices."
                control={
                  <div className="flex items-center gap-2">
                    <span className="text-[11.5px] text-muted-foreground">
                      {discovery ? "On" : "Off"}
                    </span>
                    <Switch checked={discovery} onCheckedChange={setDiscovery} />
                  </div>
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

            <Group
              label="Backup"
              description="Create a protected copy before moving to another PC or changing vault settings."
            >
              <SettingsRow
                label="Encrypted backup"
                description="Create, restore, or open your latest backup file."
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
                control={<span className="text-[12px] text-muted-foreground">Version 0.1.0</span>}
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
                    <Button
                      size="sm"
                      onClick={() => toast("Password change flow is ready to connect")}
                    >
                      <KeyRound />
                      Change
                    </Button>
                  }
                />
                <SettingsRow
                  label="Export vault"
                  description="Save a copy of your vault data to a file."
                  control={
                    <Button size="sm" onClick={() => toast("Export flow is ready to connect")}>
                      <Download />
                      Export
                    </Button>
                  }
                />
                <SettingsRow
                  label="Reset vault"
                  description="Remove the current vault and start again."
                  control={
                    <Button size="sm" variant="danger" onClick={() => setDanger("reset")}>
                      <RotateCcw />
                      Reset
                    </Button>
                  }
                />
                <SettingsRow
                  label="Delete vault"
                  description="Permanently delete this vault and all of its secrets."
                  control={
                    <Button size="sm" variant="danger" onClick={() => setDanger("delete")}>
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
                  <p className="text-[12px] font-medium">A note about local sync</p>
                  <p className="mt-1.5 text-[11.5px] leading-relaxed text-muted-foreground">
                    Only devices you approve can connect. You can remove a device at any time from
                    Trusted devices.
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
      <ConfirmDialog
        open={danger !== null}
        onOpenChange={(open) => !open && setDanger(null)}
        title={danger === "delete" ? "Delete this vault?" : "Reset this vault?"}
        body={
          danger === "delete"
            ? "This permanently deletes the vault and all 13 secrets from this PC. Create an encrypted backup first if you might need them later."
            : "This removes the current vault from this PC and starts a new one. Create an encrypted backup first if you want to keep these secrets."
        }
        confirmLabel={danger === "delete" ? "Delete vault" : "Reset vault"}
        onConfirm={() => {
          setDanger(null);
          toast(
            danger === "delete"
              ? "Delete vault flow is ready to connect"
              : "Reset vault flow is ready to connect",
          );
        }}
      />
    </div>
  );
}
