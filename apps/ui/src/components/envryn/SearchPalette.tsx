import * as React from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { Search, SlidersHorizontal } from "lucide-react";
import { type Secret } from "@/lib/envryn-data";
import { useSecretList } from "@/lib/use-vault";
import { applySearchFilter, describeFilter, isEmptyFilter } from "@/lib/search-filter";
import { cn } from "@/lib/utils";
import * as ipc from "@/lib/ipc";
import { searchSecrets } from "@/lib/secret-search";
import { isAndroidClient } from "@/lib/platform";

/** How long typing must pause before a query is parsed into filters. */
const FILTER_DEBOUNCE_MS = 150;

export function SearchPalette({
  open,
  onOpenChange,
  onSelect,
}: Readonly<{
  open: boolean;
  onOpenChange: (v: boolean) => void;
  onSelect: (s: Secret) => void;
}>) {
  const secrets = useSecretList();
  const isAndroid = isAndroidClient();
  const [q, setQ] = React.useState("");
  const [cursor, setCursor] = React.useState(0);
  // Structured results for one specific query. Tagged with the query they
  // belong to, so a stale answer never shows under newer text.
  const [filtered, setFiltered] = React.useState<{
    query: string;
    results: Secret[];
    description: string;
  } | null>(null);

  React.useEffect(() => {
    if (open) {
      setQ("");
      setCursor(0);
      setFiltered(null);
    }
  }, [open]);

  const substringResults = React.useMemo(() => searchSecrets(secrets, q), [q, secrets]);
  const trimmed = q.trim();

  /**
   * Plain name matching is instant and usually enough. Only when it finds
   * nothing is the query parsed into filters ("production database" ->
   * environment + kind). That parse is rule-based in Rust and answers in
   * well under a millisecond; the short debounce just avoids one call per
   * keystroke. A failed parse quietly leaves the plain results in place.
   */
  React.useEffect(() => {
    if (!trimmed || substringResults.length > 0) return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      ipc
        .searchParseQuery(trimmed)
        .then((filter) => {
          if (cancelled || isEmptyFilter(filter)) return;
          setFiltered({
            query: trimmed,
            results: applySearchFilter(secrets, filter),
            description: describeFilter(filter),
          });
          setCursor(0);
        })
        .catch(() => {
          // Plain matching already answered; nothing else to show.
        });
    }, FILTER_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [trimmed, substringResults.length, secrets]);

  const usingFilter = substringResults.length === 0 && filtered?.query === trimmed;
  const results = usingFilter && filtered ? filtered.results : substringResults;

  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/55" />
        <DialogPrimitive.Content
          style={
            isAndroid
              ? {
                  inset: 0,
                  width: "100vw",
                  maxWidth: "100vw",
                  transform: "none",
                  translate: "none",
                }
              : undefined
          }
          className="search-palette fixed left-1/2 top-[18%] z-50 w-full max-w-[520px] -translate-x-1/2 overflow-hidden rounded-lg border border-border bg-surface shadow-[0_16px_48px_-12px_rgba(0,0,0,0.6)]"
        >
          <DialogPrimitive.Title className="sr-only">Search</DialogPrimitive.Title>
          <div className="flex items-center gap-2 border-b border-border px-3">
            <Search className="size-3.5 shrink-0 text-subtle-foreground" />
            <input
              autoFocus
              value={q}
              onChange={(e) => {
                setQ(e.target.value);
                setCursor(0);
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setCursor((c) => Math.min(c + 1, results.length - 1));
                  return;
                }
                if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setCursor((c) => Math.max(c - 1, 0));
                  return;
                }
                if (e.key !== "Enter") return;
                e.preventDefault();
                if (results[cursor]) {
                  onSelect(results[cursor]);
                  onOpenChange(false);
                }
              }}
              placeholder="Search by name, project, environment, or type"
              className="h-9 w-full bg-transparent text-[13px] placeholder:text-subtle-foreground focus:outline-none"
            />
            <span className="kbd shrink-0">Esc</span>
          </div>

          {usingFilter && filtered && (
            <div className="flex items-center gap-1.5 border-b border-border/60 px-3 py-1.5 text-[10.5px] text-subtle-foreground">
              <SlidersHorizontal className="size-3" />
              Filtered by {filtered.description}
            </div>
          )}

          {results.length === 0 ? (
            <div className="px-4 py-8 text-center">
              <p className="text-[12.5px]">No results for "{q}"</p>
              <p className="mt-1 text-[11.5px] text-muted-foreground">
                Try a name, project, tag, or words like "production database".
              </p>
            </div>
          ) : (
            <ul className="max-h-[300px] overflow-y-auto p-1">
              {results.map((s, i) => (
                <li key={s.id}>
                  <button
                    type="button"
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
                    <span className="text-[11.5px] text-muted-foreground">{s.type}</span>
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
