import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Plus, X } from "lucide-react";
import { toast } from "sonner";
import { devices, type Device } from "@/lib/envryn-data";
import {
  Button,
  ConfirmDialog,
  DetailRow,
  IconButton,
  Modal,
  PageHeader,
  Panel,
  StatusLabel,
} from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/devices")({
  component: TrustedDevices,
});

function TrustedDevices() {
  const [detail, setDetail] = React.useState<Device | null>(null);
  const [pairing, setPairing] = React.useState(false);
  const [stage, setStage] = React.useState<"waiting" | "found" | "expired">(
    "waiting",
  );
  const [revoking, setRevoking] = React.useState<Device | null>(null);

  return (
    <div className="flex min-h-0 flex-1">
      <div className="min-w-0 flex-1">
        <PageHeader
          title="Trusted Devices"
          subtitle="Only devices you approve can sync with this vault."
          actions={
            <Button
              variant="primary"
              onClick={() => {
                setStage("waiting");
                setPairing(true);
              }}
            >
              <Plus />
              Pair Device
            </Button>
          }
        />
        <div className="px-5 pb-5">
          <Panel>
            <ul>
              {devices.map((d) => (
                <li
                  key={d.id}
                  className="group flex h-[46px] items-center gap-4 border-b border-border/60 px-3 last:border-0 hover:bg-surface-2/50"
                >
                  <div className="min-w-0 flex-1">
                    <div className="text-[12.5px]">{d.name}</div>
                    <div className="mt-0.5 flex items-center gap-2 text-[11px] text-subtle-foreground">
                      <StatusLabel
                        tone={d.status === "Trusted" ? "success" : "neutral"}
                      >
                        {d.status}
                      </StatusLabel>
                      <span>· Last synced {d.lastSync}</span>
                    </div>
                  </div>
                  <div className="hidden font-mono text-[11.5px] text-subtle-foreground md:block">
                    {d.fingerprint.slice(0, 11)}...
                  </div>
                  <Button
                    size="sm"
                    className="opacity-0 transition-opacity group-hover:opacity-100"
                    onClick={() => setDetail(d)}
                  >
                    View Details
                  </Button>
                </li>
              ))}
            </ul>
          </Panel>
        </div>
      </div>

      {detail && (
        <aside className="flex w-[300px] shrink-0 flex-col border-l border-border bg-surface">
          <div className="flex h-9 items-center justify-between border-b border-border px-3">
            <span className="text-[12.5px] font-medium">{detail.name}</span>
            <IconButton label="Close" onClick={() => setDetail(null)}>
              <X />
            </IconButton>
          </div>
          <div className="flex-1 space-y-3.5 overflow-y-auto px-3.5 py-3.5">
            <DetailRow
              label="Status"
              value={
                <StatusLabel
                  tone={detail.status === "Trusted" ? "success" : "neutral"}
                >
                  {detail.status}
                </StatusLabel>
              }
            />
            <DetailRow label="Last sync" value={`Today, 4:31 PM`} />
            <DetailRow label="Added" value={detail.added} />
            <DetailRow label="Fingerprint" value={detail.fingerprint} mono />
            <DetailRow label="Device ID" value={detail.deviceId} mono />
          </div>
          <div className="border-t border-border px-3.5 py-2.5">
            <Button
              variant="danger"
              size="block"
              onClick={() => setRevoking(detail)}
            >
              Revoke Device
            </Button>
          </div>
        </aside>
      )}

      <Modal
        open={pairing}
        onOpenChange={setPairing}
        title="Pair a Device"
        description="Scan this QR code using Envryn on your other device."
        footer={
          stage === "found" ? (
            <>
              <Button onClick={() => setPairing(false)}>Cancel</Button>
              <Button
                variant="primary"
                onClick={() => {
                  setPairing(false);
                  toast("Device paired");
                }}
              >
                Trust Device
              </Button>
            </>
          ) : stage === "expired" ? (
            <>
              <Button onClick={() => setPairing(false)}>Cancel</Button>
              <Button variant="primary" onClick={() => setStage("waiting")}>
                Generate New Code
              </Button>
            </>
          ) : (
            <>
              <Button onClick={() => setStage("expired")}>Simulate expiry</Button>
              <Button variant="primary" onClick={() => setStage("found")}>
                Simulate device found
              </Button>
            </>
          )
        }
      >
        {stage === "expired" ? (
          <div className="py-4 text-center">
            <p className="text-[12.5px]">Pairing code expired.</p>
            <p className="mt-1 text-[11.5px] text-muted-foreground">
              Generate a new code to continue.
            </p>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-3">
            <div className="grid size-[132px] grid-cols-8 gap-px rounded border border-border bg-background p-2">
              {Array.from({ length: 64 }).map((_, i) => (
                <span
                  key={i}
                  className={
                    [0, 1, 2, 3, 8, 10, 16, 18, 21, 27, 33, 36, 40, 45, 52, 58, 61, 63, 5, 6, 7, 13, 15, 23, 30, 44, 49].includes(
                      i,
                    )
                      ? "rounded-[1px] bg-foreground"
                      : ""
                  }
                />
              ))}
            </div>
            <div className="text-center">
              <div className="text-[10.5px] uppercase tracking-[0.08em] text-subtle-foreground">
                Verification code
              </div>
              <div className="mt-0.5 font-mono text-[16px] tracking-[0.2em]">
                481 927
              </div>
            </div>
            {stage === "found" ? (
              <p className="max-w-[34ch] text-center text-[12px] text-muted-foreground">
                Android Phone wants to connect. Make sure this code matches the
                other device.
              </p>
            ) : (
              <p className="flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
                <span className="size-1.5 animate-pulse rounded-full bg-primary" />
                Waiting for device...
              </p>
            )}
          </div>
        )}
      </Modal>

      <ConfirmDialog
        open={Boolean(revoking)}
        onOpenChange={(v) => !v && setRevoking(null)}
        title={`Revoke ${revoking?.name}?`}
        body="This device will no longer be allowed to sync with this vault. You will need to pair it again to reconnect."
        confirmLabel="Revoke Device"
        onConfirm={() => {
          setDetail(null);
          toast("Device revoked");
        }}
      />
    </div>
  );
}
