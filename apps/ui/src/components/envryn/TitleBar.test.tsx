import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { TitleBar } from "./TitleBar";

const minimize = vi.fn().mockResolvedValue(undefined);
const toggleMaximize = vi.fn().mockResolvedValue(undefined);
const close = vi.fn().mockResolvedValue(undefined);
const isMaximized = vi.fn().mockResolvedValue(false);
const onResized = vi.fn().mockResolvedValue(() => {});

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ minimize, toggleMaximize, close, isMaximized, onResized }),
}));

vi.mock("@/lib/ipc", () => ({
  isTauri: vi.fn(() => true),
}));

beforeEach(() => {
  minimize.mockClear();
  toggleMaximize.mockClear();
  close.mockClear();
  isMaximized.mockClear().mockResolvedValue(false);
});

describe("TitleBar", () => {
  it("renders minimize, maximize, and close controls", () => {
    render(<TitleBar />);
    expect(screen.getByLabelText("Minimize")).toBeInTheDocument();
    expect(screen.getByLabelText("Maximize")).toBeInTheDocument();
    expect(screen.getByLabelText("Close")).toBeInTheDocument();
  });

  it("clicking Minimize calls the real window API", async () => {
    render(<TitleBar />);
    fireEvent.click(screen.getByLabelText("Minimize"));
    await waitFor(() => expect(minimize).toHaveBeenCalledTimes(1));
  });

  it("clicking Maximize calls the real window API", async () => {
    render(<TitleBar />);
    fireEvent.click(screen.getByLabelText("Maximize"));
    await waitFor(() => expect(toggleMaximize).toHaveBeenCalledTimes(1));
  });

  it("clicking Close calls the real window API", async () => {
    render(<TitleBar />);
    fireEvent.click(screen.getByLabelText("Close"));
    await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
  });

  it("double-clicking the drag region also toggles maximize", async () => {
    render(<TitleBar />);
    const dragRegions = screen
      .getAllByRole("generic")
      .filter((el) => el.hasAttribute("data-tauri-drag-region"));
    expect(dragRegions.length).toBeGreaterThan(0);
    fireEvent.doubleClick(dragRegions[0] as HTMLElement);
    await waitFor(() => expect(toggleMaximize).toHaveBeenCalledTimes(1));
  });

  it("relabels the maximize button to Restore once the window reports maximized", async () => {
    isMaximized.mockResolvedValue(true);
    render(<TitleBar />);
    await waitFor(() => expect(screen.getByLabelText("Restore")).toBeInTheDocument());
    expect(screen.queryByLabelText("Maximize")).not.toBeInTheDocument();
  });
});

describe("TitleBar outside Tauri", () => {
  it("never touches the window API when isTauri() is false", async () => {
    const ipc = await import("@/lib/ipc");
    vi.mocked(ipc.isTauri).mockReturnValue(false);

    render(<TitleBar />);
    fireEvent.click(screen.getByLabelText("Close"));

    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(close).not.toHaveBeenCalled();
  });
});
