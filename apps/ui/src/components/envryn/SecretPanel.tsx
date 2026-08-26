import * as React from "react";
import { X, Copy, Eye, EyeOff, Pencil, AlertTriangle } from "lucide-react";
import { toast } from "sonner";
import type { Secret } from "@/lib/envryn-data";
import { Button, ConfirmDialog, DetailRow, IconButton } from "./ui";
import { copyValue } from "@/lib/vault-actions";
import { IpcError } from "@/lib/ipc";
import { useDeleteSecret, useRevealSecret } from "@/lib/use-vault";
import { useVaultUI } from "./vault-context";

const REVEAL_SECONDS = 20;

export function SecretPanel({ secret }: Readonly<{ secret: Secret }>) {
  const { select, openEdit } = useVaultUI();
  const revealSecret = useRevealSecret();
  const deleteSecret = useDeleteSecret();

  /**
   * The plaintext, held only while it is on screen.
   *
   * Fetched on demand rather than arriving with the list, because a list
   * carries no secret material by design. Cleared whenever the panel hides the
   * value, changes record, or unmounts.
   */
  const [value, setValue] = React.useState<string | null>(null);
  const [deleting, setDeleting] = React.useState(false);
  const [left, setLeft] = React.useState(REVEAL_SECONDS);

  const revealed = value !== null;

  const hide = React.useCallback(() => setValue(null), []);

  React.useEffect(() => {
    setValue(null);
  }, [secret.id]);

  // Drop the plaintext when the panel goes away.
  React.useEffect(() => () => setValue(null), []);

  React.useEffect(() => {
    if (!revealed) return;
    setLeft(REVEAL_SECONDS);
    const t = setInterval(() => {
      setLeft((n) => {
        if (n <= 1) {
          hide();
          return REVEAL_SECONDS;
        }
        return n - 1;
      });
    }, 1000);
    return () => clearInterval(t);
  }, [hide, revealed]);

  async function reveal() {
    try {
      setValue(await revealSecret.mutateAsync(secret.id));
    } catch (err) {
      toast(err instanceof IpcError ? err.message : "That secret could not be opened.");
    }
  }

  async function copy() {
    try {
      // Fetch fresh rather than requiring a reveal first: copying without
      // displaying is the more private path, so it should not be the harder one.
      await copyValue(await revealSecret.mutateAsync(secret.id));
    } catch (err) {
      toast(err instanceof IpcError ? err.message : "That secret could not be copied.");
    }
  }

  return (
    <aside className="flex w-[320px] shrink-0 flex-col border-l border-border bg-surface xl:w-[344px]">
      <div className="flex h-9 items-center justify-between gap-2 border-b border-border px-3">
        <span className="truncate font-mono text-[12.5px] font-medium">{secret.name}</span>
        <IconButton label="Close (Esc)" onClick={() => select(null)}>
          <X />
        </IconButton>
      </div>

      <div className="flex-1 space-y-3.5 overflow-y-auto px-3.5 py-3.5">
        <DetailRow label="Type" value={secret.type} />
        <div className="grid grid-cols-2 gap-3">
          <DetailRow label="Project" value={secret.project} />
          <DetailRow
            label="Environment"
            value={secret.environment === "—" ? "No environment" : secret.environment}
          />
        </div>
        {secret.provider && <DetailRow label="Provider" value={secret.provider} />}

        <div>
          <div className="flex items-center justify-between gap-3">
            <div className="text-[10.5px] font-medium uppercase tracking-[0.08em] text-subtle-foreground">
              Value
            </div>
            {!secret.damaged && (
              <span className="text-[10.5px] text-subtle-foreground">Hidden until revealed</span>
            )}
          </div>
          {secret.damaged ? (
            <div className="mt-1 rounded-md border border-warning/35 bg-warning/8 px-2.5 py-2">
              <p className="flex items-center gap-1.5 text-[12px] text-warning">
                <AlertTriangle className="size-3.5" />
                This secret couldn't be opened.
              </p>
              <p className="mt-0.5 text-[11.5px] text-muted-foreground">
                The stored data may be damaged. Restore from a backup or replace this secret.
              </p>
            </div>
          ) : (
            <>
              <div className="mt-1 min-h-[30px] break-all rounded-md border border-input bg-background px-2.5 py-1.5 font-mono text-[12px] leading-relaxed">
                {value ?? "••••••••••••••••••••••••••••"}
              </div>
              <div className="mt-2 flex items-center gap-2">
                <Button onClick={() => void copy()}>
                  <Copy />
                  Copy
                </Button>
                {revealed ? (
                  <Button onClick={hide}>
                    <EyeOff />
                    Hide
                  </Button>
                ) : (
                  <Button onClick={() => void reveal()}>
                    <Eye />
                    Reveal
                  </Button>
                )}
                {revealed && (
                  <span className="ml-auto text-[11px] text-subtle-foreground">
                    Hides in {left}s
                  </span>
                )}
              </div>
            </>
          )}
        </div>

        {secret.notes && <DetailRow label="Notes" value={secret.notes} />}
        {secret.tags?.length ? (
          <DetailRow
            label="Tags"
            value={
              <span className="flex flex-wrap gap-1">
                {secret.tags.map((t) => (
                  <span
                    key={t}
                    className="rounded border border-border bg-surface-2 px-1.5 py-px text-[11px] text-muted-foreground"
                  >
                    {t}
                  </span>
                ))}
              </span>
            }
          />
        ) : null}

        <div className="grid grid-cols-2 gap-3 border-t border-border pt-3.5">
          <DetailRow label="Created" value={secret.created} />
          <DetailRow label="Last updated" value={secret.updated} />
        </div>

        <div className="pt-1">
          <Button size="block" onClick={() => openEdit(secret)}>
            <Pencil />
            Edit
          </Button>
        </div>
      </div>

      <div className="border-t border-border px-3.5 py-2.5">
        <Button variant="danger" size="block" onClick={() => setDeleting(true)}>
          Delete secret
        </Button>
      </div>

      {/* The re-authentication prompt that used to sit here claimed Windows
          Hello was verifying the user; it was not wired to anything. Reveal
          gating is M4 and will use the real platform authenticator -- until
          then, showing a security step that does nothing is worse than
          showing none. */}

      <ConfirmDialog
        open={deleting}
        onOpenChange={setDeleting}
        title={`Delete ${secret.name}?`}
        body="This secret will be permanently removed from this vault. This can't be undone."
        confirmLabel="Delete secret"
        onConfirm={() => {
          void (async () => {
            try {
              await deleteSecret.mutateAsync(secret.id);
              select(null);
              toast("Secret deleted");
            } catch (err) {
              toast(err instanceof IpcError ? err.message : "That secret could not be deleted.");
            }
          })();
        }}
      />
    </aside>
  );
}
