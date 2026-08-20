import * as React from "react";
import { X, Copy, Eye, EyeOff, Pencil, ShieldCheck, AlertTriangle } from "lucide-react";
import { toast } from "sonner";
import type { Secret } from "@/lib/envryn-data";
import { Button, ConfirmDialog, DetailRow, Modal, IconButton } from "./ui";
import { copySecret } from "./SecretList";
import { useVaultUI } from "./vault-context";

const REVEAL_SECONDS = 20;

export function SecretPanel({ secret }: { secret: Secret }) {
  const { select, openEdit } = useVaultUI();
  const [revealed, setRevealed] = React.useState(false);
  const [confirming, setConfirming] = React.useState(false);
  const [deleting, setDeleting] = React.useState(false);
  const [left, setLeft] = React.useState(REVEAL_SECONDS);

  React.useEffect(() => {
    setRevealed(false);
    setConfirming(false);
  }, [secret.id]);

  React.useEffect(() => {
    if (!revealed) return;
    setLeft(REVEAL_SECONDS);
    const t = setInterval(
      () => setLeft((n) => (n <= 1 ? (setRevealed(false), REVEAL_SECONDS) : n - 1)),
      1000,
    );
    return () => clearInterval(t);
  }, [revealed]);

  return (
    <aside className="flex w-[320px] shrink-0 flex-col border-l border-border bg-surface xl:w-[344px]">
      <div className="flex h-9 items-center justify-between gap-2 border-b border-border px-3">
        <span className="truncate font-mono text-[12.5px] font-medium">
          {secret.name}
        </span>
        <IconButton label="Close (Esc)" onClick={() => select(null)}>
          <X />
        </IconButton>
      </div>

      <div className="flex-1 space-y-3.5 overflow-y-auto px-3.5 py-3.5">
        <DetailRow label="Type" value={secret.type} />
        <div className="grid grid-cols-2 gap-3">
          <DetailRow label="Project" value={secret.project} />
          <DetailRow label="Environment" value={secret.environment} />
        </div>
        {secret.provider && <DetailRow label="Provider" value={secret.provider} />}

        <div>
          <div className="text-[10.5px] font-medium uppercase tracking-[0.08em] text-subtle-foreground">
            Value
          </div>
          {secret.damaged ? (
            <div className="mt-1 rounded-md border border-warning/35 bg-warning/8 px-2.5 py-2">
              <p className="flex items-center gap-1.5 text-[12px] text-warning">
                <AlertTriangle className="size-3.5" />
                This secret couldn't be opened.
              </p>
              <p className="mt-0.5 text-[11.5px] text-muted-foreground">
                The stored data may be damaged.
              </p>
            </div>
          ) : (
            <>
              <div className="mt-1 min-h-[30px] break-all rounded-md border border-input bg-background px-2.5 py-1.5 font-mono text-[12px] leading-relaxed">
                {revealed ? secret.value : "••••••••••••••••••••••••••••"}
              </div>
              <div className="mt-2 flex items-center gap-2">
                <Button onClick={copySecret}>
                  <Copy />
                  Copy
                </Button>
                {revealed ? (
                  <Button onClick={() => setRevealed(false)}>
                    <EyeOff />
                    Hide
                  </Button>
                ) : (
                  <Button onClick={() => setConfirming(true)}>
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
          Delete Secret
        </Button>
      </div>

      <Modal
        open={confirming}
        onOpenChange={setConfirming}
        title="Confirm your identity"
        description="Authentication is required before a secret value is shown."
        footer={
          <>
            <Button onClick={() => setConfirming(false)}>Cancel</Button>
            <Button
              variant="primary"
              onClick={() => {
                setConfirming(false);
                setRevealed(true);
              }}
            >
              <ShieldCheck />
              Use Windows Hello
            </Button>
          </>
        }
      />

      <ConfirmDialog
        open={deleting}
        onOpenChange={setDeleting}
        title={`Delete ${secret.name}?`}
        body="This secret will be permanently removed from this vault. This can't be undone."
        confirmLabel="Delete Secret"
        onConfirm={() => {
          select(null);
          toast("Secret deleted");
        }}
      />
    </aside>
  );
}
