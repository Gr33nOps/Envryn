import { describe, expect, it } from "vitest";
import type { Secret } from "./envryn-data";
import { searchSecrets } from "./secret-search";

const records = [
  {
    id: "1",
    name: "Stripe Live Secret Key",
    project: "shop",
    environment: "Production",
    type: "API Key",
    provider: "Stripe",
    tags: ["billing"],
  },
  {
    id: "2",
    name: "Staging Postgres URL",
    project: "shop",
    environment: "Staging",
    type: "Database",
    provider: "PostgreSQL",
    tags: [],
  },
] as Secret[];

describe("searchSecrets", () => {
  it("matches separated terms and ranks locally without AI", () => {
    expect(searchSecrets(records, "stripe key").map((secret) => secret.id)).toEqual(["1"]);
  });

  it("tolerates useful short typos through fuzzy matching", () => {
    expect(searchSecrets(records, "postgrs").map((secret) => secret.id)).toEqual(["2"]);
  });

  it("requires every query term to match metadata", () => {
    expect(searchSecrets(records, "stripe staging")).toEqual([]);
  });
});
