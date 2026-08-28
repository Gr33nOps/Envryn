import * as React from "react";
import { createFileRoute, Outlet, useNavigate } from "@tanstack/react-router";
import { listen } from "@tauri-apps/api/event";
import { Search } from "lucide-react";
import { toast } from "sonner";
import { Sidebar } from "@/components/envryn/Sidebar";
import { SecretPanel } from "@/components/envryn/SecretPanel";
import { SecretFormModal } from "@/components/envryn/SecretForm";
import { SearchPalette } from "@/components/envryn/SearchPalette";
import { EnvImportModal } from "@/components/envryn/EnvImportModal";
import { StructuredExtractModal } from "@/components/envryn/StructuredExtractModal";
import { MobileNavigation } from "@/components/envryn/MobileNavigation";
import { Wordmark } from "@/components/envryn/Logo";
import { VaultUIContext } from "@/components/envryn/vault-context";
import { type Secret } from "@/lib/envryn-data";
import { useClearVaultCache, useRevealSecret, useSecretList } from "@/lib/use-vault";
import { copyValue, forgetClipboardTimer } from "@/lib/vault-actions";
import { isTauri, settingsGet, syncListenStart, syncListenStop, vaultLock } from "@/lib/ipc";

export const Route = createFileRoute("/vault")({
  component: VaultLayout,
});

function TopBar({ onSearch }: Readonly<{ onSearch: () => void }>) {
  return (
    <header className="vault-topbar titlebar flex h-[50px] shrink-0 items-center gap-3 border-b border-border bg-background px-4">
      <Wordmark className="mobile-wordmark" size={25} />
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
  const secrets = useSecretList();
  const [selected, setSelected] = React.useState<Secret | null>(null);
  const [formOpen, setFormOpen] = React.useState(false);
  const [editing, setEditing] = React.useState<Secret | null>(null);
  const [preset, setPreset] = React.useState<Partial<Secret> | undefined>();
  const [searchOpen, setSearchOpen] = React.useState(false);
  const [importOpen, setImportOpen] = React.useState(false);
  const [extractOpen, setExtractOpen] = React.useState(false);

  const clearVaultCache = useClearVaultCache();
  const revealSecret = useRevealSecret();

  // Stay reachable for the whole unlocked vault session. Previously the
  // listener and mDNS advertisement existed only while the Sync page was
  // mounted, which meant a paired device disappeared as soon as its user
  // viewed any other page. The backend also stops the listener itself as
  // soon as the vault locks; this cleanup handles normal navigation/unmount.
  React.useEffect(() => {
    if (!isTauri()) return;
    void syncListenStart().catch(() => {
      // Outbound sync remains usable if this device cannot bind a listener.
      // The Sync page will report the peer as offline if neither direction
      // can establish a connection.
    });
    return () => void syncListenStop();
  }, []);

  // The client-side half of locking: drop cached records, forget any pending
  // clipboard state, and leave the screen. Shared between a manual lock and
  // the backend's own idle auto-lock, which has already done the Rust-side
  // half (zeroizing keys) by the time either path reaches this.
  const finishLocking = React.useCallback(
    (message: string) => {
      clearVaultCache();
      forgetClipboardTimer();
      setSelected(null);
      toast(message);
      navigate({ to: "/" });
    },
    [clearVaultCache, navigate],
  );

  const lockVault = React.useCallback(() => {
    // Order matters: the Rust core zeroizes its keys first, then the client
    // side cleans up. Cleaning up first would briefly render a vault view
    // over data the core has already discarded.
    void vaultLock();
    finishLocking("Vault locked");
  }, [finishLocking]);

  // Android has no implementation of Windows' system-wide idle timer. Keep
  // the same security promise in the mobile shell by measuring interaction
  // inside Envryn, and lock immediately whenever the activity is backgrounded
  // or the phone screen turns off (both surface as document.hidden).
  React.useEffect(() => {
    if (!isTauri() || !/Android/i.test(navigator.userAgent)) return;

    let disposed = false;
    let timer: number | undefined;
    const events: Array<keyof WindowEventMap> = ["pointerdown", "keydown", "input", "scroll"];

    const start = async () => {
      const settings = await settingsGet().catch(() => ({ auto_lock_minutes: 5 }));
      if (disposed) return;
      const delay = Math.max(1, settings.auto_lock_minutes) * 60_000;
      const arm = () => {
        window.clearTimeout(timer);
        timer = window.setTimeout(lockVault, delay);
      };
      const onVisibility = () => {
        if (document.hidden) lockVault();
        else arm();
      };

      for (const event of events) window.addEventListener(event, arm, { passive: true });
      document.addEventListener("visibilitychange", onVisibility);
      arm();

      return () => {
        for (const event of events) window.removeEventListener(event, arm);
        document.removeEventListener("visibilitychange", onVisibility);
      };
    };

    const cleanup = start();
    return () => {
      disposed = true;
      window.clearTimeout(timer);
      void cleanup.then((fn) => fn?.());
    };
  }, [lockVault]);

  // The vault can also lock itself: idle auto-lock (src-tauri/src/autolock.rs)
  // runs entirely in Rust and has no way to call back into React directly, so
  // it emits this event instead. Calling `vaultLock()` again here would be
  // harmless -- it is idempotent -- but pointless, since the backend has
  // already locked by the time the event arrives; only the client-side
  // cleanup is still owed.
  React.useEffect(() => {
    const unlisten = listen("vault-locked", () => {
      finishLocking("Vault locked after inactivity");
    }).catch(() => undefined);
    return () => {
      void unlisten.then((fn) => fn?.());
    };
  }, [finishLocking]);

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
      openImport: () => setImportOpen(true),
      openExtract: () => setExtractOpen(true),
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
        void revealSecret
          .mutateAsync(selected.id)
          .then(copyValue)
          .catch(() => toast("That secret could not be copied."));
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
  }, [ctx, lockVault, revealSecret, secrets, selected]);

  return (
    <VaultUIContext.Provider value={ctx}>
      <div className="app-frame flex h-full flex-col overflow-hidden bg-background">
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
      <MobileNavigation onLock={lockVault} />
      <SecretFormModal
        open={formOpen}
        onOpenChange={setFormOpen}
        secret={editing}
        preset={preset}
      />
      <SearchPalette open={searchOpen} onOpenChange={setSearchOpen} onSelect={setSelected} />
      <EnvImportModal open={importOpen} onOpenChange={setImportOpen} />
      <StructuredExtractModal open={extractOpen} onOpenChange={setExtractOpen} />
    </VaultUIContext.Provider>
  );
}
