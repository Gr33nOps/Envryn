import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { StructuredExtractModal } from "./StructuredExtractModal";

const controls = vi.hoisted(() => ({
  status: vi.fn(),
  extract: vi.fn(),
  create: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  aiStatus: (...args: unknown[]) => controls.status(...args),
  aiExtractStructuredFields: (...args: unknown[]) => controls.extract(...args),
  IpcError: class IpcError extends Error {},
}));

vi.mock("@/lib/use-vault", () => ({
  useProjects: () => [{ id: "one", name: "Envryn" }],
  useCreateSecret: () => ({ mutateAsync: controls.create }),
}));

vi.mock("@/components/envryn/ui", () => ({
  Button: ({ children, ...props }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
  Field: ({
    children,
    label,
    error,
  }: React.PropsWithChildren<{ label: string; error?: string }>) => (
    <label>
      {label}
      {children}
      {error && <span>{error}</span>}
    </label>
  ),
  Input: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props} />,
  Select: (props: React.SelectHTMLAttributes<HTMLSelectElement>) => <select {...props} />,
  Modal: ({
    children,
    footer,
    title,
  }: React.PropsWithChildren<{ footer: React.ReactNode; title: string }>) => (
    <div>
      <h1>{title}</h1>
      {children}
      {footer}
    </div>
  ),
}));

beforeEach(() => {
  controls.status.mockReset().mockResolvedValue({
    enabled_in_settings: true,
    engine_running: true,
  });
  controls.extract.mockReset().mockResolvedValue({
    fields: [{ label: "Host", value: "db.example.com" }],
  });
  controls.create.mockReset().mockResolvedValue(undefined);
});

describe("StructuredExtractModal", () => {
  it("validates empty input before calling local AI", async () => {
    render(<StructuredExtractModal open onOpenChange={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Extract fields" }));
    expect(await screen.findByText("Paste the text you want fields extracted from.")).toBeVisible();
    expect(controls.status).not.toHaveBeenCalled();
  });

  it("extracts, reviews, and saves fields", async () => {
    const onOpenChange = vi.fn();
    render(<StructuredExtractModal open onOpenChange={onOpenChange} />);
    fireEvent.change(screen.getByPlaceholderText(/Host: db.example.com/), {
      target: { value: "Host: db.example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Extract fields" }));
    await screen.findByDisplayValue("db.example.com");
    fireEvent.change(screen.getByPlaceholderText("e.g. Staging DB"), {
      target: { value: "Database" },
    });
    fireEvent.change(screen.getByPlaceholderText("e.g. Rescripto"), {
      target: { value: "Envryn" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save secret" }));
    await waitFor(() => expect(controls.create).toHaveBeenCalledTimes(1));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("refuses extraction while local AI is disabled", async () => {
    controls.status.mockResolvedValue({ enabled_in_settings: false, engine_running: false });
    render(<StructuredExtractModal open onOpenChange={() => {}} />);
    fireEvent.change(screen.getByPlaceholderText(/Host: db.example.com/), {
      target: { value: "Host: db.example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Extract fields" }));
    expect(await screen.findByText(/Enable local AI in Settings/)).toBeVisible();
  });
});
