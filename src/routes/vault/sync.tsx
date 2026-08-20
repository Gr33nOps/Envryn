import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { RefreshCw, Check } from "lucide-react";
import { toast } from "sonner";
import {
  Button,
  PageHeader,
  Panel,
  StatusLabel,
  Tabs,
} from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/sync")({
  component: Sync;
});

type State = "idle" | "syncing" | "done" | "offline" | "failed";

function Sync() {
  const [state, setState] = React.useState<State>("idle");

  return (
    <>
      <PageHeader
        title="Sync"
        subtitle="Your devices communicate directly over your local network."
        actions={
          <Button
            variant="primary"
            loading={state === "syncing"}
            onClick={() => {
              setState("syncing");
              setTimeout(() => {
                setState("done");
                toast("Sync complete");
              }, 900);
            }}
          >
            <RefreshCw />
            Sync Now
          </Button>
        }
      />

      <div className="space-y-3 px-5 pb-5">
        <Tabs
          variant="segmented"
          items={[
            { value: "idle", label: "Connected" },
            { value: "offline", label: "Offline" },
            { value: "failed", label: "Failed" },
          ]}
          value={state === "syncing" || state === "done" ? "idle" : state}
          onChange={(v) => setState(v as State)}
        />

        <Panel>
          <div className="flex h-[46px] items-center gap-4 border-b border-border/60 px-3">
            <div className="min-w-0 flex-1">
              <div className="text-[12.5px]">Android Phone</div>
              <div className="mt-0.5 flex items-center gap-2">
                {state === "offline" ? (
                  <StatusLabel tone="neutral">Offline</StatusLabel>
                ) : state === "failed" ? (
                  <StatusLabel tone="danger">Sync failed</StatusLabel>
                ) : state === "syncing" ? (
                  <StatusLabel tone="syncing">Syncing</StatusLabel>
                ) : (
                  <StatusLabel tone="success">Connected</StatusLabel>
                )}
                <span className="text-[11px] text-subtle-foreground">
                  · Last sync:{" "}
                  {state === "offline" ? "Yesterday" : "2 minutes ago"}
                </span>
              </div>
            </div>
          </div>
          <div className="flex h-[46px] items-center gap-4 px-3">
            <div className="min-w-0 flex-1">
              <div className="text-[12.5px]">Development Laptop</div>
              <div className="mt-0.5">
                <StatusLabel tone="neutral">Offline · Last sync: Yesterday</StatusLabel>
              </div>
            </div>
          </div>
        </Panel>

        {state === "done" && (
          <p className="flex items-center gap-1.5 text-[12px] text-success">
            <Check className="size-3.5" />
            Everything is up to date
          </p>
        )}

        {state === "failed" && (
          <div className="rounded-md border border-destructive/35 bg-destructive-muted px-3 py-2.5">
            <p className="text-[12.5px] text-destructive">Sync couldn't complete.</p>
            <p className="mt-0.5 text-[11.5px] text-muted-foreground">
              Try again when both devices are on the same local network.
            </p>
            <Button className="mt-2" size="sm" onClick={() => setState("idle")}>
              Retry
            </Button>
          </div>
        )}
      </div>
    </>
  );
}
