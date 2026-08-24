import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Check, Copy, Laptop, Pencil, Plus, Smartphone, X } from "lucide-react";
import { toast } from "sonner";
import { devices, type Device } from "@/lib/envryn-data";
import {
  Button,
  ConfirmDialog,
  DetailRow,
  Field,
  IconButton,
  Input,
  Modal,
  StatusLabel,
} from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/devices")({
  component: TrustedDevices,
});

function deviceIcon(name: string) {
  return name.toLowerCase().includes("android") ? Smartphone : Laptop;
}

function statusTone(status: Device["status"]) {
  return status === "Trusted"
    ? ("success" as const)
    : status === "Syncing"
      ? ("syncing" as const)
      : ("neutral" as const);
}

function TrustedDevices() {
  const [detail, setDetail] = React.useState<Device | null>(null);
  const [pairing, setPairing] = React.useState(false);
  const [stage, setStage] = React.useState<"waiting" | "found" | "expired">("waiting");
  const [revoking, setRevoking] = React.useState<Device | null>(null);
  const [renaming, setRenaming] = React.useState(false);
  const [name, setName] = React.useState("");
  const [deviceNames, setDeviceNames] = React.useState<Record<string, string>>({});

  function displayName(device: Device) {
    return deviceNames[device.id] ?? device.name;
  }

  function openDetails(device: Device) {
    setDetail(device);
    setName(displayName(device));
    setRenaming(false);
  }

  function copyFingerprint() {
    if (detail) navigator.clipboard?.writeText(detail.fingerprint);
    toast("Fingerprint copied");
  }

  return (
    <div className="min-h-full bg-background">
      <div className="content-wrap content-wrap--narrow">
        <header className="page-hero">
          <div>
            <p className="breadcrumb">
              Vault <span>/</span> Devices
            </p>
            <h1 className="mt-3 text-[22px] font-semibold tracking-[-0.035em]">Trusted devices</h1>
            <p className="mt-1.5 text-[12.5px] text-muted-foreground">
              Only devices you approve can connect to this vault.
            </p>
          </div>
          <Button
            variant="primary"
            size="lg"
            onClick={() => {
              setStage("waiting");
              setPairing(true);
            }}
          >
            <Plus />
            Pair a device
          </Button>
        </header>

        <div className="mb-3 flex items-center justify-between border-y border-border/70 py-2.5 text-[11.5px] text-muted-foreground">
          <span>
            {devices.length} approved devices ·{" "}
            {devices.filter((device) => device.status !== "Offline").length} online
          </span>
          <span>Review access regularly</span>
        </div>

        <div className={detail ? "grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]" : "grid gap-4"}>
          <div className="overflow-hidden rounded-lg border border-border bg-surface">
            {devices.map((device) => {
              const Icon = deviceIcon(displayName(device));
              const selected = detail?.id === device.id;
              return (
                <button
                  type="button"
                  key={device.id}
                  onClick={() => openDetails(device)}
                  className={`device-row group flex w-full items-center gap-3 border-b border-border/60 px-4 py-3.5 text-left transition-colors last:border-0 ${selected ? "bg-surface-3 shadow-[inset_2px_0_0_var(--primary)]" : "hover:bg-surface-2/65"}`}
                >
                  <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-surface-2 text-muted-foreground">
                    <Icon className="size-4" />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[12.5px] font-medium text-foreground">
                      {displayName(device)}
                    </span>
                    <span className="mt-1 flex items-center gap-2 text-[11px] text-muted-foreground">
                      <StatusLabel tone={statusTone(device.status)}>{device.status}</StatusLabel>
                      <span>Last seen {device.lastSync}</span>
                    </span>
                  </span>
                  <span className="hidden text-right md:block">
                    <span className="block font-mono text-[10.5px] text-muted-foreground">
                      {device.fingerprint.slice(0, 14)}...
                    </span>
                    <span className="mt-1 block text-[10px] text-subtle-foreground">
                      {device.deviceId}
                    </span>
                  </span>
                  <span className="shrink-0 text-[11px] text-subtle-foreground transition-colors group-hover:text-foreground">
                    Details
                  </span>
                </button>
              );
            })}
          </div>

          {detail ? (
            <aside className="flex min-h-[390px] flex-col rounded-lg border border-border bg-surface">
              <div className="flex items-center justify-between border-b border-border px-4 py-3">
                <div className="flex items-center gap-2.5">
                  <span className="inline-flex size-7 items-center justify-center rounded-md border border-border bg-surface-2 text-muted-foreground">
                    {React.createElement(deviceIcon(displayName(detail)), {
                      className: "size-3.5",
                    })}
                  </span>
                  <span className="text-[12.5px] font-medium">{displayName(detail)}</span>
                </div>
                <IconButton label="Close details" onClick={() => setDetail(null)}>
                  <X />
                </IconButton>
              </div>
              <div className="flex-1 space-y-4 overflow-y-auto px-4 py-4">
                {renaming ? (
                  <Field label="Device name">
                    <div className="flex gap-2">
                      <Input
                        value={name}
                        autoFocus
                        onChange={(event) => setName(event.target.value)}
                      />
                      <Button
                        variant="primary"
                        onClick={() => {
                          const nextName = name.trim() || detail.name;
                          setDeviceNames((current) => ({ ...current, [detail.id]: nextName }));
                          setDetail({ ...detail, name: nextName });
                          setRenaming(false);
                          toast("Device name updated");
                        }}
                      >
                        Save
                      </Button>
                    </div>
                  </Field>
                ) : (
                  <div className="flex items-center justify-between">
                    <DetailRow label="Device" value={detail.name} />
                    <Button size="sm" onClick={() => setRenaming(true)}>
                      <Pencil />
                      Rename
                    </Button>
                  </div>
                )}
                <div className="grid grid-cols-2 gap-4">
                  <DetailRow
                    label="Status"
                    value={
                      <StatusLabel tone={statusTone(detail.status)}>{detail.status}</StatusLabel>
                    }
                  />
                  <DetailRow label="Last seen" value={detail.lastSync} />
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <DetailRow label="Added" value={detail.added} />
                  <DetailRow label="Device ID" value={detail.deviceId} mono />
                </div>
                <div>
                  <div className="mb-1 flex items-center justify-between">
                    <span className="text-[10.5px] font-medium uppercase tracking-[0.08em] text-subtle-foreground">
                      Full fingerprint
                    </span>
                    <Button size="sm" onClick={copyFingerprint}>
                      <Copy />
                      Copy
                    </Button>
                  </div>
                  <div className="break-all rounded-md border border-input bg-background px-2.5 py-2 font-mono text-[11px] leading-relaxed text-muted-foreground">
                    {detail.fingerprint}
                  </div>
                </div>
              </div>
              <div className="border-t border-border px-4 py-3">
                <Button variant="danger" size="block" onClick={() => setRevoking(detail)}>
                  Revoke device
                </Button>
              </div>
            </aside>
          ) : null}
        </div>
      </div>

      <Modal
        open={pairing}
        onOpenChange={setPairing}
        title="Pair a device"
        description="Open Envryn on the other device and scan this code."
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
                <Check />
                Trust device
              </Button>
            </>
          ) : stage === "expired" ? (
            <>
              <Button onClick={() => setPairing(false)}>Cancel</Button>
              <Button variant="primary" onClick={() => setStage("waiting")}>
                Generate new code
              </Button>
            </>
          ) : (
            <>
              <Button onClick={() => setStage("expired")}>Cancel</Button>
              <Button variant="primary" onClick={() => setStage("found")}>
                I found the device
              </Button>
            </>
          )
        }
      >
        {stage === "expired" ? (
          <div className="py-4 text-center">
            <p className="text-[12.5px]">This pairing code expired.</p>
            <p className="mt-1 text-[11.5px] text-muted-foreground">
              Generate a new code to continue.
            </p>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-3">
            <div className="grid size-[132px] grid-cols-8 gap-px rounded border border-border bg-background p-2">
              {Array.from({ length: 64 }).map((_, index) => (
                <span
                  key={index}
                  className={
                    [
                      0, 1, 2, 3, 8, 10, 16, 18, 21, 27, 33, 36, 40, 45, 52, 58, 61, 63, 5, 6, 7,
                      13, 15, 23, 30, 44, 49,
                    ].includes(index)
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
              <div className="mt-0.5 font-mono text-[16px] tracking-[0.2em]">481 927</div>
            </div>
            {stage === "found" ? (
              <p className="max-w-[34ch] text-center text-[12px] text-muted-foreground">
                Android Phone wants to connect. Make sure this code matches the other device.
              </p>
            ) : (
              <p className="text-[11.5px] text-muted-foreground">Waiting for the other device...</p>
            )}
          </div>
        )}
      </Modal>

      <ConfirmDialog
        open={Boolean(revoking)}
        onOpenChange={(open) => !open && setRevoking(null)}
        title={`Revoke ${revoking?.name}?`}
        body="This device will stop syncing with the vault. You will need to pair it again to reconnect."
        confirmLabel="Revoke device"
        onConfirm={() => {
          setDetail(null);
          setRevoking(null);
          toast("Device access removed");
        }}
      />
    </div>
  );
}
