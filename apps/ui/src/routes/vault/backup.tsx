import * as React from "react";
import { Archive, FileDown, RotateCcw, ShieldCheck } from "lucide-react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { toast } from "sonner";
import { Button, Field, Input, Modal } from "@/components/envryn/ui";
import { PasswordStrengthMeter } from "@/components/envryn/PasswordStrengthMeter";
import { backupCreate, backupRestore, IpcError } from "@/lib/ipc";
import { useRefreshVaultCache } from "@/lib/use-vault";

export const Route = createFileRoute("/vault/backup")({ component: Backup });

function restoreErrorMessage(err: unknown): string {
  if (err instanceof IpcError) {
    return err.code === "auth_failed" ? "That backup password did not work." : err.message;
  }
  return "That backup could not be restored.";
}

function CreateBackupModal({
  open,
  onOpenChange,
}: Readonly<{
  open: boolean;
  onOpenChange: (v: boolean) => void;
}>) {
  const [path, setPath] = React.useState("");
  const [password, setPassword] = React.useState("");
  const [confirm, setConfirm] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [loading, setLoading] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setPath("");
      setPassword("");
      setConfirm("");
      setError(null);
      setLoading(false);
    }
  }, [open]);

  async function create() {
    setError(null);
    if (!path.trim()) {
      setError("Choose where to save the backup file.");
      return;
    }
    if (password.length < 8) {
      setError("Your backup password must be at least 8 characters.");
      return;
    }
    if (password !== confirm) {
      setError("Passwords do not match.");
      return;
    }
    setLoading(true);
    try {
      await backupCreate(path.trim(), password);
      onOpenChange(false);
      toast("Backup created", { description: path.trim() });
    } catch (err) {
      setError(err instanceof IpcError ? err.message : "That backup could not be created.");
    } finally {
      setLoading(false);
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title="Create encrypted backup"
      description="Choose a password for this backup, separate from your vault password. Envryn cannot recover a lost backup password."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button variant="primary" loading={loading} onClick={() => void create()}>
            Create backup
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field
          label="Save to"
          hint="A full file path on this PC, e.g. C:\Users\You\Documents\envryn-backup.envrynbk"
        >
          <Input
            mono
            autoFocus
            value={path}
            onChange={(event) => setPath(event.target.value)}
            placeholder="C:\Users\You\Documents\envryn-backup.envrynbk"
          />
        </Field>
        <Field label="Backup password" error={error ?? undefined}>
          <Input
            type="password"
            invalid={Boolean(error)}
            value={password}
            onChange={(event) => {
              setPassword(event.target.value);
              setError(null);
            }}
          />
          <PasswordStrengthMeter password={password} />
        </Field>
        <Field label="Confirm password">
          <Input
            type="password"
            value={confirm}
            onChange={(event) => {
              setConfirm(event.target.value);
              setError(null);
            }}
          />
        </Field>
      </div>
    </Modal>
  );
}

