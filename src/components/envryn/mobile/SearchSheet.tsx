import * as React from "react";
import { Search, X } from "lucide-react";
import { secrets, type Secret } from "@/lib/envryn-data";
import { Sheet, MobileInput } from "./Sheet";
import { EnvDot } from "./SecretRow";

export function SearchSheet({
  open,
  onOpenChange,
  onSelect,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  onSelect: (s: Secret) => void;
}) {
  const [q, setQ] = React.useState("");

  React.useEffect(() => {
    if (open) setQ("");
  }, [open]);

  const results = secrets.filter((s) =>
    `${s.name} ${s.project} ${s.type} ${s.tags?.join(" ") ?? ""}`
      .toLowerCase()
      .includes(q.trim().toLowerCase()),
  );

  return (
    <Sheet open={open} onOpenChange={onOpenChange} full>
      <div className="sticky top-0 -mx-4 mb-2 bg-surface px-4 pb-2 pt-1">
        <div className="relative">
          <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-subtle-foreground" />
          <MobileInput
            autoFocus
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search secrets, projects, tags…"
            className="pl-9 pr-10"
          />
          <button
            aria-label="Close search"
            onClick={() => onOpenChange(false)}
            className="absolute right-2 top-1/2 grid size-8 -translate-y-1/2 place-items-center rounded-lg text-subtle-foreground active:bg-surface-2"
          >
            <X className="size-4" />
          </button>
        </div>
      </div>

      {results.length === 0 ? (
        <div className="py-16 text-center">
          <p className="text-[13px] text-muted-foreground">No secrets found</p>
          <p className="mt-1 text-[12px] text-subtle-foreground">
            Try a different name, project or tag.
          </p>
        </div>
      ) : (
        <ul className="overflow-hidden rounded-xl border border-border">
          {results.map((s) => (
            <li key={s.id}>
              <button
                onClick={() => {
                  onOpenChange(false);
                  onSelect(s);
                }}
                className="flex w-full items-center gap-2 border-b border-border/60 bg-surface px-3 py-2.5 text-left last:border-0 active:bg-surface-2"
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate font-mono text-[12.5px]">{s.name}</div>
                  <div className="mt-0.5 flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
                    <span>{s.project}</span>
                    {s.environment !== "—" && (
                      <>
                        <EnvDot env={s.environment} />
                        <span>{s.environment}</span>
                      </>
                    )}
                  </div>
                </div>
                <span className="shrink-0 text-[11px] text-subtle-foreground">
                  {s.type}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </Sheet>
  );
}
