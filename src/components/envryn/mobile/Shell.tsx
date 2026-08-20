import * as React from "react";
import { Link, useRouterState } from "@tanstack/react-router";
import { ChevronLeft, FolderClosed, Layers, RefreshCw, Settings } from "lucide-react";
import { cn } from "@/lib/utils";

/** Status bar mock for a native-feeling phone frame. */
export function StatusBar() {
  return (
    <div className="flex h-7 shrink-0 items-center justify-between px-5 text-[11px] font-medium text-muted-foreground">
      <span>9:41</span>
      <div className="flex items-center gap-1.5">
        <span className="tracking-tight">5G</span>
        <span className="inline-block h-2.5 w-5 rounded-[3px] border border-border-strong p-[1.5px]">
          <span className="block h-full w-3/4 rounded-[1px] bg-success" />
        </span>
      </div>
    </div>
  );
}

export function MobileHeader({
  title,
  subtitle,
  back,
  right,
}: {
  title: string;
  subtitle?: string;
  back?: string;
  right?: React.ReactNode;
}) {
  return (
    <header className="sticky top-0 z-20 grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-border bg-surface/95 px-3 py-2.5 backdrop-blur">
      <div className="flex min-w-0 items-center gap-1.5">
        {back && (
          <Link
            to={back}
            aria-label="Back"
            className="-ml-1 grid size-8 shrink-0 place-items-center rounded-lg text-muted-foreground active:bg-surface-2"
          >
            <ChevronLeft className="size-5" />
          </Link>
        )}
        <div className="min-w-0">
          <h1 className="truncate text-[16px] font-semibold tracking-[-0.015em]">
            {title}
          </h1>
          {subtitle && (
            <p className="truncate text-[11.5px] text-muted-foreground">{subtitle}</p>
          )}
        </div>
      </div>
      {right && <div className="flex shrink-0 items-center gap-1">{right}</div>}
    </header>
  );
}

export function HeaderIcon({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick?: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      aria-label={label}
      onClick={onClick}
      className="grid size-9 place-items-center rounded-lg text-muted-foreground active:bg-surface-2 [&_svg]:size-[18px]"
    >
      {children}
    </button>
  );
}

const tabs = [
  { to: "/m/vault", label: "Vault", icon: Layers },
  { to: "/m/projects", label: "Projects", icon: FolderClosed },
  { to: "/m/sync", label: "Sync", icon: RefreshCw },
  { to: "/m/settings", label: "Settings", icon: Settings },
];

export function TabBar() {
  const path = useRouterState({ select: (s) => s.location.pathname });
  return (
    <nav className="shrink-0 border-t border-border bg-surface pb-2 pt-1.5">
      <ul className="grid grid-cols-4">
        {tabs.map((t) => {
          const active = path.startsWith(t.to);
          return (
            <li key={t.to}>
              <Link
                to={t.to}
                className={cn(
                  "flex flex-col items-center gap-1 py-1 text-[10.5px] font-medium transition-colors",
                  active ? "text-primary" : "text-subtle-foreground",
                )}
              >
                <t.icon className="size-[19px]" />
                {t.label}
              </Link>
            </li>
          );
        })}
      </ul>
      <div className="mx-auto mt-1.5 h-1 w-28 rounded-full bg-surface-3" />
    </nav>
  );
}
