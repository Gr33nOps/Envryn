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

const aiParseSearchIntent = vi.fn();
const isTauri = vi.fn(() => true);

class FakeIpcError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.code = code;
  }
}

vi.mock("@/lib/ipc", () => ({
  isTauri: (...a: unknown[]) => isTauri(...(a as [])),
  aiParseSearchIntent: (...a: unknown[]) => aiParseSearchIntent(...(a as [string])),
  aiStatus: vi.fn().mockResolvedValue({ enabled_in_settings: true, engine_running: true }),
  IpcError: FakeIpcError,
}));

vi.mock("@/lib/use-vault", () => ({
  useSecretList: () => VAULT,
}));

const { SearchPalette } = await import("./SearchPalette");

function open() {
  return render(<SearchPalette open onOpenChange={() => {}} onSelect={() => {}} />);
}

function type(value: string) {
  fireEvent.change(screen.getByPlaceholderText(/Search your vault/i), { target: { value } });
}

beforeEach(() => {
  aiParseSearchIntent.mockReset();
  aiParseSearchIntent.mockResolvedValue({
    project: null,
    environment: null,
    kind: null,
    tags: [],
    text: null,
  });
});

describe("SearchPalette: typing never triggers a search", () => {
  /**
   * The reported bug: natural-language search fired on a debounce timer
   * after every keystroke, so typing a sentence launched a burst of
   * inference requests nobody asked for. Typing must now do nothing at all.
   */
  it("does not call the AI search backend while the user types", async () => {
    open();
    for (const value of ["p", "pr", "pro", "prod", "produ", "production keys"]) {
      type(value);
    }
    // Give any lingering debounce/effect a generous window to misfire.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 700));
    });
    expect(aiParseSearchIntent).not.toHaveBeenCalled();
  });

  it("shows no loading state until an explicit submit", async () => {
    open();
    type("production database");
    await act(async () => {
      await new Promise((r) => setTimeout(r, 700));
    });
    expect(screen.queryByText(/Searching your vault/i)).not.toBeInTheDocument();
  });
});

describe("SearchPalette: explicit submission", () => {
  it("runs assisted search when the Interpret button is clicked", async () => {
    open();
    type("production database");
    fireEvent.click(screen.getByRole("button", { name: /^Interpret$/i }));
    await waitFor(() => expect(aiParseSearchIntent).toHaveBeenCalledTimes(1));
    expect(aiParseSearchIntent).toHaveBeenCalledWith("production database");
  });

  it("opens a local result on Enter without invoking assisted search", async () => {
    open();
    type("production database");
    fireEvent.keyDown(screen.getByPlaceholderText(/Search your vault/i), { key: "Enter" });
    expect(aiParseSearchIntent).not.toHaveBeenCalled();
  });

  it("uses assisted search on Enter when local metadata has no result", async () => {
    open();
    type("credentials I used last winter");
    fireEvent.keyDown(screen.getByPlaceholderText(/Search your vault/i), { key: "Enter" });
    await waitFor(() => expect(aiParseSearchIntent).toHaveBeenCalledTimes(1));
  });

  it("disables submission for an empty query", () => {
    open();
    type("   ");
    expect(screen.getByRole("button", { name: /^Interpret$/i })).toBeDisabled();
    fireEvent.keyDown(screen.getByPlaceholderText(/Search your vault/i), { key: "Enter" });
    expect(aiParseSearchIntent).not.toHaveBeenCalled();
  });

  it("does not start a second search while one is still running", async () => {
    let release: (v: unknown) => void = () => {};
    aiParseSearchIntent.mockImplementation(() => new Promise((resolve) => (release = resolve)));
    open();
    type("production database");
    const button = screen.getByRole("button", { name: /Interpret/i });
    fireEvent.click(button);
    await waitFor(() => expect(screen.getByText(/Searching your vault/i)).toBeInTheDocument());
    fireEvent.click(button);
    fireEvent.click(button);
    expect(aiParseSearchIntent).toHaveBeenCalledTimes(1);
    await act(async () => {
      release({ project: null, environment: null, kind: null, tags: [], text: null });
    });
  });
});