function RestoreBackupModal({
  open,
  onOpenChange,
  onRestored,
}: Readonly<{
  open: boolean;
  onOpenChange: (v: boolean) => void;
  onRestored: (count: number) => void;
}>) {
  const [path, setPath] = React.useState("");
  const [backupPassword, setBackupPassword] = React.useState("");
  const [newPassword, setNewPassword] = React.useState("");
  const [confirm, setConfirm] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [loading, setLoading] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setPath("");
      setBackupPassword("");
      setNewPassword("");
      setConfirm("");
      setError(null);
      setLoading(false);
    }
  }, [open]);

  async function restore() {
    setError(null);
    if (!path.trim()) {
      setError("Choose the backup file to restore.");
      return;
    }
    if (newPassword.length < 8) {
      setError("Your new master password must be at least 8 characters.");
      return;
    }
    if (newPassword !== confirm) {
      setError("Those new passwords do not match.");
      return;
    }
    setLoading(true);
    try {
      const summary = await backupRestore(path.trim(), backupPassword, newPassword);
      onOpenChange(false);
      onRestored(summary.restored);
    } catch (err) {
      setError(restoreErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title="Restore from backup"
      description="This replaces the vault currently on this PC. The existing vault file is kept, renamed aside with a timestamp, not deleted."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button variant="primary" loading={loading} onClick={() => void restore()}>
            Restore vault
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="rounded-md border border-warning/35 bg-warning/10 p-3 text-[11.5px] leading-relaxed text-muted-foreground">
          <p className="font-medium text-warning">Restoring replaces the current vault.</p>
          <p className="mt-1">
            You will choose a new master password below -- restoring does not reuse the backup's
            password or the current vault's password.
          </p>
        </div>
        <Field label="Backup file" hint="The full path to the .envrynbk file.">
          <Input
            mono
            autoFocus
            value={path}
            onChange={(event) => setPath(event.target.value)}
            placeholder="C:\Users\You\Documents\envryn-backup.envrynbk"
          />
        </Field>
        <Field label="Backup password">
          <Input
            type="password"
            value={backupPassword}
            onChange={(event) => setBackupPassword(event.target.value)}
          />
        </Field>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <Field label="New master password" error={error ?? undefined}>
            <Input
              type="password"
              invalid={Boolean(error)}
              value={newPassword}
              onChange={(event) => {
                setNewPassword(event.target.value);
                setError(null);
              }}
            />
            <PasswordStrengthMeter password={newPassword} />
          </Field>
          <Field label="Confirm new password">
            <Input
              type="password"
              value={confirm}
              onChange={(event) => {
                setConfirm(event.target.value);
                setError(null);
              }}
            />
          </Field>
        </div>
      </div>
    </Modal>
  );
}

function Backup() {
  const navigate = useNavigate();
  const refreshVaultCache = useRefreshVaultCache();
  const [creating, setCreating] = React.useState(false);
  const [restoring, setRestoring] = React.useState(false);

  function handleRestored(count: number) {
    refreshVaultCache();
    toast(`Restored ${count} secret${count === 1 ? "" : "s"}`, {
      description: "Your vault now uses the new password you chose.",
    });
    void navigate({ to: "/vault" });
  }

  return (
    <div className="min-h-full bg-background">
      <div className="content-wrap content-wrap--narrow">
        <header className="page-hero">
          <div>
            <p className="breadcrumb">
              Vault <span>/</span> Backup
            </p>
            <h1 className="mt-3 text-[22px] font-semibold tracking-[-0.035em]">Backup</h1>
            <p className="mt-1.5 max-w-[54ch] text-[12.5px] text-muted-foreground">
              Keep an encrypted copy of your vault somewhere safe.
            </p>
          </div>
          <Button variant="primary" size="lg" onClick={() => setCreating(true)}>
            <FileDown />
            Back up now
          </Button>
        </header>

        <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_280px]">
          <div className="overflow-hidden rounded-lg border border-border bg-surface">
            <div className="flex items-start gap-3 border-b border-border px-4 py-4">
              <span className="inline-flex size-9 items-center justify-center rounded-md border border-border bg-surface-2 text-muted-foreground">
                <Archive className="size-4" />
              </span>
              <div>
                <p className="text-[13px] font-medium">Encrypted backup</p>
                <p className="mt-1 text-[11.5px] text-muted-foreground">
                  Envryn does not keep a history of past backups -- each one is a file you choose
                  the location for.
                </p>
              </div>
            </div>
            <div className="flex flex-wrap gap-2 px-4 py-3">
              <Button variant="primary" onClick={() => setCreating(true)}>
                <FileDown />
                Back up now
              </Button>
              <Button onClick={() => setRestoring(true)}>
                <RotateCcw />
                Restore
              </Button>
              <span className="self-center text-[10.5px] text-subtle-foreground">
                Restore replaces this vault
              </span>
            </div>
          </div>
          <div className="rounded-lg border border-border bg-surface p-4">
            <div className="flex items-start gap-2.5">
              <ShieldCheck className="mt-0.5 size-4 shrink-0 text-primary" />
              <div>
                <p className="text-[12px] font-medium">How backup protection works</p>
                <p className="mt-1.5 text-[11.5px] leading-relaxed text-muted-foreground">
                  Your backup is protected by its own password, independent of your vault's master
                  password and this vault's encryption key. Envryn cannot recover it if you forget
                  it.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>

      <CreateBackupModal open={creating} onOpenChange={setCreating} />
      <RestoreBackupModal
        open={restoring}
        onOpenChange={setRestoring}
        onRestored={handleRestored}
      />
    </div>
  );
}
