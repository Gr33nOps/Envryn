import * as React from "react";
import { Check, CircleAlert, RefreshCw } from "lucide-react";
import { createFileRoute } from "@tanstack/react-router";
import { toast } from "sonner";
import { devices } from "@/lib/envryn-data";
import { Button, DetailRow, Panel, StatusLabel, Tabs } from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/sync")({ component: Sync });

type State = "connected" | "offline" | "failed";

function Sync() {
  const [state, setState] = React.useState<State>("connected");
  const [syncing, setSyncing] = React.useState(false);
  const [done, setDone] = React.useState(false);

  function syncNow() {
    setDone(false);
    setSyncing(true);
    setTimeout(() => {
      setSyncing(false);
      setDone(true);
      toast("Sync complete");
    }, 900);
  }

  const visibleDevices =
    state === "connected"
      ? devices.filter((device) => device.status !== "Offline")
      : state === "offline"
        ? devices.filter((device) => device.status === "Offline")
        : devices.filter((device) => device.name === "Work Desktop");

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
          <Button variant="primary" size="lg" loading={syncing} onClick={syncNow}>
            <RefreshCw />
            Sync now
          </Button>
        </header>

        <Panel className="mb-4 p-4">
          <div className="grid gap-4 sm:grid-cols-3">
            <DetailRow label="What is synced" value="Encrypted secret values" />
            <DetailRow label="Last successful sync" value="Today, 4:31 PM" />
            <DetailRow label="Conflicts" value="None" />
          </div>
        </Panel>

        <div className="mb-3 flex items-center justify-between">
          <Tabs
            variant="segmented"
            items={[
              { value: "connected", label: "Connected" },
              { value: "offline", label: "Offline" },
              { value: "failed", label: "Failed" },
            ]}
            value={state}
            onChange={(value) => {
              setState(value as State);
              setDone(false);
            }}
          />
          <span className="text-[11px] text-subtle-foreground">
            {visibleDevices.length} device{visibleDevices.length === 1 ? "" : "s"}
          </span>
        </div>

        <Panel>
          {visibleDevices.length ? (
            visibleDevices.map((device) => (
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
                    {state === "failed"
                      ? "Could not reach this device"
                      : `Last sync ${device.lastSync}`}
                  </p>
                </div>
                {state === "failed" ? (
                  <StatusLabel tone="danger">Failed</StatusLabel>
                ) : state === "offline" ? (
                  <StatusLabel tone="neutral">Offline</StatusLabel>
                ) : syncing ? (
                  <StatusLabel tone="syncing">Syncing</StatusLabel>
                ) : (
                  <StatusLabel tone="success">Connected</StatusLabel>
                )}
              </div>
            ))
          ) : (
            <div className="px-4 py-8 text-center text-[12px] text-subtle-foreground">
              No devices in this state.
            </div>
          )}
        </Panel>

        {done && (
          <p className="mt-3 flex items-center gap-1.5 text-[12px] text-success">
            <Check className="size-3.5" />
            Everything is up to date.
          </p>
        )}
        {state === "failed" && (
          <div className="mt-3 rounded-md border border-destructive/35 bg-destructive-muted px-3 py-3">
            <p className="flex items-center gap-1.5 text-[12.5px] text-destructive">
              <CircleAlert className="size-3.5" />
              Sync could not complete.
            </p>
            <p className="mt-1 text-[11.5px] leading-relaxed text-muted-foreground">
              The device did not respond on the local network. Check that it is awake and connected,
              then try again.
            </p>
            <Button
              className="mt-2"
              size="sm"
              onClick={() => {
                setState("connected");
                syncNow();
              }}
            >
              Retry
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
