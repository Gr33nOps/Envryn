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

export function SecretList({
  items,
  columns = ["project", "environment", "type"],
}: {
  items: Secret[];
  columns?: Column[];
}) {
  const { selected, select } = useVaultUI();

  const head: Record<Column, string> = {
    project: "Project",
    environment: "Environment",
    type: "Type",
    updated: "Updated",
  };

  const gridCols = `minmax(0,1.6fr) ${columns.map(() => "minmax(0,1fr)").join(" ")} 92px`;

  return (
    <div className="overflow-hidden rounded-lg border border-border bg-surface/60">
      <div
        className="grid items-center gap-3 border-b border-border bg-background/60 px-3 py-1.5 text-[10.5px] font-medium uppercase tracking-[0.08em] text-subtle-foreground"
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

      <ul>
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
                  "group relative grid cursor-default items-center gap-3 border-b border-border/50 px-3 text-[12.5px] transition-colors last:border-0",
                  "h-[34px]",
                  active
                    ? "bg-primary-muted text-foreground"
                    : "hover:bg-surface-2/50",
                )}
                style={{ gridTemplateColumns: gridCols }}
              >
                {active && (
                  <span className="absolute left-0 top-0 h-full w-[2px] bg-primary" />
                )}

                <div className="flex min-w-0 items-center gap-1.5">
                  <span
                    className={cn(
                      "truncate",
                      s.type === "Environment" ||
                        s.name === s.name.toUpperCase()
                        ? "font-mono text-[12px]"
                        : "",
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
                      "truncate text-muted-foreground",
                      c === "updated" && "text-right",
                      c === "environment" &&
                        s.environment === "Production" &&
                        "text-foreground",
                    )}
                  >
                    {c === "environment" && s.environment !== "—" ? (
                      <span className="inline-flex items-center gap-1.5">
                        <span
                          className={cn(
                            "size-1.5 rounded-full",
                            s.environment === "Production"
                              ? "bg-warning"
                              : s.environment === "Staging"
                                ? "bg-primary"
                                : "bg-subtle-foreground",
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
                    label="Copy"
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
