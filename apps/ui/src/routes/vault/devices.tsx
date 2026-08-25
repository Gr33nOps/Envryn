import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Check, Copy, Laptop, Pencil, Plus, Smartphone, X } from "lucide-react";
import { toast } from "sonner";
import { type Device } from "@/lib/envryn-data";
import * as ipc from "@/lib/ipc";
import { useDevices, useRenameDevice, useRevokeDevice } from "@/lib/use-vault";
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

/**
 * Manual-code pairing, driven by real IPC. This device always plays the
 * "host" role here (it listens; the other device dials in) -- the
 * complementary "join" role (a brand-new install pairing *into* an existing
 * vault) lives on the first-run screen instead (`apps/ui/src/routes/index.tsx`),
 * since a device with no vault yet cannot reach this page at all.
 */
type PairingStage = "waiting" | "found" | "confirming" | "error";

function usePairingSession(onPaired: () => void) {
  const [open, setOpen] = React.useState(false);
  const [stage, setStage] = React.useState<PairingStage>("waiting");
  const [host, setHost] = React.useState<ipc.PairingHostStarted | null>(null);
  const [sas, setSas] = React.useState<ipc.PairingSasReady | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [password, setPassword] = React.useState("");
  const unlistenRef = React.useRef<(() => void) | null>(null);

  const stop = React.useCallback(() => {
    unlistenRef.current?.();
    unlistenRef.current = null;
  }, []);

  React.useEffect(() => () => stop(), [stop]);

  const start = React.useCallback(async () => {
    setOpen(true);
    setStage("waiting");
    setHost(null);
    setSas(null);
    setError(null);
    setPassword("");
    stop();
    try {
      unlistenRef.current = await ipc.listenPairingEvents({
        onSasReady: (event) => {
          setSas(event);
          setStage("found");
        },
        onFailed: (event) => {
          setError(event.message);
          setStage("error");
        },
        onComplete: () => {
          setOpen(false);
          toast("Device paired");
          onPaired();
        },
      });
      const info = await ipc.pairingHostStart(true);
      setHost(info);
    } catch (err) {
      setError(err instanceof ipc.IpcError ? err.message : "Could not start pairing.");
      setStage("error");
    }
  }, [onPaired, stop]);

  const cancel = React.useCallback(() => {
    stop();
    void ipc.pairingCancel();
    setOpen(false);
  }, [stop]);

  const confirm = React.useCallback(async () => {
    setStage("confirming");
    try {
      await ipc.pairingConfirm(password);
      // Outcome arrives as a pairing://complete or pairing://failed event.
    } catch (err) {
      setError(err instanceof ipc.IpcError ? err.message : "Could not confirm pairing.");
      setStage("error");
    }
  }, [password]);

  return { open, stage, host, sas, error, password, setPassword, start, cancel, confirm };
}

