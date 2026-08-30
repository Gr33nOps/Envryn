import * as React from "react";
import {
  AlertTriangle,
  Bot,
  Braces,
  Cloud,
  CloudCog,
  CodeXml,
  Copy,
  CreditCard,
  Database,
  Eye,
  FileLock2,
  Film,
  GitBranch,
  Globe2,
  KeyRound,
  MessageSquare,
  MoreHorizontal,
  Terminal,
  Webhook,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import type { Secret } from "@/lib/envryn-data";
import { copyValue } from "@/lib/vault-actions";
import { useRevealSecret } from "@/lib/use-vault";
import { IpcError } from "@/lib/ipc";
import { IconButton, StatusDot } from "./ui";
import { useVaultUI } from "./vault-context";

type Column = "project" | "environment" | "type" | "updated";

const typeIcons: Partial<Record<Secret["type"], LucideIcon>> = {
  "API Key": KeyRound,
  Token: KeyRound,
  Environment: Braces,
  Database,
  SSH: Terminal,
  OAuth: KeyRound,
  Webhook,
  Note: FileLock2,
  Custom: Braces,
};

const providerIcons: Array<{ terms: string[]; icon: LucideIcon }> = [
  {
    terms: ["postgres", "mysql", "mariadb", "mongo", "redis", "database", "supabase"],
    icon: Database,
  },
  { terms: ["aws", "azure", "google cloud", "cloudflare", "digitalocean"], icon: Cloud },
  { terms: ["vercel", "netlify", "render", "railway", "cli"], icon: CloudCog },
  { terms: ["github", "gitlab", "bitbucket"], icon: GitBranch },
  {
    terms: ["openai", "anthropic", "groq", "perplexity", "hugging face", "replicate", "ai"],
    icon: Bot,
  },
  { terms: ["stripe", "paypal", "payment"], icon: CreditCard },
  { terms: ["slack", "discord", "telegram", "twilio"], icon: MessageSquare },
  { terms: ["igdb", "tmdb", "movie", "game"], icon: Film },
  { terms: ["api", "web", "http"], icon: Globe2 },
  { terms: ["environment", "env", "config"], icon: CodeXml },
];

function iconForSecret(secret: Pick<Secret, "name" | "provider" | "type">): LucideIcon {
  const searchable = `${secret.provider ?? ""} ${secret.name}`.toLowerCase();
  const words = searchable.split(/[^a-z0-9]+/).filter(Boolean);
  return (
    providerIcons.find(({ terms }) =>
      terms.some((term) => (term.length <= 3 ? words.includes(term) : searchable.includes(term))),
    )?.icon ??
    typeIcons[secret.type] ??
    Braces
  );
}

function environmentTone(environment: Secret["environment"]) {
  if (environment === "Production") return "warning" as const;
  if (environment === "Staging") return "syncing" as const;
  if (environment === "Development") return "neutral" as const;
  return "neutral" as const;
}

function SecretListCell({ column, secret }: Readonly<{ column: Column; secret: Secret }>) {
  if (column === "environment" && secret.environment !== "—") {
    return (
      <span className="inline-flex items-center gap-1.5">
        <StatusDot tone={environmentTone(secret.environment)} />
        {secret.environment}
      </span>
    );
  }
  if (column === "environment") {
    return <span className="text-subtle-foreground">No environment</span>;
  }
  if (column === "type") {
    return (
      <span className="rounded-full border border-border bg-background/40 px-2 py-0.5 text-[10px]">
        {secret.type}
      </span>
    );
  }
  return <>{secret[column]}</>;
}

export function SecretList({
  items,
  columns = ["project", "environment", "type", "updated"],
}: Readonly<{
  items: Secret[];
  columns?: Column[];
}>) {
  const { selected, select, openEdit } = useVaultUI();
  const revealSecret = useRevealSecret();

  // A list row carries no secret material, so copying fetches the value on
  // demand rather than reading it from the row.
  const copySecret = React.useCallback(
    async (secret: Secret) => {
      try {
        await copyValue(await revealSecret.mutateAsync(secret.id));
      } catch (err) {
        toast(err instanceof IpcError ? err.message : "That secret could not be copied.");
      }
    },
    [revealSecret],
  );
  const [context, setContext] = React.useState<{ secret: Secret; x: number; y: number } | null>(
    null,
  );
  const head: Record<Column, string> = {
    project: "Project",
    environment: "Environment",
    type: "Type",
    updated: "Updated",
  };
  const gridCols = `minmax(0,1.8fr) ${columns.map(() => "minmax(0,1fr)").join(" ")} 92px`;

  React.useEffect(() => {
    const close = () => setContext(null);
    window.addEventListener("click", close);
    window.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("scroll", close, true);
    };
  }, []);

  return (
    <div className="secret-list overflow-hidden">
      <div
        className="secret-list-header grid items-center gap-3 border-b border-border bg-background/45 px-3.5 py-2 text-[10px] font-semibold uppercase tracking-[0.1em] text-subtle-foreground"
        style={{ gridTemplateColumns: gridCols }}
      >
        <div>Name</div>
        {columns.map((column) => (
          <div key={column} className={cn(column === "updated" && "text-right")}>
            {head[column]}
          </div>
        ))}
        <div />
      </div>
      <ul>
        {items.map((secret) => {
          const active = selected?.id === secret.id;
          const Icon = iconForSecret(secret);
          return (
            <li key={secret.id}>
              <div
                title="Select to view details"
                onClick={() => {
                  setContext(null);
                  select(active ? null : secret);
                }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  // A coordinate-positioned desktop menu is easy to clip on
                  // a touch screen. Opening the detail surface is the useful
                  // mobile equivalent of a long press.
                  const compactViewport =
                    typeof window.matchMedia === "function" &&
                    window.matchMedia("(max-width: 767px)").matches;
                  if (compactViewport) {
                    setContext(null);
                    select(secret);
                    return;
                  }
                  setContext({ secret, x: event.clientX, y: event.clientY });
                }}
                className={cn(
                  "secret-row group relative grid min-h-[64px] cursor-pointer items-center gap-3 border-b border-border/55 px-3.5 text-[12.5px] transition-colors last:border-0",
                  active ? "bg-primary-muted/75" : "hover:bg-surface-2/60",
                )}
                style={{ gridTemplateColumns: gridCols }}
              >
                {active && <span className="absolute inset-y-0 left-0 w-0.5 bg-primary" />}
                <button
                  type="button"
                  aria-label={`Open ${secret.name} details`}
                  onClick={(event) => {
                    event.stopPropagation();
                    setContext(null);
                    select(active ? null : secret);
                  }}
                  className="flex min-w-0 items-center gap-2.5 text-left"
                >
                  <span className="inline-flex size-7 shrink-0 items-center justify-center rounded-md border border-border bg-surface-2 text-muted-foreground">
                    <Icon className="size-3.5" />
                  </span>
                  <span className="min-w-0">
                    <span
                      className={cn(
                        "block truncate text-[12.5px] font-medium",
                        secret.type === "Environment" || secret.name === secret.name.toUpperCase()
                          ? "font-mono text-[11.5px]"
                          : "",
                      )}
                    >
                      {secret.name}
                    </span>
                    <span className="mt-0.5 block truncate text-[11px] text-subtle-foreground">
                      {secret.provider ?? secret.type}
                      {secret.tags?.length ? ` · ${secret.tags.join(" · ")}` : ""}
                    </span>
                  </span>
                  {secret.damaged && (
                    <span title="Needs review" aria-label="Needs review">
                      <AlertTriangle className="size-3.5 shrink-0 text-warning" />
                    </span>
                  )}
                </button>
                {columns.map((column) => (
                  <div
                    key={column}
                    data-column={column}
                    className={cn(
                      "secret-column truncate text-[12px] text-muted-foreground",
                      column === "updated" && "text-right",
                      column === "environment" &&
                        secret.environment === "Production" &&
                        "text-foreground",
                    )}
                  >
                    <SecretListCell column={column} secret={secret} />
                  </div>
                ))}
                <div
                  className={cn(
                    "secret-row-actions flex items-center justify-end gap-0.5 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100",
                    active ? "opacity-100" : "opacity-0",
                  )}
                >
                  <IconButton
                    label="Copy secret"
                    onClick={(event) => {
                      event.stopPropagation();
                      void copySecret(secret);
                    }}
                  >
                    <Copy />
                  </IconButton>
                  <IconButton
                    label="Reveal details"
                    onClick={(event) => {
                      event.stopPropagation();
                      select(secret);
                    }}
                  >
                    <Eye />
                  </IconButton>
                  <IconButton
                    label="More actions"
                    onClick={(event) => {
                      event.stopPropagation();
                      toast("More actions are ready to connect");
                    }}
                  >
                    <MoreHorizontal />
                  </IconButton>
                </div>
              </div>
            </li>
          );
        })}
      </ul>
      {context && (
        <div
          role="menu"
          tabIndex={-1}
          className="fixed z-[60] w-[150px] rounded-md border border-border bg-surface p-1 shadow-[0_12px_28px_-12px_rgba(0,0,0,0.9)]"
          style={{ left: context.x, top: context.y }}
          onClick={(event) => event.stopPropagation()}
          onKeyDown={(event) => {
            if (event.key === "Escape") setContext(null);
          }}
        >
          <button
            type="button"
            className="context-action"
            onClick={() => {
              select(context.secret);
              setContext(null);
            }}
          >
            Reveal details
          </button>
          <button
            type="button"
            className="context-action"
            onClick={() => {
              if (context) void copySecret(context.secret);
              setContext(null);
            }}
          >
            Copy value
          </button>
          <button
            type="button"
            className="context-action"
            onClick={() => {
              openEdit(context.secret);
              setContext(null);
            }}
          >
            Edit secret
          </button>
        </div>
      )}
    </div>
  );
}
