import { Link, useRouterState } from "@tanstack/react-router";
import {
  Archive,
  Database,
  FileLock2,
  FolderClosed,
  KeyRound,
  Layers,
  Lock,
  MonitorSmartphone,
  RefreshCw,
  Settings,
  Terminal,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { categories, type Secret } from "@/lib/envryn-data";
import { useProjects, useSecretList } from "@/lib/use-vault";
import { cn } from "@/lib/utils";
import { Wordmark } from "./Logo";
import { SectionLabel } from "./ui";

interface Item {
  to: string;
  label: string;
  icon: LucideIcon;
  exact?: boolean;
  count?: number;
}

function navigationGroups(
  secrets: Secret[],
  projectCount: number,
): { label: string; items: Item[] }[] {
  const countForTypes = (types: string[]) =>
    secrets.filter((secret) => types.includes(secret.type)).length;

  return [
    {
      label: "Vault",
      items: [
        { to: "/vault", label: "All secrets", icon: Layers, exact: true, count: secrets.length },
        { to: "/vault/projects", label: "Projects", icon: FolderClosed, count: projectCount },
        {
          to: "/vault/category/api-tokens",
          label: "API & tokens",
          icon: KeyRound,
          count: countForTypes(categories["api-tokens"].types),
        },
        {
          to: "/vault/category/databases",
          label: "Databases",
          icon: Database,
          count: countForTypes(categories.databases.types),
        },
        {
          to: "/vault/category/ssh",
          label: "SSH",
          icon: Terminal,
          count: countForTypes(categories.ssh.types),
        },
        {
          to: "/vault/category/notes",
          label: "Secure notes",
          icon: FileLock2,
          count: countForTypes(categories.notes.types),
        },
      ],
    },
    {
      label: "Devices",
      items: [
        { to: "/vault/devices", label: "Trusted devices", icon: MonitorSmartphone },
        { to: "/vault/sync", label: "Sync", icon: RefreshCw },
      ],
    },
  ];
}

export function Sidebar({ onLock }: { onLock: () => void }) {
  const path = useRouterState({ select: (state) => state.location.pathname });
  const secrets = useSecretList();
  const projects = useProjects();
  const groups = navigationGroups(secrets, projects.length);

  return (
    <aside className="app-sidebar flex w-[220px] shrink-0 flex-col border-r border-border bg-background">
      <div className="flex h-[58px] items-center border-b border-border px-4">
        <Wordmark size={18} />
      </div>
      <div className="px-4 pt-4">
        <div className="flex items-center gap-2 border-b border-border pb-3">
          <span className="workspace-avatar">E</span>
          <div className="min-w-0">
            <p className="truncate text-[12px] font-medium text-foreground">My vault</p>
            <p className="mt-0.5 text-[10.5px] text-subtle-foreground">On this PC</p>
          </div>
        </div>
      </div>
      <nav className="flex-1 overflow-y-auto px-3 pb-3 pt-5">
        {groups.map((group) => (
          <div key={group.label} className="mb-5">
            <div className="px-2 pb-1.5">
              <SectionLabel>{group.label}</SectionLabel>
            </div>
            <ul className="space-y-0.5">
              {group.items.map((item) => {
                const active = item.exact ? path === item.to : path.startsWith(item.to);
                return (
                  <li key={item.to}>
                    <Link
                      to={item.to}
                      className={cn(
                        "group relative flex h-8 items-center gap-2.5 rounded-md px-2.5 text-[12px] transition-colors",
                        active
                          ? "bg-surface-2 font-medium text-foreground"
                          : "text-muted-foreground hover:bg-surface-2/65 hover:text-foreground",
                      )}
                    >
                      {active && (
                        <span className="absolute left-0 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-full bg-primary" />
                      )}
                      <item.icon
                        className={cn(
                          "size-3.5",
                          active
                            ? "text-primary"
                            : "text-subtle-foreground group-hover:text-foreground",
                        )}
                      />
                      <span className="flex-1 truncate">{item.label}</span>
                      {item.count !== undefined && (
                        <span className="font-mono text-[10px] text-subtle-foreground">
                          {item.count}
                        </span>
                      )}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
        <div className="border-t border-border pt-4">
          <div className="px-2 pb-1.5">
            <SectionLabel>More</SectionLabel>
          </div>
          <ul className="space-y-0.5">
            <li>
              <Link
                to="/vault/backup"
                className="flex h-8 items-center gap-2.5 rounded-md px-2.5 text-[12px] text-muted-foreground hover:bg-surface-2/65 hover:text-foreground"
              >
                <Archive className="size-3.5 text-subtle-foreground" />
                Backup
              </Link>
            </li>
            <li>
              <Link
                to="/vault/settings"
                className="flex h-8 items-center gap-2.5 rounded-md px-2.5 text-[12px] text-muted-foreground hover:bg-surface-2/65 hover:text-foreground"
              >
                <Settings className="size-3.5 text-subtle-foreground" />
                Settings
              </Link>
            </li>
          </ul>
        </div>
      </nav>
      <div className="border-t border-border px-3 py-3">
        <div className="sidebar-status mb-2 flex items-center gap-2 px-2.5 text-[11px]">
          <span className="size-1 rounded-full bg-subtle-foreground" />
          Unlocked
        </div>
        <button
          onClick={onLock}
          type="button"
          className="flex h-8 w-full items-center gap-2 rounded-md px-2.5 text-[11.5px] text-muted-foreground transition-colors hover:bg-surface-2 hover:text-foreground"
        >
          <Lock className="size-3.5 text-subtle-foreground" />
          Lock vault<span className="kbd ml-auto">Ctrl L</span>
        </button>
      </div>
    </aside>
  );
}
