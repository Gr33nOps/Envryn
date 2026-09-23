import { describe, expect, it } from "vitest";
import type { Secret } from "./envryn-data";
import { applySearchFilter, describeFilter, isEmptyFilter } from "./search-filter";

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
    project: "Shop",
    environment: "Staging",
    type: "Database",
    provider: "PostgreSQL",
    tags: [],
  },
  {
    id: "3",
    name: "Deploy notes",
    project: "infra",
    environment: "—",
    type: "Note",
    tags: undefined,
  },
] as Secret[];

const ids = (secrets: Secret[]) => secrets.map((s) => s.id);

describe("applySearchFilter", () => {
  it("matches the project case-insensitively and the environment through the UI mapping", () => {
    expect(ids(applySearchFilter(records, { project: "SHOP", tags: [] }))).toEqual(["1", "2"]);
    expect(ids(applySearchFilter(records, { environment: "Unassigned", tags: [] }))).toEqual(["3"]);
  });

  it("filters by kind and by tag", () => {
    expect(ids(applySearchFilter(records, { kind: "Database", tags: [] }))).toEqual(["2"]);
    expect(ids(applySearchFilter(records, { tags: ["billing"] }))).toEqual(["1"]);
  });

  it("requires every leftover word, with an optional plural s", () => {
    expect(ids(applySearchFilter(records, { text: "stripe keys", tags: [] }))).toEqual(["1"]);
    expect(ids(applySearchFilter(records, { text: "stripe postgres", tags: [] }))).toEqual([]);
    expect(ids(applySearchFilter(records, { text: "notes", tags: [] }))).toEqual(["3"]);
  });

  it("tolerates a filter with no tags array from the native side", () => {
    const filter = { project: "infra" } as unknown as Parameters<typeof applySearchFilter>[1];
    expect(ids(applySearchFilter(records, filter))).toEqual(["3"]);
  });
});

describe("describeFilter and isEmptyFilter", () => {
  it("summarises what a parsed query narrowed the list to", () => {
    expect(
      describeFilter({
        environment: "Production",
        kind: "Database",
        project: "shop",
        text: " primary ",
        tags: [],
      }),
    ).toBe('Production · Database · shop · "primary"');
    expect(describeFilter({ tags: [] })).toBe("");
  });

  it("treats blank text and an empty tag list as no filter", () => {
    expect(isEmptyFilter({ text: "  ", tags: [] })).toBe(true);
    expect(isEmptyFilter({ tags: ["billing"] })).toBe(false);
    expect(isEmptyFilter({ kind: "Token", tags: [] })).toBe(false);
  });
});
