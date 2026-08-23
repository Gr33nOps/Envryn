import { Copy, Eye, MoreHorizontal, AlertTriangle } from "lucide-react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import type { Secret } from "@/lib/envryn-data";
import { IconButton } from "./ui";
import { useVaultUI } from "./vault-context";

export function copySecret() {
  toast("Secret copied", { description: "Clipboard clears in 30 seconds." });
}

type Column = "project" | "environment" | "type" | "updated";

/** Environment chips stay neutral — an environment is not a warning state. */
const envDot: Record<string, string> = {
  Production: "bg-foreground/70",
  Staging: "bg-muted-foreground/60",
  Development: "bg-subtle-foreground/60",
};

export function SecretList({
  items,
  columns = ["project", "environment", "type"],
  fill,
}: {
  items: Secret[];
  columns?: Column[];
  fill?: boolean;
}) {
  const { selected, select } = useVaultUI();

  const head: Record<Column, string> = {
    project: "Project",
    environment: "Environment",
    type: "Type",
    updated: "Updated",
  };

  const gridCols = `minmax(0,1.8fr) ${columns.map(() => "minmax(0,1fr)").join(" ")} 96px`;

  return (
    <div
      className={cn(
        "flex flex-col overflow-hidden rounded-lg border border-border bg-surface",
        fill && "min-h-0 flex-1",
      )}
    >
      <div
        className="grid shrink-0 items-center gap-3 border-b border-border bg-surface-2/60 px-3 py-1.5 text-[10.5px] font-medium uppercase tracking-[0.08em] text-subtle-foreground"
        style={{ gridTemplateColumns: gridCols }}
      >
        <div>Name</div>
        {columns.map((c) => (
          <div key={c} className={cn(c === "updated" && "text-right")}>
            {head[c]}
          </div>
        ))}
        <div />
      </div>

      <ul className={cn(fill && "min-h-0 flex-1 overflow-y-auto")}>
        {items.map((s) => {
          const active = selected?.id === s.id;
          return (
            <li key={s.id}>
              <div
                role="button"
                tabIndex={0}
                onClick={() => select(active ? null : s)}
                onKeyDown={(e) => e.key === "Enter" && select(s)}
                className={cn(
                  "group relative grid h-[34px] cursor-default items-center gap-3 border-b border-border/45 px-3 text-[12.5px] transition-colors last:border-0",
                  active ? "bg-surface-3" : "hover:bg-surface-2",
                )}
                style={{ gridTemplateColumns: gridCols }}
              >
                {active && (
                  <span className="absolute left-0 top-0 h-full w-[2px] bg-primary" />
                )}

                <div className="flex min-w-0 items-center gap-1.5">
                  <span
                    className={cn(
                      "truncate font-mono text-[12px] tracking-tight",
                      active
                        ? "font-medium text-foreground"
                        : "text-foreground",
                    )}
                  >
                    {s.name}
                  </span>
                  {s.damaged && (
                    <AlertTriangle className="size-3 shrink-0 text-warning" />
                  )}
                </div>

                {columns.map((c) => (
                  <div
                    key={c}
                    className={cn(
                      "truncate",
                      c === "updated" && "text-right",
                      c === "project" && "text-muted-foreground",
                      c === "environment" && "text-muted-foreground",
                      c === "type" && "text-[11.5px] text-subtle-foreground",
                    )}
                  >
                    {c === "environment" && s.environment !== "—" ? (
                      <span className="inline-flex items-center gap-1.5">
                        <span
                          className={cn(
                            "size-1.5 rounded-full",
                            envDot[s.environment] ?? "bg-subtle-foreground/60",
                          )}
                        />
                        {s.environment}
                      </span>
                    ) : (
                      s[c]
                    )}
                  </div>
                ))}

                <div className="flex items-center justify-end gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
                  <IconButton
                    label="Copy value"
                    onClick={(e) => {
                      e.stopPropagation();
                      copySecret();
                    }}
                  >
                    <Copy />
                  </IconButton>
                  <IconButton
                    label="Reveal"
                    onClick={(e) => {
                      e.stopPropagation();
                      select(s);
                    }}
                  >
                    <Eye />
                  </IconButton>
                  <IconButton label="More" onClick={(e) => e.stopPropagation()}>
                    <MoreHorizontal />
                  </IconButton>
                </div>
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
