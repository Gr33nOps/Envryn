import * as React from "react";
import { createFileRoute, Outlet, useRouterState } from "@tanstack/react-router";
import { MobileUIContext } from "@/components/envryn/mobile/mobile-context";
import { SecretSheet } from "@/components/envryn/mobile/SecretSheet";
import { SecretFormSheet } from "@/components/envryn/mobile/SecretFormSheet";
import { SearchSheet } from "@/components/envryn/mobile/SearchSheet";
import { StatusBar, TabBar } from "@/components/envryn/mobile/Shell";
import type { Secret } from "@/lib/envryn-data";

export const Route = createFileRoute("/m")({
  component: MobileShell,
});

function MobileShell() {
  const path = useRouterState({ select: (s) => s.location.pathname });
  const locked = path === "/m" || path === "/m/";

  const [selected, setSelected] = React.useState<Secret | null>(null);
  const [formOpen, setFormOpen] = React.useState(false);
  const [editing, setEditing] = React.useState<Secret | null>(null);
  const [preset, setPreset] = React.useState<Partial<Secret> | undefined>();
  const [searchOpen, setSearchOpen] = React.useState(false);

  const ctx = React.useMemo(
    () => ({
      selected,
      select: setSelected,
      openAdd: (p?: Partial<Secret>) => {
        setEditing(null);
        setPreset(p);
        setFormOpen(true);
      },
      openEdit: (s: Secret) => {
        setSelected(null);
        setEditing(s);
        setFormOpen(true);
      },
      openSearch: () => setSearchOpen(true),
    }),
    [selected],
  );

  return (
    <MobileUIContext.Provider value={ctx}>
      <div className="flex h-screen justify-center overflow-hidden bg-background">
        <div className="flex h-full w-full max-w-[430px] flex-col overflow-hidden border-x border-border bg-background">
          <StatusBar />
          <main className="min-h-0 flex-1 overflow-y-auto overscroll-contain">
            <Outlet />
          </main>
          {!locked && <TabBar />}
        </div>
      </div>

      <SecretSheet secret={selected} />
      <SecretFormSheet
        open={formOpen}
        onOpenChange={setFormOpen}
        secret={editing}
        preset={preset}
      />
      <SearchSheet
        open={searchOpen}
        onOpenChange={setSearchOpen}
        onSelect={setSelected}
      />
    </MobileUIContext.Provider>
  );
}
