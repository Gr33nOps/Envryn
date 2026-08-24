import * as React from "react";
import { createFileRoute, Outlet, useNavigate } from "@tanstack/react-router";
import { Search } from "lucide-react";
import { toast } from "sonner";
import { Sidebar } from "@/components/envryn/Sidebar";
import { SecretPanel } from "@/components/envryn/SecretPanel";
import { SecretFormModal } from "@/components/envryn/SecretForm";
import { SearchPalette } from "@/components/envryn/SearchPalette";
import { VaultUIContext } from "@/components/envryn/vault-context";
import { secrets, type Secret } from "@/lib/envryn-data";
import { copySecret } from "@/lib/vault-actions";

export const Route = createFileRoute("/vault")({
  component: VaultLayout,
});

function TopBar({ onSearch }: { onSearch: () => void }) {
  return (
    <header className="titlebar flex h-[50px] shrink-0 items-center gap-3 border-b border-border bg-background px-4">
      <button
        type="button"
        className="topbar-search group flex min-w-0 flex-1 items-center gap-2 rounded-md border border-border bg-surface px-2.5 text-left transition-colors hover:border-border-strong hover:bg-surface-2 md:max-w-[420px]"
        onClick={onSearch}
      >
        <Search className="size-3.5 text-subtle-foreground" />
        <span className="flex-1 truncate text-[11.5px] text-muted-foreground">
          Search everywhere
        </span>
        <span className="kbd">Ctrl K</span>
      </button>
    </header>
  );
}

function VaultLayout() {
  const navigate = useNavigate();
  const [selected, setSelected] = React.useState<Secret | null>(null);
  const [formOpen, setFormOpen] = React.useState(false);
  const [editing, setEditing] = React.useState<Secret | null>(null);
  const [preset, setPreset] = React.useState<Partial<Secret> | undefined>();
  const [searchOpen, setSearchOpen] = React.useState(false);

  const lockVault = React.useCallback(() => {
    toast("Vault locked", { description: "Your local workspace is ready to secure." });
    navigate({ to: "/" });
  }, [navigate]);

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
        setEditing(s);
        setFormOpen(true);
      },
      openSearch: () => setSearchOpen(true),
    }),
    [selected],
  );

  React.useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const key = event.key.toLowerCase();
      const target = event.target as HTMLElement | null;
      if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
      if (event.ctrlKey && key === "k") {
        event.preventDefault();
        setSearchOpen(true);
      } else if (event.ctrlKey && key === "n") {
        event.preventDefault();
        ctx.openAdd();
      } else if (event.ctrlKey && key === "c" && selected) {
        event.preventDefault();
        void copySecret(selected);
      } else if (selected && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
        event.preventDefault();
        const index = secrets.findIndex((secret) => secret.id === selected.id);
        const next =
          event.key === "ArrowDown"
            ? Math.min(secrets.length - 1, index + 1)
            : Math.max(0, index - 1);
        setSelected(secrets[next] ?? selected);
      } else if (event.ctrlKey && key === "l") {
        event.preventDefault();
        lockVault();
      } else if (event.key === "Escape") {
        setSelected(null);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [ctx, lockVault, selected]);

  return (
    <VaultUIContext.Provider value={ctx}>
      <div className="app-frame flex h-screen flex-col overflow-hidden bg-background">
        <div className="flex min-h-0 flex-1">
          <Sidebar onLock={lockVault} />
          <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
            <TopBar onSearch={() => setSearchOpen(true)} />
            <div className="min-h-0 flex-1 overflow-y-auto">
              <Outlet />
            </div>
          </main>
          {selected && <SecretPanel secret={selected} />}
        </div>
      </div>
      <SecretFormModal
        open={formOpen}
        onOpenChange={setFormOpen}
        secret={editing}
        preset={preset}
      />
      <SearchPalette open={searchOpen} onOpenChange={setSearchOpen} onSelect={setSelected} />
    </VaultUIContext.Provider>
  );
}
