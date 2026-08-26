import * as React from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { Search, Sparkles } from "lucide-react";
import { type Secret } from "@/lib/envryn-data";
import { useSecretList } from "@/lib/use-vault";
import { KIND_TO_TYPE, toUiEnvironment } from "@/lib/vault-repository";
import { cn } from "@/lib/utils";
import * as ipc from "@/lib/ipc";

/**
 * Turn a parsed `SearchFilterOutput` into the same `Secret[]` shape plain
 * substring filtering already produces, so both paths render through one
 * result list. `text` (if the model extracted a residual free-text term)
 * still matches by substring -- only `project`/`environment`/`kind`/`tags`
 * are structured.
 */
function applyAiFilter(secrets: Secret[], filter: ipc.SearchFilterOutput): Secret[] {
  const text = filter.text?.trim().toLowerCase();
  return secrets.filter((s) => {
    if (filter.project && s.project.toLowerCase() !== filter.project.toLowerCase()) return false;
    if (filter.environment && s.environment !== toUiEnvironment(filter.environment)) return false;
    if (filter.kind && s.type !== KIND_TO_TYPE[filter.kind]) return false;
    if (filter.tags.length && !filter.tags.some((t) => (s.tags ?? []).includes(t))) return false;
    if (text) {
      const haystack = [s.name, s.project, s.provider ?? "", ...(s.tags ?? [])]
        .join(" ")
        .toLowerCase();
      if (!haystack.includes(text)) return false;
    }
    return true;
  });
}

function SearchStatusBanner({
  aiSearching,
  aiResults,
}: Readonly<{ aiSearching: boolean; aiResults: Secret[] | null }>) {
  if (aiSearching) {
    return (
      <div className="flex items-center gap-1.5 border-b border-border/60 px-3 py-1.5 text-[10.5px] text-subtle-foreground">
        <Sparkles className="size-3 animate-pulse" />
        Asking local AI what you mean...
      </div>
    );
  }
  if (aiResults) {
    return (
      <div className="flex items-center gap-1.5 border-b border-border/60 px-3 py-1.5 text-[10.5px] text-subtle-foreground">
        <Sparkles className="size-3" />
        Matched by local AI, not an exact search
      </div>
    );
  }
  return null;
}

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
  const [q, setQ] = React.useState("");
  const [cursor, setCursor] = React.useState(0);
  const [aiResults, setAiResults] = React.useState<Secret[] | null>(null);
  const [aiSearching, setAiSearching] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setQ("");
      setCursor(0);
      setAiResults(null);
    }
  }, [open]);

  const substringResults = React.useMemo(() => {
    const t = q.trim().toLowerCase();
    if (!t) return secrets.slice(0, 6);
    return secrets.filter((s) =>
      [s.name, s.project, s.environment, s.type, s.provider ?? "", ...(s.tags ?? [])]
        .join(" ")
        .toLowerCase()
        .includes(t),
    );
  }, [q, secrets]);

  // Natural-language fallback (docs/AI_DATA_ACCESS.md Tier 1 "search"): only
  // attempted once plain substring matching finds nothing and the query
  // looks like a sentence rather than a single term someone would expect to
  // match literally -- never replaces the fast, deterministic path above,
  // only fills the gap it deliberately leaves (no fuzzy/semantic matching).
  React.useEffect(() => {
    setAiResults(null);
    if (!ipc.isTauri() || substringResults.length > 0) return;
    const trimmed = q.trim();
    if (trimmed.split(/\s+/).length < 2) return;

    let cancelled = false;
    const timer = setTimeout(() => {
      void (async () => {
        setAiSearching(true);
        try {
          const status = await ipc.aiStatus();
          if (!status.enabled_in_settings || !status.engine_running) return;
          const filter = await ipc.aiParseSearchIntent(trimmed);
          if (!cancelled) setAiResults(applyAiFilter(secrets, filter));
        } catch {
          // No AI result is a silent fallback to "no matches," never a
          // blocked search -- the substring path already answered the user.
        } finally {
          if (!cancelled) setAiSearching(false);
        }
      })();
    }, 500);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [q, secrets, substringResults.length]);

  const results = aiResults ?? substringResults;

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
                if (e.key === "ArrowDown") setCursor((c) => Math.min(c + 1, results.length - 1));
                if (e.key === "ArrowUp") setCursor((c) => Math.max(c - 1, 0));
                if (e.key === "Enter" && results[cursor]) {
                  onSelect(results[cursor]);
                  onOpenChange(false);
                }
              }}
              placeholder="Search everywhere in your vault..."
              className="h-9 w-full bg-transparent text-[13px] placeholder:text-subtle-foreground focus:outline-none"
            />
            <span className="kbd">Esc</span>
          </div>

          <SearchStatusBanner aiSearching={aiSearching} aiResults={aiResults} />

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
