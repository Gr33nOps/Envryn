import * as React from "react";
import { Archive, Check, ExternalLink, FileDown, RotateCcw, ShieldCheck } from "lucide-react";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import { Button, DetailRow, Field, Input, Modal, Panel } from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/backup")({ component: Backup });

function Backup() {
  const [creating, setCreating] = React.useState(false);
  const [restoring, setRestoring] = React.useState(false);
  const [restoreStep, setRestoreStep] = React.useState<"password" | "review">("password");
  const [mismatch, setMismatch] = React.useState(false);
  const [restoreError, setRestoreError] = React.useState(false);

  function openRestore() {
    setRestoreStep("password");
    setRestoreError(false);
    setRestoring(true);
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
          <Panel className="overflow-hidden">
            <div className="flex items-start gap-3 border-b border-border px-4 py-4">
              <span className="inline-flex size-9 items-center justify-center rounded-md border border-border bg-surface-2 text-muted-foreground">
                <Archive className="size-4" />
              </span>
              <div>
                <p className="text-[13px] font-medium">Latest backup</p>
                <p className="mt-1 text-[11.5px] text-muted-foreground">
                  August 18, 2026 at 4:20 PM
                </p>
              </div>
              <span className="status-ready ml-auto inline-flex items-center gap-1.5 text-[11px]">
                <Check className="size-3.5" />
                Ready
              </span>
            </div>
            <div className="grid gap-4 px-4 py-4 sm:grid-cols-2">
              <DetailRow label="File" value="envryn-backup-2026-08-18" mono />
              <DetailRow label="Size" value="3.2 MB" />
              <DetailRow label="Location" value={"C:\\Users\\You\\Documents\\Envryn"} mono />
              <DetailRow label="Protection" value="Backup password" />
            </div>
            <div className="flex flex-wrap gap-2 border-t border-border px-4 py-3">
              <Button variant="primary" onClick={() => setCreating(true)}>
                <FileDown />
                Back up now
              </Button>
              <Button onClick={() => toast("Opening the backup folder")}>
                <ExternalLink />
                Open location
              </Button>
              <Button onClick={openRestore}>
                <RotateCcw />
                Restore
              </Button>
              <span className="self-center text-[10.5px] text-subtle-foreground">
                Restore replaces this vault
              </span>
            </div>
          </Panel>
          <Panel className="p-4">
            <div className="flex items-start gap-2.5">
              <ShieldCheck className="mt-0.5 size-4 shrink-0 text-primary" />
              <div>
                <p className="text-[12px] font-medium">How backup protection works</p>
                <p className="mt-1.5 text-[11.5px] leading-relaxed text-muted-foreground">
                  Your backup is protected by a separate password. Envryn cannot recover it if you
                  forget it.
                </p>
              </div>
            </div>
          </Panel>
        </div>
      </div>

      <Modal
        open={creating}
        onOpenChange={setCreating}
        title="Create encrypted backup"
        description="Choose a password for this backup. Keep it separate from your vault password."
        footer={
          <>
            <Button onClick={() => setCreating(false)}>Cancel</Button>
            <Button
              variant="primary"
              onClick={() => {
                setCreating(false);
                toast("Backup created");
              }}
            >
              Create backup
            </Button>
          </>
        }
      >
        <div className="space-y-3">
          <Field label="Backup password" hint="You will need this password to restore the file.">
            <Input type="password" autoFocus />
          </Field>
          <Field label="Confirm password" error={mismatch ? "Passwords do not match." : undefined}>
            <Input
              type="password"
              invalid={mismatch}
              onBlur={(event) => setMismatch(event.target.value === "x")}
            />
          </Field>
        </div>
      </Modal>

      <Modal
        open={restoring}
        onOpenChange={setRestoring}
        title="Restore backup"
        description={
          restoreStep === "password"
            ? "Check the backup details, then enter its password."
            : "Review what will happen before restoring."
        }
        footer={
          restoreStep === "review" ? (
            <>
              <Button onClick={() => setRestoring(false)}>Cancel</Button>
              <Button
                variant="primary"
                onClick={() => {
                  setRestoring(false);
                  toast("Backup restored");
                }}
              >
                Restore vault
              </Button>
            </>
          ) : (
            <>
              <Button onClick={() => setRestoring(false)}>Cancel</Button>
              <Button
                variant="primary"
                onClick={() => {
                  setRestoreError(false);
                  setRestoreStep("review");
                }}
              >
                Continue
              </Button>
            </>
          )
        }
      >
        {restoreStep === "password" ? (
          <div className="space-y-4">
            <div className="rounded-md border border-border bg-surface-2/45 p-3">
              <DetailRow label="Backup file" value="envryn-backup-2026-08-18" mono />
              <div className="mt-3 grid grid-cols-2 gap-3">
                <DetailRow label="Created" value="August 18, 2026" />
                <DetailRow label="Size" value="3.2 MB" />
              </div>
            </div>
            <Field
              label="Backup password"
              error={restoreError ? "That password did not work." : undefined}
            >
              <Input type="password" invalid={restoreError} autoFocus />
            </Field>
          </div>
        ) : (
          <div className="space-y-3">
            <div className="rounded-md border border-warning/35 bg-warning/10 p-3 text-[11.5px] leading-relaxed text-muted-foreground">
              <p className="font-medium text-warning">Restoring replaces the current vault.</p>
              <p className="mt-1">
                Create a fresh backup first if you want to keep the secrets currently on this PC.
              </p>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <DetailRow label="Restore from" value="August 18, 2026" />
              <DetailRow label="Secrets inside" value="13" />
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
