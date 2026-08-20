import * as React from "react";
import {
  AlertTriangle,
  Copy,
  Eye,
  EyeOff,
  Fingerprint,
  Pencil,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import type { Secret } from "@/lib/envryn-data";
import { Sheet, TouchButton } from "./Sheet";
import { copySecretMobile, EnvDot } from "./SecretRow";
import { useMobileUI } from "./mobile-context";

const REVEAL_SECONDS = 20;

function Meta({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="min-w-0">
      <div className="text-[10.5px] font-medium uppercase tracking-[0.07em] text-subtle-foreground">
        {label}
      </div>
      <div className="mt-0.5 truncate text-[13px]">{value}</div>
    </div>
  );
}

export function SecretSheet({ secret }: { secret: Secret | null }) {
  const { select, openEdit } = useMobileUI();
  const [revealed, setRevealed] = React.useState(false);
  const [auth, setAuth] = React.useState(false);
  const [deleting, setDeleting] = React.useState(false);
  const [left, setLeft] = React.useState(REVEAL_SECONDS);

  React.useEffect(() => {
    setRevealed(false);
    setAuth(false);
  }, [secret?.id]);

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
    <>
      <Sheet
        open={!!secret}
        onOpenChange={(v) => !v && select(null)}
        title={secret?.name}
        description={secret ? `${secret.type} · ${secret.project}` : undefined}
      >
        {secret && (
          <div className="space-y-4 pt-1">
            <div className="grid grid-cols-2 gap-3 rounded-xl border border-border bg-surface-2/50 p-3">
              <Meta label="Project" value={secret.project} />
              <Meta
                label="Environment"
                value={
                  <span className="inline-flex items-center gap-1.5">
                    <EnvDot env={secret.environment} />
                    {secret.environment}
                  </span>
                }
              />
              <Meta label="Type" value={secret.type} />
              <Meta label="Provider" value={secret.provider ?? "—"} />
              <Meta label="Created" value={secret.created} />
              <Meta label="Last updated" value={secret.updated} />
            </div>

            <div>
              <div className="text-[10.5px] font-medium uppercase tracking-[0.07em] text-subtle-foreground">
                Value
              </div>
              {secret.damaged ? (
                <div className="mt-1.5 rounded-xl border border-warning/35 bg-warning/8 p-3">
                  <p className="flex items-center gap-1.5 text-[13px] text-warning">
                    <AlertTriangle className="size-4" />
                    This secret couldn&apos;t be opened.
                  </p>
                  <p className="mt-1 text-[12px] text-muted-foreground">
                    The stored data may be damaged. Restore it from a backup.
                  </p>
                </div>
              ) : (
                <>
                  <div className="mt-1.5 min-h-[46px] break-all rounded-xl border border-input bg-background px-3 py-2.5 font-mono text-[12.5px] leading-relaxed">
                    {revealed ? secret.value : "••••••••••••••••••••••••"}
                  </div>
                  {revealed && (
                    <p className="mt-1.5 text-[11.5px] text-subtle-foreground">
                      Hides automatically in {left}s
                    </p>
                  )}
                  <div className="mt-2.5 grid grid-cols-2 gap-2">
                    <TouchButton onClick={copySecretMobile}>
                      <Copy />
                      Copy
                    </TouchButton>
                    {revealed ? (
                      <TouchButton onClick={() => setRevealed(false)}>
                        <EyeOff />
                        Hide
                      </TouchButton>
                    ) : (
                      <TouchButton variant="primary" onClick={() => setAuth(true)}>
                        <Eye />
                        Reveal
                      </TouchButton>
                    )}
                  </div>
                </>
              )}
            </div>

            {secret.notes && <Meta label="Notes" value={secret.notes} />}
            {secret.tags?.length ? (
              <div className="flex flex-wrap gap-1.5">
                {secret.tags.map((t) => (
                  <span
                    key={t}
                    className="rounded-full border border-border bg-surface-2 px-2 py-0.5 text-[11.5px] text-muted-foreground"
                  >
                    {t}
                  </span>
                ))}
              </div>
            ) : null}

            <div className="grid grid-cols-2 gap-2 pt-1">
              <TouchButton onClick={() => openEdit(secret)}>
                <Pencil />
                Edit
              </TouchButton>
              <TouchButton variant="danger" onClick={() => setDeleting(true)}>
                <Trash2 />
                Delete
              </TouchButton>
            </div>
          </div>
        )}
      </Sheet>

      {/* Biometric confirmation before reveal */}
      <Sheet
        open={auth}
        onOpenChange={setAuth}
        title="Confirm your identity"
        description="Authentication is required before a secret value is shown."
        footer={
          <>
            <TouchButton onClick={() => setAuth(false)}>Cancel</TouchButton>
            <TouchButton
              variant="primary"
              onClick={() => {
                setAuth(false);
                setRevealed(true);
              }}
            >
              <Fingerprint />
              Use fingerprint
            </TouchButton>
          </>
        }
      >
        <div className="flex flex-col items-center gap-3 py-6">
          <div className="grid size-16 place-items-center rounded-full border border-primary/40 bg-primary-muted">
            <Fingerprint className="size-8 text-primary" />
          </div>
          <p className="text-center text-[12.5px] text-muted-foreground">
            Touch the sensor to unlock this value
          </p>
        </div>
      </Sheet>

      <Sheet
        open={deleting}
        onOpenChange={setDeleting}
        title={secret ? `Delete ${secret.name}?` : "Delete secret?"}
        description="This secret will be permanently removed from this device. This can't be undone."
        footer={
          <>
            <TouchButton onClick={() => setDeleting(false)}>Cancel</TouchButton>
            <TouchButton
              variant="danger"
              onClick={() => {
                setDeleting(false);
                select(null);
                toast("Secret deleted");
              }}
            >
              Delete
            </TouchButton>
          </>
        }
      />
    </>
  );
}
