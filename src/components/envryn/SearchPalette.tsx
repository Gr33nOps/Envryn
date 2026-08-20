import * as React from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { Search } from "lucide-react";
import { secrets, type Secret } from "@/lib/envryn-data";
import { cn } from "@/lib/utils";

export function SearchPalette({
  open,
  onOpenChange,
  onSelect,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  onSelect: (s: Secret) => void;
}) {
  const [q, setQ] = React.useState("");
  const [cursor, setCursor] = React.useState(0);

  React.useEffect(() => {
    if (open) {
      setQ("");
      setCursor(0);
    }
  }, [open]);

  const results = React.useMemo(() => {
    const t = q.trim().toLowerCase();
    if (!t) return secrets.slice(0, 6);
    return secrets.filter((s) =>
      [s.name, s.project, s.environment, s.type, s.provider ?? "", ...(s.tags ?? [])]
        .join(" ")
        .toLowerCase()
        .includes(t),
    );
  }, [q]);

  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/55" />
        <DialogPrimitive.Content className="fixed left-1/2 top-[18%] z-50 w-full max-w-[520px] -translate-x-1/2 overflow-hidden rounded-lg border border-border bg-surface shadow-[0_16px_48px_-12px_rgba(0,0,0,0.6)]">
          <DialogPrimitive.Title className="sr-only">Search</DialogPrimitive.Title>
          <div className="flex items-center gap-2 border-b border-border px-3">
            <Search className="size-3.5 text-subtle-foreground" />
            <input
              autoFocus
              value={q}
              onChange={(e) => {
                setQ(e.target.value);
                setCursor(0);
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown")
                  setCursor((c) => Math.min(c + 1, results.length - 1));
                if (e.key === "ArrowUp") setCursor((c) => Math.max(c - 1, 0));
                if (e.key === "Enter" && results[cursor]) {
                  onSelect(results[cursor]);
                  onOpenChange(false);
                }
              }}
              placeholder="Search secrets, projects, tags..."
              className="h-9 w-full bg-transparent text-[13px] placeholder:text-subtle-foreground focus:outline-none"
            />
            <span className="kbd">Esc</span>
          </div>

          {results.length === 0 ? (
            <div className="px-4 py-8 text-center">
              <p className="text-[12.5px]">No results for "{q}"</p>
              <p className="mt-1 text-[11.5px] text-muted-foreground">
                Try another name, project, or tag.
              </p>
            </div>
          ) : (
            <ul className="max-h-[300px] overflow-y-auto p-1">
              {results.map((s, i) => (
                <li key={s.id}>
                  <button
                    onMouseEnter={() => setCursor(i)}
                    onClick={() => {
                      onSelect(s);
                      onOpenChange(false);
                    }}
                    className={cn(
                      "flex h-[38px] w-full items-center gap-3 rounded-md px-2.5 text-left",
                      i === cursor && "bg-surface-2",
                    )}
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate font-mono text-[12px]">{s.name}</div>
                      <div className="truncate text-[11px] text-subtle-foreground">
                        {s.project} · {s.environment}
                      </div>
                    </div>
                    <span className="text-[11.5px] text-muted-foreground">
                      {s.type}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
