import * as React from "react";
import { createFileRoute, Outlet, useNavigate } from "@tanstack/react-router";
import { toast } from "sonner";
import { Sidebar } from "@/components/envryn/Sidebar";
import { SecretPanel } from "@/components/envryn/SecretPanel";
import { SecretFormModal } from "@/components/envryn/SecretForm";
import { SearchPalette } from "@/components/envryn/SearchPalette";
import { VaultUIContext } from "@/components/envryn/vault-context";
import { LogoMark } from "@/components/envryn/Logo";
import type { Secret } from "@/lib/envryn-data";

export const Route = createFileRoute("/vault")({
  head: () => ({
    meta: [
      { title: "Vault — Envryn" },
      {
        name: "description",
        content:
          "Browse and manage API keys, tokens, database credentials and SSH secrets across your projects and environments.",
      },
      { property: "og:title", content: "Vault — Envryn" },
      {
        property: "og:description",
        content: "Manage developer secrets across projects and environments.",
      },
      { property: "og:type", content: "website" },
      { name: "twitter:card", content: "summary_large_image" },
    ],
  }),
  component: VaultLayout,
});

function VaultLayout() {
  const navigate = useNavigate();
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
        setEditing(s);
        setFormOpen(true);
      },
      openSearch: () => setSearchOpen(true),
    }),
    [selected],
  );

  React.useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const k = e.key.toLowerCase();
      if (e.ctrlKey && k === "k") {
        e.preventDefault();
        setSearchOpen(true);
      } else if (e.ctrlKey && k === "n") {
        e.preventDefault();
        ctx.openAdd();
      } else if (e.ctrlKey && k === "l") {
        e.preventDefault();
        toast("Vault locked");
        navigate({ to: "/" });
      } else if (e.key === "Escape") {
        setSelected(null);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [ctx, navigate]);

  return (
    <VaultUIContext.Provider value={ctx}>
      <div className="flex h-screen flex-col overflow-hidden bg-background">
        <div className="flex h-7 shrink-0 items-center justify-between border-b border-border bg-background px-3 text-[11.5px] text-subtle-foreground">
          <span className="flex items-center gap-1.5">
            <LogoMark size={12} />
            Envryn
          </span>
          <div className="flex items-center gap-3 text-[11px]">
            <span>—</span>
            <span>▢</span>
            <span>✕</span>
          </div>
        </div>


        <div className="flex min-h-0 flex-1">
          <Sidebar
            onLock={() => {
              toast("Vault locked");
              navigate({ to: "/" });
            }}
          />
          <main className="flex min-w-0 flex-1 flex-col overflow-y-auto">
            <Outlet />
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
      <SearchPalette
        open={searchOpen}
        onOpenChange={setSearchOpen}
        onSelect={setSelected}
      />
    </VaultUIContext.Provider>
  );
}
