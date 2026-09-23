import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import type { Secret } from "@/lib/envryn-data";

/**
 * A synthetic vault of realistic-but-fake records, covering every provider
 * family the search pipeline is expected to find. No value here is a real
 * credential -- these records carry metadata only, which is all search ever
 * matches against.
 */
const VAULT: Secret[] = [
  {
    id: "1",
    name: "GitHub Deploy Token",
    project: "acme-web",
    environment: "Production",
    type: "Token",
    provider: "GitHub",
    tags: ["ci"],
  },
  {
    id: "2",
    name: "OpenRouter API Key",
    project: "acme-ai",
    environment: "Development",
    type: "API Key",
    provider: "OpenRouter",
    tags: ["llm"],
  },
  {
    id: "3",
    name: "Stripe Live Secret Key",
    project: "acme-payments",
    environment: "Production",
    type: "API Key",
    provider: "Stripe",
    tags: ["billing"],
  },
  {
    id: "4",
    name: "Primary Postgres URL",
    project: "acme-web",
    environment: "Production",
    type: "Database",
    provider: "PostgreSQL",
    tags: [],
  },
  {
    id: "5",
    name: "Supabase Service Role",
    project: "acme-web",
    environment: "Staging",
    type: "Token",
    provider: "Supabase",
    tags: [],
  },
  {
    id: "6",
    name: "Staging Postgres URL",
    project: "acme-web",
    environment: "Staging",
    type: "Database",
    provider: "PostgreSQL",
    tags: [],
  },
] as Secret[];

const searchParseQuery = vi.fn();
const isTauri = vi.fn(() => true);

vi.mock("@/lib/ipc", () => ({
  isTauri: (...a: unknown[]) => isTauri(...(a as [])),
  searchParseQuery: (...a: unknown[]) => searchParseQuery(...(a as [string])),
}));

vi.mock("@/lib/use-vault", () => ({
  useSecretList: () => VAULT,
}));

const { SearchPalette } = await import("./SearchPalette");
const { applySearchFilter } = await import("@/lib/search-filter");

function open(onSelect: (s: Secret) => void = () => {}) {
  return render(<SearchPalette open onOpenChange={() => {}} onSelect={onSelect} />);
}

function type(value: string) {
  fireEvent.change(screen.getByPlaceholderText(/Search by name/i), { target: { value } });
}

const EMPTY = { project: null, environment: null, kind: null, tags: [], text: null };

beforeEach(() => {
  searchParseQuery.mockReset();
  searchParseQuery.mockResolvedValue(EMPTY);
});

describe("SearchPalette: plain name matching", () => {
  it("matches by name as you type without parsing the query", async () => {
    open();
    type("stripe");
    expect(screen.getByText("Stripe Live Secret Key")).toBeTruthy();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 300));
    });
    // A plain match answered it; the filter parser is never asked.
    expect(searchParseQuery).not.toHaveBeenCalled();
  });

  it("selects the highlighted result on Enter", () => {
    const onSelect = vi.fn();
    open(onSelect);
    type("openrouter");
    fireEvent.keyDown(screen.getByPlaceholderText(/Search by name/i), { key: "Enter" });
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: "2" }));
  });
});

describe("SearchPalette: filters when plain matching finds nothing", () => {
  it("parses the query and shows what it filtered by", async () => {
    searchParseQuery.mockResolvedValue({ ...EMPTY, environment: "Production", kind: "Database" });
    open();
    // Nothing matches this by plain name search, so the parser is consulted
    // (its answer is mocked above).
    type("zq-prodn zq-dbase");
    await waitFor(() => expect(searchParseQuery).toHaveBeenCalledWith("zq-prodn zq-dbase"));
    expect(await screen.findByText("Primary Postgres URL")).toBeTruthy();
    expect(screen.queryByText("Staging Postgres URL")).toBeNull();
    expect(screen.getByText(/Filtered by Production · Database/)).toBeTruthy();
  });

  it("parses once after typing pauses, not on every keystroke", async () => {
    open();
    for (const value of ["p", "pr", "pro", "prod x", "production xyz"]) type(value);
    await waitFor(() => expect(searchParseQuery).toHaveBeenCalledTimes(1));
    expect(searchParseQuery).toHaveBeenCalledWith("production xyz");
  });

  it("keeps the empty state when the parser recognises nothing", async () => {
    open();
    type("zzzz-nothing");
    await waitFor(() => expect(searchParseQuery).toHaveBeenCalled());
    expect(screen.getByText(/No results for/)).toBeTruthy();
    expect(screen.queryByText(/Filtered by/)).toBeNull();
  });

  it("stays usable when the parse call fails", async () => {
    searchParseQuery.mockRejectedValue(new Error("ipc down"));
    open();
    type("zzzz-nothing");
    await waitFor(() => expect(searchParseQuery).toHaveBeenCalled());
    expect(screen.getByText(/No results for/)).toBeTruthy();
  });
});

describe("applySearchFilter", () => {
  it("narrows by environment, kind, and residual text together", () => {
    const got = applySearchFilter(VAULT, {
      ...EMPTY,
      environment: "Production",
      kind: "ApiKey",
      text: "stripe keys",
    });
    expect(got.map((s) => s.id)).toEqual(["3"]);
  });

  it("treats missing tags as no constraint", () => {
    const got = applySearchFilter(VAULT, {
      project: null,
      environment: "Staging",
      kind: null,
      text: null,
    } as unknown as Parameters<typeof applySearchFilter>[1]);
    expect(got.map((s) => s.id).sort()).toEqual(["5", "6"]);
  });
});
