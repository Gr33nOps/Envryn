import * as React from "react";
import { Check, CircleAlert, RefreshCw, TriangleAlert } from "lucide-react";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import * as ipc from "@/lib/ipc";
import { useDevices } from "@/lib/use-vault";
import { Button, DetailRow, Panel, StatusLabel } from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/sync")({ component: Sync });

type Outcome = "ok" | "failed";

function deviceStatusText(outcome: Outcome | undefined, online: boolean, lastSync: string): string {
  if (outcome === "ok") return "Synced just now";
  if (outcome === "failed") return "Could not reach this device";
  if (online) return "Seen on this network";
  return `Last sync ${lastSync}`;
}

function DeviceStatusLabel({
  outcome,
  syncing,
  online,
}: Readonly<{ outcome: Outcome | undefined; syncing: boolean; online: boolean }>) {
  if (outcome === "failed") return <StatusLabel tone="danger">Failed</StatusLabel>;
  if (syncing) return <StatusLabel tone="syncing">Syncing</StatusLabel>;
  if (online) return <StatusLabel tone="success">Online</StatusLabel>;
  return <StatusLabel tone="neutral">Offline</StatusLabel>;
}

function Sync() {
  const devicesQuery = useDevices();
  const devices = devicesQuery.data ?? [];
  const [syncing, setSyncing] = React.useState(false);
  const [peers, setPeers] = React.useState<ipc.DiscoveredPeer[]>([]);
  const [outcomes, setOutcomes] = React.useState<Record<string, Outcome>>({});
  const [lastSyncAt, setLastSyncAt] = React.useState<Date | null>(null);
  const [conflicts, setConflicts] = React.useState<ipc.ConflictSummary[]>([]);
  const [resolving, setResolving] = React.useState<string | null>(null);

  const refreshConflicts = React.useCallback(async () => {
    if (!ipc.isTauri()) return;
    try {
      setConflicts(await ipc.conflictListAll());
    } catch {
      // Non-fatal: the sync/device list above is the primary content of
      // this page even if conflicts can't be loaded for some reason.
    }
  }, []);

  React.useEffect(() => {
    void refreshConflicts();
  }, [refreshConflicts]);

  async function recover(conflictId: string) {
    setResolving(conflictId);
    try {
      await ipc.conflictRecover(conflictId);
      toast("Kept as a new, separate secret.");
      await refreshConflicts();
    } catch (err) {
      toast(err instanceof ipc.IpcError ? err.message : "Could not recover this conflict.");
    } finally {
      setResolving(null);
    }
  }

  async function discard(conflictId: string) {
    setResolving(conflictId);
    try {
      await ipc.conflictDiscard(conflictId);
      toast("Discarded.");
      await refreshConflicts();
    } catch (err) {
      toast(err instanceof ipc.IpcError ? err.message : "Could not discard this conflict.");
    } finally {
      setResolving(null);
    }
  }

  // Listening lets a peer reach *this* device too, not only the other way
  // around. Tied to this page's lifetime rather than the whole app's --
  // `envryn_core::vault::Vault::trusted_fingerprints`'s own doc comment
  // already commits sync to "something the user starts from inside the
  // unlocked app," not a background service.
  React.useEffect(() => {
    if (!ipc.isTauri()) return;
    void ipc.syncListenStart().catch(() => {
      // Non-fatal: this device can still sync out even if it can't accept
      // incoming connections (e.g. the port could not be opened).
    });
    return () => void ipc.syncListenStop();
  }, []);

  const refreshPeers = React.useCallback(async (): Promise<ipc.DiscoveredPeer[]> => {
    if (!ipc.isTauri()) return [];
    try {
      const found = await ipc.discoveryBrowse();
      setPeers(found);
      return found;
    } catch {
      setPeers([]);
      return [];
    }
  }, []);

  React.useEffect(() => {
    void refreshPeers();
  }, [refreshPeers]);

  function peerFor(deviceId: string) {
    return peers.find((p) => p.device_id === deviceId);
  }

  async function syncNow() {
    setSyncing(true);
    try {
      const found = await refreshPeers();
      const results: Record<string, Outcome> = {};
      let totalApplied = 0;
      let totalConflicts = 0;
      let attempted = 0;

      for (const device of devices) {
        const peer = found.find((p) => p.device_id === device.deviceId);
        const address = peer?.addresses[0];
        if (!peer || !address) continue;
        attempted += 1;
        try {
          const summary = await ipc.syncNow(address, peer.port);
          results[device.id] = "ok";
          totalApplied += summary.records_applied;
          totalConflicts += summary.conflicts;
        } catch {
          results[device.id] = "failed";
        }
      }

      setOutcomes(results);
      setLastSyncAt(new Date());
      await refreshConflicts();
      if (attempted === 0) {
        toast("No trusted devices found on this network");
      } else if (totalConflicts > 0) {
        toast(
          `Sync complete — ${totalConflicts} conflicting edit${totalConflicts === 1 ? "" : "s"} found. Review below.`,
        );
      } else {
        toast(`Sync complete — ${totalApplied} record${totalApplied === 1 ? "" : "s"} updated`);
      }
    } catch (err) {
      toast(err instanceof ipc.IpcError ? err.message : "Sync could not complete.");
    } finally {
      setSyncing(false);
    }
  }

  const anyFailed = Object.values(outcomes).includes("failed");

  return (
    <div className="min-h-full bg-background">
      <div className="content-wrap content-wrap--narrow">
        <header className="page-hero">
          <div>
            <p className="breadcrumb">
              Vault <span>/</span> Sync
            </p>
            <h1 className="mt-3 text-[22px] font-semibold tracking-[-0.035em]">Sync</h1>
            <p className="mt-1.5 max-w-[54ch] text-[12.5px] text-muted-foreground">
              Keep approved devices up to date over your local network.
            </p>
          </div>
          <Button variant="primary" size="lg" loading={syncing} onClick={() => void syncNow()}>
            <RefreshCw />
            Sync now
          </Button>
        </header>

        <Panel className="mb-4 p-4">
          <div className="grid gap-4 sm:grid-cols-2">
            <DetailRow label="What is synced" value="Encrypted secret values" />
            <DetailRow
              label="Last sync this session"
              value={lastSyncAt ? lastSyncAt.toLocaleTimeString() : "Not yet"}
            />
          </div>
        </Panel>

        <div className="mb-3 flex items-center justify-between">
          <span className="text-[12.5px] font-medium">Trusted devices</span>
          <span className="text-[11px] text-subtle-foreground">
            {devices.length} device{devices.length === 1 ? "" : "s"} · {peers.length} seen on this
            network
          </span>
        </div>

        <Panel>
          {devices.length ? (
            devices.map((device) => {
              const online = Boolean(peerFor(device.deviceId));
              const outcome = outcomes[device.id];
              return (
                <div
                  key={device.id}
                  className="flex items-center gap-3 border-b border-border/60 px-4 py-3.5 last:border-0"
                >
                  <span className="inline-flex size-8 items-center justify-center rounded-md border border-border bg-surface-2 text-muted-foreground">
                    <RefreshCw className="size-3.5" />
                  </span>
                  <div className="min-w-0 flex-1">
                    <p className="text-[12.5px] font-medium">{device.name}</p>
                    <p className="mt-1 text-[11px] text-muted-foreground">
                      {deviceStatusText(outcome, online, device.lastSync)}
                    </p>
                  </div>
                  <DeviceStatusLabel outcome={outcome} syncing={syncing} online={online} />
                </div>
              );
            })
          ) : (
            <div className="px-4 py-8 text-center text-[12px] text-subtle-foreground">
              No trusted devices yet. Pair one from the Devices page.
            </div>
          )}
        </Panel>

        {conflicts.length > 0 && (
          <>
            <div className="mb-3 mt-6 flex items-center justify-between">
              <span className="text-[12.5px] font-medium">Conflicts</span>
              <span className="text-[11px] text-subtle-foreground">
                {conflicts.length} pending review
              </span>
            </div>
            <p className="mb-3 text-[11.5px] leading-relaxed text-muted-foreground">
              Two devices edited the same secret before they synced. The most recent edit is what
              you see everywhere else; the other edit is kept here until you decide.
            </p>
            <Panel>
              {conflicts.map((c) => (
                <div
                  key={c.conflict_id}
                  className="flex items-center gap-3 border-b border-border/60 px-4 py-3.5 last:border-0"
                >
                  <span className="inline-flex size-8 items-center justify-center rounded-md border border-warning/40 bg-warning/10 text-warning">
                    <TriangleAlert className="size-3.5" />
                  </span>
                  <div className="min-w-0 flex-1">
                    <p className="text-[12.5px] font-medium">{c.record.name}</p>
                    <p className="mt-1 text-[11px] text-muted-foreground">
                      An older edit to this secret, replaced by a newer one from another device.
                    </p>
                  </div>
                  <Button
                    size="sm"
                    variant="danger"
                    loading={resolving === c.conflict_id}
                    onClick={() => void discard(c.conflict_id)}
                  >
                    Discard
                  </Button>
                  <Button
                    size="sm"
                    variant="primary"
                    loading={resolving === c.conflict_id}
                    onClick={() => void recover(c.conflict_id)}
                  >
                    Keep as new secret
                  </Button>
                </div>
              ))}
            </Panel>
          </>
        )}

        {lastSyncAt && !anyFailed && !syncing && (
          <p className="mt-3 flex items-center gap-1.5 text-[12px] text-success">
            <Check className="size-3.5" />
            Everything is up to date.
          </p>
        )}
        {anyFailed && !syncing && (
          <div className="mt-3 rounded-md border border-destructive/35 bg-destructive-muted px-3 py-3">
            <p className="flex items-center gap-1.5 text-[12.5px] text-destructive">
              <CircleAlert className="size-3.5" />
              Sync could not complete for one or more devices.
            </p>
            <p className="mt-1 text-[11.5px] leading-relaxed text-muted-foreground">
              The device did not respond on the local network. Check that it is awake and connected,
              then try again.
            </p>
            <Button className="mt-2" size="sm" onClick={() => void syncNow()}>
              Retry
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