function TrustedDevices() {
  const devicesQuery = useDevices();
  const devices = devicesQuery.data ?? [];
  const renameDevice = useRenameDevice();
  const revokeDevice = useRevokeDevice();

  const [detail, setDetail] = React.useState<Device | null>(null);
  const [revoking, setRevoking] = React.useState<Device | null>(null);
  const [renaming, setRenaming] = React.useState(false);
  const [name, setName] = React.useState("");

  const pairing = usePairingSession(() => void devicesQuery.refetch());

  function openDetails(device: Device) {
    setDetail(device);
    setName(device.name);
    setRenaming(false);
  }

  // The list refetches after a rename/revoke; keep the open detail panel in
  // sync with whichever row it currently points at, and close it if that
  // device was just revoked.
  React.useEffect(() => {
    if (!detail) return;
    const fresh = devices.find((d) => d.id === detail.id);
    if (fresh && fresh !== detail) setDetail(fresh);
    else if (!fresh && devicesQuery.isFetched) setDetail(null);
  }, [devices, detail, devicesQuery.isFetched]);

  function copyFingerprint() {
    if (detail) navigator.clipboard?.writeText(detail.fingerprint);
    toast("Fingerprint copied");
  }

  async function saveRename() {
    if (!detail) return;
    const nextName = name.trim() || detail.name;
    try {
      await renameDevice.mutateAsync({ deviceId: detail.deviceId, name: nextName });
      setRenaming(false);
      toast("Device name updated");
    } catch (err) {
      toast(err instanceof ipc.IpcError ? err.message : "Could not rename that device.");
    }
  }

  async function confirmRevoke() {
    if (!revoking) return;
    try {
      await revokeDevice.mutateAsync(revoking.deviceId);
      setDetail(null);
      setRevoking(null);
      toast("Device access removed");
    } catch (err) {
      toast(err instanceof ipc.IpcError ? err.message : "Could not revoke that device.");
    }
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
          <Button variant="primary" size="lg" onClick={() => void pairing.start()}>
            <Plus />
            Pair a device
          </Button>
        </header>

        <div className="mb-3 flex items-center justify-between border-y border-border/70 py-2.5 text-[11.5px] text-muted-foreground">
          <span>
            {devices.length} approved device{devices.length === 1 ? "" : "s"}
          </span>
          <span>Review access regularly</span>
        </div>

        <div className={detail ? "grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]" : "grid gap-4"}>
          <div className="overflow-hidden rounded-lg border border-border bg-surface">
            {devices.length === 0 ? (
              <div className="px-4 py-8 text-center text-[12px] text-subtle-foreground">
                No devices paired yet. Pair one to start syncing.
              </div>
            ) : (
              devices.map((device) => {
                const Icon = deviceIcon(device.name);
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
                        {device.name}
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
              })
            )}
          </div>

          {detail ? (
            <aside className="flex min-h-[390px] flex-col rounded-lg border border-border bg-surface">
              <div className="flex items-center justify-between border-b border-border px-4 py-3">
                <div className="flex items-center gap-2.5">
                  <span className="inline-flex size-7 items-center justify-center rounded-md border border-border bg-surface-2 text-muted-foreground">
                    {React.createElement(deviceIcon(detail.name), {
                      className: "size-3.5",
                    })}
                  </span>
                  <span className="text-[12.5px] font-medium">{detail.name}</span>
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
                        loading={renameDevice.isPending}
                        onClick={() => void saveRename()}
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
        open={pairing.open}
        onOpenChange={(open) => !open && pairing.cancel()}
        title="Pair a device"
        description="Enter this code and address on the other device."
        footer={
          pairing.stage === "found" || pairing.stage === "confirming" ? (
            <>
              <Button onClick={pairing.cancel}>Cancel</Button>
              <Button
                variant="primary"
                loading={pairing.stage === "confirming"}
                disabled={pairing.password.length < 8}
                onClick={() => void pairing.confirm()}
              >
                <Check />
                Trust device
              </Button>
            </>
          ) : pairing.stage === "error" ? (
            <>
              <Button onClick={pairing.cancel}>Close</Button>
              <Button variant="primary" onClick={() => void pairing.start()}>
                Try again
              </Button>
            </>
          ) : (
            <Button onClick={pairing.cancel}>Cancel</Button>
          )
        }
      >
        {pairing.stage === "error" ? (
          <div className="py-4 text-center">
            <p className="text-[12.5px]">Pairing didn't complete.</p>
            <p className="mt-1 text-[11.5px] text-muted-foreground">
              {pairing.error ?? "Something went wrong."}
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
              <div className="mt-0.5 font-mono text-[16px] tracking-[0.2em]">
                {pairing.host?.code ?? "······"}
              </div>
              {pairing.host ? (
                <div className="mt-1 font-mono text-[11px] text-muted-foreground">
                  {pairing.host.address}:{pairing.host.port}
                </div>
              ) : null}
            </div>
            {pairing.stage === "found" || pairing.stage === "confirming" ? (
              <div className="w-full space-y-2.5">
                <p className="max-w-[34ch] text-center text-[12px] text-muted-foreground">
                  {pairing.sas?.peer_device_id} wants to connect. Make sure this code matches the
                  other device: <span className="font-mono">{pairing.sas?.sas}</span>
                </p>
                <Field label="Your current master password">
                  <Input
                    type="password"
                    autoFocus
                    value={pairing.password}
                    onChange={(event) => pairing.setPassword(event.target.value)}
                  />
                </Field>
              </div>
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
        onConfirm={() => void confirmRevoke()}
      />
    </div>
  );
}
