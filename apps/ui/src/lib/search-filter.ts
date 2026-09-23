import type { Secret } from "./envryn-data";
import type { SearchFilter } from "./ipc";
import { KIND_TO_TYPE, toUiEnvironment } from "./vault-repository";

/**
 * Apply a parsed `SearchFilter` to the vault list. `text` (any words the
 * parser did not turn into a filter) still matches by substring -- only
 * `project`/`environment`/`kind`/`tags` are structured.
 */
export function applySearchFilter(secrets: Secret[], filter: SearchFilter): Secret[] {
  const text = filter.text?.trim().toLowerCase();
  // Defensive default: this runs on whatever crossed the IPC boundary, and
  // `.length` on an absent array would throw and unmount the dialog.
  const tags = filter.tags ?? [];
  return secrets.filter((s) => {
    if (filter.project && s.project.toLowerCase() !== filter.project.toLowerCase()) return false;
    if (filter.environment && s.environment !== toUiEnvironment(filter.environment)) return false;
    // An unmapped kind is "no constraint", never "matches nothing".
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
      // Every term must appear somewhere, in any order. A trailing plural "s"
      // is optional, so "stripe keys" matches a "Stripe Live Secret Key".
      const matches = (term: string) =>
        haystack.includes(term) ||
        (term.length > 3 && term.endsWith("s") && haystack.includes(term.slice(0, -1)));
      if (!text.split(/\s+/).every(matches)) return false;
    }
    return true;
  });
}

/** "Production · Database" -- what a parsed query narrowed the list to. */
export function describeFilter(filter: SearchFilter): string {
  const parts: string[] = [];
  if (filter.environment) parts.push(toUiEnvironment(filter.environment));
  if (filter.kind) parts.push(KIND_TO_TYPE[filter.kind] ?? filter.kind);
  if (filter.project) parts.push(filter.project);
  if (filter.text?.trim()) parts.push(`"${filter.text.trim()}"`);
  return parts.join(" · ");
}

export function isEmptyFilter(filter: SearchFilter): boolean {
  return (
    !filter.project &&
    !filter.environment &&
    !filter.kind &&
    !(filter.tags ?? []).length &&
    !filter.text?.trim()
  );
}
