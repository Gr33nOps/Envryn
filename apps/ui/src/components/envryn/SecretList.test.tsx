import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Secret } from "@/lib/envryn-data";
import { SecretList } from "./SecretList";

const controls = vi.hoisted(() => ({
  select: vi.fn(),
  openEdit: vi.fn(),
  reveal: vi.fn().mockResolvedValue("secret-value"),
  copy: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("./vault-context", () => ({
  useVaultUI: () => ({ selected: null, select: controls.select, openEdit: controls.openEdit }),
}));

vi.mock("@/lib/use-vault", () => ({
  useRevealSecret: () => ({ mutateAsync: controls.reveal }),
}));

vi.mock("@/lib/vault-actions", () => ({
  copyValue: (...args: unknown[]) => controls.copy(...args),
}));

const items = [
  {
    id: "one",
    name: "STRIPE_KEY",
    project: "payments",
    environment: "Production",
    type: "API Key",
    provider: "Stripe",
    tags: ["billing"],
    updated: "Today",
    damaged: true,
  },
  {
    id: "two",
    name: "Untitled note",
    project: "general",
    environment: "—",
    type: "Note",
    updated: "Yesterday",
  },
] as Secret[];

beforeEach(() => {
  controls.select.mockClear();
  controls.openEdit.mockClear();
  controls.reveal.mockClear();
  controls.copy.mockClear();
});

describe("SecretList", () => {
  it("renders production and empty-environment states and exposes accessible selection", () => {
    render(<SecretList items={items} />);
    expect(screen.getByText("Production")).toBeInTheDocument();
    expect(screen.getByText("No environment")).toBeInTheDocument();
    expect(screen.getByLabelText("Needs review")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open STRIPE_KEY details" }));
    expect(controls.select).toHaveBeenCalledWith(items[0]);
  });

  it("reveals only on demand before copying", async () => {
    render(<SecretList items={items.slice(0, 1)} />);
    fireEvent.click(screen.getByRole("button", { name: "Copy secret" }));
    await waitFor(() => expect(controls.reveal).toHaveBeenCalledWith("one"));
    expect(controls.copy).toHaveBeenCalledWith("secret-value");
  });

  it("offers reveal, copy, and edit from the context menu", () => {
    render(<SecretList items={items.slice(0, 1)} />);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Open STRIPE_KEY details" }), {
      clientX: 20,
      clientY: 30,
    });
    fireEvent.click(screen.getByRole("button", { name: "Edit secret" }));
    expect(controls.openEdit).toHaveBeenCalledWith(items[0]);
  });
});
