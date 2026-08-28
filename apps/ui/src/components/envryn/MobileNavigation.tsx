import * as React from "react";
import { Link, useRouterState } from "@tanstack/react-router";
import {
  Archive,
  Database,
  FileLock2,
  FolderClosed,
  KeyRound,
  Layers,
  Lock,
  Menu,
  MonitorSmartphone,
  Plus,
  RefreshCw,
  Settings,
  Terminal,
} from "lucide-react";
import {
  Drawer,
  DrawerClose,
  DrawerContent,
  DrawerDescription,
  DrawerTitle,
} from "@/components/ui/drawer";
import { cn } from "@/lib/utils";
import { useVaultUI } from "./vault-context";

const primaryItems = [
  { to: "/vault", label: "Secrets", icon: Layers, exact: true },
  { to: "/vault/projects", label: "Projects", icon: FolderClosed },
  { to: "/vault/devices", label: "Devices", icon: MonitorSmartphone },
] as const;

const moreItems = [
  { categoryId: "api-tokens", label: "API & tokens", icon: KeyRound },
  { categoryId: "databases", label: "Databases", icon: Database },
  { categoryId: "ssh", label: "SSH keys", icon: Terminal },
  { categoryId: "notes", label: "Secure notes", icon: FileLock2 },
  { to: "/vault/sync", label: "Sync", icon: RefreshCw },
  { to: "/vault/backup", label: "Backup", icon: Archive },
  { to: "/vault/settings", label: "Settings", icon: Settings },
] as const;

function MobileNavLink({
  item,
  active,
}: Readonly<{
  item: (typeof primaryItems)[number];
  active: boolean;
}>) {
  return (
    <Link
      to={item.to}
      aria-current={active ? "page" : undefined}
      className={cn("mobile-nav-item", active && "mobile-nav-item--active")}
    >
      <item.icon />
      <span>{item.label}</span>
    </Link>
  );
}

export function MobileNavigation({ onLock }: Readonly<{ onLock: () => void }>) {
  const path = useRouterState({ select: (state) => state.location.pathname });
  const { openAdd } = useVaultUI();
  const [moreOpen, setMoreOpen] = React.useState(false);

  const activeFor = (item: (typeof primaryItems)[number]) =>
    "exact" in item && item.exact
      ? path === item.to || path === `${item.to}/`
      : path.startsWith(item.to);

  return (
    <>
      <nav className="mobile-bottom-nav" aria-label="Primary navigation">
        <MobileNavLink item={primaryItems[0]} active={activeFor(primaryItems[0])} />
        <MobileNavLink item={primaryItems[1]} active={activeFor(primaryItems[1])} />
        <button type="button" className="mobile-add-action" onClick={() => openAdd()}>
          <span>
            <Plus />
          </span>
          <span>Add</span>
        </button>
        <MobileNavLink item={primaryItems[2]} active={activeFor(primaryItems[2])} />
        <button
          type="button"
          className={cn("mobile-nav-item", moreOpen && "mobile-nav-item--active")}
          onClick={() => setMoreOpen(true)}
        >
          <Menu />
          <span>More</span>
        </button>
      </nav>

      <Drawer open={moreOpen} onOpenChange={setMoreOpen} shouldScaleBackground={false}>
        <DrawerContent className="mobile-more-drawer">
          <div className="sr-only">
            <DrawerTitle>More vault options</DrawerTitle>
            <DrawerDescription>Browse categories, sync, backup, and settings.</DrawerDescription>
          </div>
          <div className="mobile-drawer-handle" />
          <div className="px-5 pb-2 pt-4">
            <p className="text-[18px] font-semibold tracking-[-0.025em]">Your vault</p>
            <p className="mt-1 text-[13px] text-muted-foreground">Categories and vault controls</p>
          </div>
          <div className="grid grid-cols-2 gap-2 px-3 py-3">
            {moreItems.map((item) => (
              <DrawerClose asChild key={"to" in item ? item.to : item.categoryId}>
                {"to" in item ? (
                  <Link to={item.to} className="mobile-more-item">
                    <span>
                      <item.icon />
                    </span>
                    {item.label}
                  </Link>
                ) : (
                  <Link
                    to="/vault/category/$categoryId"
                    params={{ categoryId: item.categoryId }}
                    className="mobile-more-item"
                  >
                    <span>
                      <item.icon />
                    </span>
                    {item.label}
                  </Link>
                )}
              </DrawerClose>
            ))}
            <DrawerClose asChild>
              <button type="button" className="mobile-more-item text-destructive" onClick={onLock}>
                <span>
                  <Lock />
                </span>
                Lock vault
              </button>
            </DrawerClose>
          </div>
          <div className="h-[max(12px,env(safe-area-inset-bottom))]" />
        </DrawerContent>
      </Drawer>
    </>
  );
}
