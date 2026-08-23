import { Link, useRouterState } from "@tanstack/react-router";
import {
  KeyRound,
  FolderClosed,
  Terminal,
  Database,
  FileLock2,
  MonitorSmartphone,
  RefreshCw,
  Archive,
  Settings,
  Lock,
  Layers,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { Wordmark } from "./Logo";
import { SectionLabel } from "./ui";

interface Item {
  to: string;
  label: string;
  icon: LucideIcon;
  exact?: boolean;
}

const groups: { label: string; items: Item[] }[] = [
  {
    label: "Vault",
    items: [
      { to: "/vault", label: "All Secrets", icon: Layers, exact: true },
      { to: "/vault/projects", label: "Projects", icon: FolderClosed },
    ],
  },
  {
    label: "Categories",
    items: [
      { to: "/vault/category/api-tokens", label: "API & Tokens", icon: KeyRound },
      { to: "/vault/category/databases", label: "Databases", icon: Database },
      { to: "/vault/category/ssh", label: "SSH", icon: Terminal },
      { to: "/vault/category/notes", label: "Secure Notes", icon: FileLock2 },
    ],
  },
  {
    label: "Devices",
    items: [
      { to: "/vault/devices", label: "Trusted Devices", icon: MonitorSmartphone },
      { to: "/vault/sync", label: "Sync", icon: RefreshCw },
    ],
  },
  {
    label: "Other",
    items: [
      { to: "/vault/backup", label: "Backup", icon: Archive },
      { to: "/vault/settings", label: "Settings", icon: Settings },
    ],
  },
];

export function Sidebar({ onLock }: { onLock: () => void }) {
  const path = useRouterState({ select: (s) => s.location.pathname });

  return (
    <aside className="flex w-[224px] shrink-0 flex-col border-r border-border bg-background lg:w-[236px]">
      <div className="flex h-11 items-center px-3.5">
        <Wordmark size={17} subtitle="local vault" />
      </div>

      <nav className="flex-1 overflow-y-auto px-2 pb-2">
        {groups.map((g) => (
          <div key={g.label} className="mb-3">
            <div className="px-2 pb-1">
              <SectionLabel>{g.label}</SectionLabel>
            </div>
            <ul className="space-y-px">
              {g.items.map((item) => {
                const active = item.exact
                  ? path === item.to
                  : path.startsWith(item.to);
                return (
                  <li key={item.to}>
                    <Link
                      to={item.to}
                      className={cn(
                        "group relative flex h-[28px] items-center gap-2 rounded-md px-2 text-[12.5px] transition-colors",
                        active
                          ? "bg-surface font-medium text-foreground"
                          : "text-muted-foreground hover:bg-surface/70 hover:text-foreground",
                      )}
                    >
                      {active && (
                        <span className="absolute left-0 top-1/2 h-4 w-[2px] -translate-y-1/2 rounded-full bg-primary" />
                      )}
                      <item.icon
                        className={cn(
                          "size-3.5",
                          active ? "text-foreground" : "text-subtle-foreground",
                        )}
                      />
                      <span className="truncate">{item.label}</span>
                    </Link>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </nav>


      <div className="border-t border-border px-3 py-2.5">
        <div className="flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
          <span className="size-1.5 rounded-full bg-success" />
          Vault unlocked
        </div>
        <button
          onClick={onLock}
          className="mt-2 flex h-6 w-full items-center gap-1.5 rounded-md px-1.5 text-[12px] text-muted-foreground transition-colors hover:bg-surface-2 hover:text-foreground"
        >
          <Lock className="size-3.5 text-subtle-foreground" />
          Lock Vault
          <span className="kbd ml-auto">Ctrl L</span>
        </button>
      </div>
    </aside>
  );
}
