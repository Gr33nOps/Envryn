import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import {
  Button,
  DetailRow,
  Field,
  Input,
  Modal,
  PageHeader,
  Panel,
} from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/backup")({
  component: Backup,
});

function Backup() {
  const [creating, setCreating] = React.useState(false);
  const [restoring, setRestoring] = React.useState(false);
  const [mismatch, setMismatch] = React.useState(false);
  const [restoreError, setRestoreError] = React.useState(false);

  return (
    <>
      <PageHeader
        title="Backup"
        subtitle="Create an encrypted offline copy of your Envryn vault."
      />

      <div className="max-w-[560px] space-y-3 px-5 pb-5">
        <Panel className="px-3 py-2.5">
          <DetailRow
            label="Last backup"
            value={
              <span>
                August 18, 2026
                <span className="ml-2 font-mono text-[11.5px] text-subtle-foreground">
                  envryn-backup-2026-08-18
                </span>
              </span>
            }
          />
        </Panel>

        <div className="flex items-center gap-2">
          <Button variant="primary" onClick={() => setCreating(true)}>
            Create Encrypted Backup
          </Button>
          <Button onClick={() => setRestoring(true)}>Restore Backup</Button>
        </div>
      </div>

      <Modal
        open={creating}
        onOpenChange={setCreating}
        title="Create Encrypted Backup"
        description="Protect the backup with a password."
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
              Create Backup
            </Button>
          </>
        }
      >
        <div className="space-y-3">
          <Field label="Backup password">
            <Input type="password" autoFocus />
          </Field>
          <Field
            label="Confirm password"
            error={mismatch ? "Passwords don't match." : undefined}
          >
            <Input
              type="password"
              invalid={mismatch}
              onBlur={(e) => setMismatch(e.target.value === "x")}
            />
          </Field>
        </div>
      </Modal>

      <Modal
        open={restoring}
        onOpenChange={setRestoring}
        title="Restore Envryn Backup"
        footer={
          <>
            <Button onClick={() => setRestoring(false)}>Cancel</Button>
            <Button variant="primary" onClick={() => setRestoreError(true)}>
              Restore
            </Button>
          </>
        }
      >
        <div className="space-y-3">
          <DetailRow label="Backup" value="envryn-backup-2026-08-18" mono />
          <Field
            label="Backup password"
            error={restoreError ? "Incorrect password. Try again." : undefined}
          >
            <Input type="password" invalid={restoreError} autoFocus />
          </Field>
        </div>
      </Modal>
    </>
  );
}