describe("SearchPalette: known records are actually found", () => {
  /**
   * The "AI search always returns No match found" regression. Each case
   * feeds the filter the backend genuinely produces for that query and
   * asserts the right records survive -- proving the filter-application
   * step works, which is where an absent `tags` array used to throw.
   */
  it("finds production databases from a structured filter", async () => {
    aiParseSearchIntent.mockResolvedValue({
      project: null,
      environment: "Production",
      kind: "Database",
      tags: [],
      text: null,
    });
    open();
    type("production databases");
    fireEvent.click(screen.getByRole("button", { name: /^Interpret$/i }));
    await waitFor(() => expect(screen.getByText("Primary Postgres URL")).toBeInTheDocument());
    expect(screen.queryByText("Staging Postgres URL")).not.toBeInTheDocument();
    expect(screen.queryByText("Stripe Live Secret Key")).not.toBeInTheDocument();
  });

  it("finds a provider by residual free text", async () => {
    aiParseSearchIntent.mockResolvedValue({
      project: null,
      environment: null,
      kind: null,
      tags: [],
      text: "openrouter",
    });
    open();
    type("my openrouter key");
    fireEvent.click(screen.getByRole("button", { name: /^Interpret$/i }));
    await waitFor(() => expect(screen.getByText("OpenRouter API Key")).toBeInTheDocument());
    expect(screen.queryByText("Stripe Live Secret Key")).not.toBeInTheDocument();
  });

  it("matches multi-word free text whose words are not adjacent", async () => {
    aiParseSearchIntent.mockResolvedValue({
      project: null,
      environment: null,
      kind: null,
      tags: [],
      text: "stripe key",
    });
    open();
    type("stripe key");
    fireEvent.click(screen.getByRole("button", { name: /^Interpret$/i }));
    // "Stripe Live Secret Key" contains both words, but not as the adjacent
    // substring "stripe key" -- a naive includes() check would miss it.
    await waitFor(() => expect(screen.getByText("Stripe Live Secret Key")).toBeInTheDocument());
  });

  it("survives a filter whose optional fields are entirely absent", async () => {
    // Exactly what a small model returns when it omits keys: the frontend
    // must not throw on `filter.tags.length`.
    aiParseSearchIntent.mockResolvedValue({ text: "supabase" });
    open();
    type("supabase");
    fireEvent.click(screen.getByRole("button", { name: /^Interpret$/i }));
    await waitFor(() => expect(screen.getByText("Supabase Service Role")).toBeInTheDocument());
  });

  it("finds a project's secrets", async () => {
    aiParseSearchIntent.mockResolvedValue({
      project: "acme-payments",
      environment: null,
      kind: null,
      tags: [],
      text: null,
    });
    open();
    type("acme-payments");
    fireEvent.click(screen.getByRole("button", { name: /^Interpret$/i }));
    await waitFor(() => expect(screen.getByText("Stripe Live Secret Key")).toBeInTheDocument());
    expect(screen.queryByText("GitHub Deploy Token")).not.toBeInTheDocument();
  });
});

describe("SearchPalette: AI failures stay recoverable", () => {
  /**
   * The crash requirement: a worker crash, timeout, malformed response, or
   * IPC disconnect must surface as an in-dialog message, never take the
   * component (or the app) down.
   */
  it("shows a recoverable error when the backend rejects", async () => {
    aiParseSearchIntent.mockRejectedValue(
      new FakeIpcError("ai_unavailable", "The local AI model is not available right now."),
    );
    open();
    type("production database");
    fireEvent.click(screen.getByRole("button", { name: /^Interpret$/i }));
    await waitFor(() => expect(screen.getByText(/not available right now/i)).toBeInTheDocument());
    // The dialog is still mounted and still usable.
    expect(screen.getByPlaceholderText(/Search your vault/i)).toBeInTheDocument();
  });

  it("falls back to plain name matches after a failure", async () => {
    aiParseSearchIntent.mockRejectedValue(new Error("worker died mid-request"));
    open();
    type("Stripe");
    fireEvent.click(screen.getByRole("button", { name: /^Interpret$/i }));
    await waitFor(() => expect(screen.getByText(/could not be completed/i)).toBeInTheDocument());
    expect(screen.getByText("Stripe Live Secret Key")).toBeInTheDocument();
  });

  it("recovers on a retry after a failed search", async () => {
    aiParseSearchIntent.mockRejectedValueOnce(new Error("transient"));
    open();
    type("supabase");
    fireEvent.click(screen.getByRole("button", { name: /^Interpret$/i }));
    await waitFor(() => expect(screen.getByText(/could not be completed/i)).toBeInTheDocument());

    aiParseSearchIntent.mockResolvedValue({
      project: null,
      environment: null,
      kind: null,
      tags: [],
      text: "supabase",
    });
    fireEvent.click(screen.getByRole("button", { name: /^Interpret$/i }));
    await waitFor(() => expect(screen.getByText("Supabase Service Role")).toBeInTheDocument());
    expect(screen.queryByText(/could not be completed/i)).not.toBeInTheDocument();
  });
});
