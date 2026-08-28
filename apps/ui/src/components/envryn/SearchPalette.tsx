import * as React from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { Search, Sparkles, TriangleAlert } from "lucide-react";
import { type Secret } from "@/lib/envryn-data";
import { useSecretList } from "@/lib/use-vault";
import { KIND_TO_TYPE, toUiEnvironment } from "@/lib/vault-repository";
import { cn } from "@/lib/utils";
import * as ipc from "@/lib/ipc";
import { IpcError } from "@/lib/ipc";

/**
 * Turn a parsed `SearchFilterOutput` into the same `Secret[]` shape plain
 * substring filtering already produces, so both paths render through one
 * result list. `text` (if the model extracted a residual free-text term)
 * still matches by substring -- only `project`/`environment`/`kind`/`tags`
 * are structured.
 */
function applyAiFilter(secrets: Secret[], filter: ipc.SearchFilterOutput): Secret[] {
  const text = filter.text?.trim().toLowerCase();
  // Every field is defensively defaulted. The Rust type guarantees the
  // shape, but this runs on whatever the IPC boundary actually handed back,
  // and a `filter.tags.length` on an absent array is a TypeError that
  // unmounts the dialog rather than showing "no results".
  const tags = filter.tags ?? [];
  return secrets.filter((s) => {
    if (filter.project && s.project.toLowerCase() !== filter.project.toLowerCase()) return false;
    if (filter.environment && s.environment !== toUiEnvironment(filter.environment)) return false;
    // An unrecognised kind must not silently exclude everything -- if the
    // map has no entry, treat the kind as "no constraint" rather than as a
    // constraint nothing can satisfy.
    if (filter.kind) {
      const mapped = KIND_TO_TYPE[filter.kind];
      if (mapped && s.type !== mapped) return false;
    }
    if (tags.length && !tags.some((t) => (s.tags ?? []).includes(t))) return false;
    if (text) {
      const haystack = [
        s.name,
        s.project,
        s.environment,
        s.type,
        s.provider ?? "",
        ...(s.tags ?? []),
      ]
        .join(" ")
        .toLowerCase();
      // Every whitespace-separated term must appear somewhere, so a
      // residual like "stripe keys" still matches a "Stripe API Key"
      // record whose words are not adjacent in that order.
      if (!text.split(/\s+/).every((term) => haystack.includes(term))) return false;
    }
    return true;
  });
}

function SearchStatusBanner({
  aiSearching,
  aiResults,
  aiError,
}: Readonly<{ aiSearching: boolean; aiResults: Secret[] | null; aiError: string | null }>) {
  if (aiSearching) {
    return (
      <div className="flex items-center gap-1.5 border-b border-border/60 px-3 py-1.5 text-[10.5px] text-subtle-foreground">
        <Sparkles className="size-3 animate-pulse" />
        Searching your vault...
      </div>
    );
  }
  // A failed search is a visible, recoverable state -- not a silently empty
  // result list that looks identical to "you have nothing matching this".
  if (aiError) {
    return (
      <div className="flex items-center gap-1.5 border-b border-border/60 px-3 py-1.5 text-[10.5px] text-warning">
        <TriangleAlert className="size-3" />
        {aiError} Showing plain name matches instead.
      </div>
    );
  }
  if (aiResults) {
    return (
      <div className="flex items-center gap-1.5 border-b border-border/60 px-3 py-1.5 text-[10.5px] text-subtle-foreground">
        <Sparkles className="size-3" />
        Interpreted your search
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
  const [aiError, setAiError] = React.useState<string | null>(null);
  // Which query the current `aiResults` belong to. Editing the box after a
  // search must clear the stale result set without launching a new one.
  const [searchedQuery, setSearchedQuery] = React.useState<string | null>(null);
  // Guards against a second in-flight request: the worker answers one
  // request at a time, so a double-click would queue rather than parallelise.
  const runningRef = React.useRef(false);

  React.useEffect(() => {
    if (open) {
      setQ("");
      setCursor(0);
      setAiResults(null);
      setAiError(null);
      setSearchedQuery(null);
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

  const trimmed = q.trim();
  const canSearch = trimmed.length > 0 && !aiSearching;

  /**
   * Run the assisted search. **Only ever called from the Search button or
   * the Enter key** -- never from a `useEffect` watching the query.
   *
   * It used to run on a 500ms timer after every keystroke, which meant
   * typing a sentence fired a burst of inference requests, each one
   * competing for the same single-threaded worker, for a result the user
   * had not asked for yet. Nothing here is triggered by typing now.
   */
  async function runSearch() {
    const query = q.trim();
    if (!query || runningRef.current) return;

    runningRef.current = true;
    setAiSearching(true);
    setAiError(null);
    try {
      // `aiParseSearchIntent` deliberately never fails closed: with AI off
      // or the worker down it still returns a deterministic parse, so this
      // is a real search either way rather than a disabled feature.
      const filter = await ipc.aiParseSearchIntent(query);
      const matched = applyAiFilter(secrets, filter);
      setAiResults(matched);
      setSearchedQuery(query);
      setCursor(0);
    } catch (err) {
      // A worker crash, timeout, or malformed response lands here and shows
      // an inline, recoverable message. It must never propagate -- an
      // unhandled rejection out of this handler would unmount the dialog.
      setAiResults(null);
      setSearchedQuery(query);
      setAiError(
        err instanceof IpcError ? err.message : "Search could not be completed. Try again.",
      );
    } finally {
      runningRef.current = false;
      setAiSearching(false);
    }
  }

  // Typing invalidates a previous result set without starting a new search.
  const resultsAreStale = searchedQuery !== null && searchedQuery !== trimmed;
  const results = aiResults && !resultsAreStale ? aiResults : substringResults;

  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/55" />
        <DialogPrimitive.Content className="search-palette fixed left-1/2 top-[18%] z-50 w-full max-w-[520px] -translate-x-1/2 overflow-hidden rounded-lg border border-border bg-surface shadow-[0_16px_48px_-12px_rgba(0,0,0,0.6)]">
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
                // Enter submits the search. Only once a search has run (or
                // the plain substring list is already showing a highlighted
                // row for this exact query) does Enter open that row --
                // otherwise the first Enter would skip searching entirely.
                if (canSearch && (resultsAreStale || searchedQuery === null)) {
                  void runSearch();
                  return;
                }
                if (results[cursor]) {
                  onSelect(results[cursor]);
                  onOpenChange(false);
                }
              }}
              placeholder="Search your vault, then press Enter"
              className="h-9 w-full bg-transparent text-[13px] placeholder:text-subtle-foreground focus:outline-none"
            />
            <button
              type="button"
              onClick={() => void runSearch()}
              disabled={!canSearch}
              className="shrink-0 rounded-md border border-border bg-surface-2 px-2 py-1 text-[11px] text-muted-foreground transition-colors hover:border-border-strong hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-border disabled:hover:text-muted-foreground"
            >
              {aiSearching ? "Searching..." : "Search"}
            </button>
            <span className="kbd shrink-0">Esc</span>
          </div>

          <SearchStatusBanner
            aiSearching={aiSearching}
            aiResults={resultsAreStale ? null : aiResults}
            aiError={resultsAreStale ? null : aiError}
          />

          {results.length === 0 ? (
            <div className="px-4 py-8 text-center">
              <p className="text-[12.5px]">No results for "{q}"</p>
              <p className="mt-1 text-[11.5px] text-muted-foreground">
                {resultsAreStale || searchedQuery === null
                  ? "Press Enter or choose Search to look more thoroughly."
                  : "Try another name, project, or tag."}
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
