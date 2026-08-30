import type { Secret } from "./envryn-data";

function normalize(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

function isSubsequence(term: string, candidate: string): boolean {
  let index = 0;
  for (const character of candidate) {
    if (character === term[index]) index += 1;
    if (index === term.length) return true;
  }
  return false;
}

function scoreTerm(term: string, fields: string[]): number {
  let best = 0;
  for (const field of fields) {
    if (field === term) best = Math.max(best, 120);
    else if (field.startsWith(term)) best = Math.max(best, 90);
    else if (field.includes(term)) best = Math.max(best, 65);
    else if (term.length >= 3 && isSubsequence(term, field)) best = Math.max(best, 25);
  }
  return best;
}

/** Fast local metadata search. No secret values and no model are involved. */
export function searchSecrets(secrets: Secret[], query: string, limit = 50): Secret[] {
  const normalizedQuery = normalize(query);
  if (!normalizedQuery) return secrets.slice(0, Math.min(limit, 6));

  // Exact names are the most common command-palette lookup and have an
  // explicit 300 ms release target at 1,000 records. Resolve them in one
  // allocation-light pass before building fuzzy field arrays for every row.
  const exactNames = secrets.filter((secret) => normalize(secret.name) === normalizedQuery);
  if (exactNames.length > 0) return exactNames.slice(0, limit);

  const terms = normalizedQuery.split(/\s+/).filter(Boolean);
  if (!terms.length) return secrets.slice(0, Math.min(limit, 6));

  return secrets
    .map((secret) => {
      const fields = [
        secret.name,
        secret.project,
        secret.environment,
        secret.type,
        secret.provider ?? "",
        ...(secret.tags ?? []),
      ]
        .flatMap((field) => {
          const normalized = normalize(field);
          return [normalized, ...normalized.split(" ")];
        })
        .filter(Boolean);
      const scores = terms.map((term) => scoreTerm(term, fields));
      if (scores.some((score) => score === 0)) return { secret, score: 0 };
      return { secret, score: scores.reduce((sum, score) => sum + score, 0) };
    })
    .filter(({ score }) => score > 0)
    .sort((a, b) => b.score - a.score || a.secret.name.localeCompare(b.secret.name))
    .slice(0, limit)
    .map(({ secret }) => secret);
}
