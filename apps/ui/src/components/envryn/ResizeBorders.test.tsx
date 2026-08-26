import { describe, expect, it, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import { ResizeBorders } from "./ResizeBorders";

const startResizeDragging = vi.fn().mockResolvedValue(undefined);

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startResizeDragging }),
}));

vi.mock("@/lib/ipc", () => ({
  isTauri: vi.fn(() => true),
}));

beforeEach(() => {
  startResizeDragging.mockClear();
});

describe("ResizeBorders", () => {
  it("renders all eight edge and corner handles", () => {
    const { container } = render(<ResizeBorders />);
    expect(container.querySelectorAll("div").length).toBe(8);
  });

  it("starts a resize in the correct direction when a handle is pressed", () => {
    const { container } = render(<ResizeBorders />);
    const handles = Array.from(container.querySelectorAll("div"));
    // Cursor style is a reliable, order-independent way to identify which
    // handle is which -- matches Handle's own `cursor` prop in the source.
    const north = handles.find((el) => el.style.cursor === "n-resize");
    expect(north).toBeDefined();

    north?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));

    expect(startResizeDragging).toHaveBeenCalledTimes(1);
    expect(startResizeDragging).toHaveBeenCalledWith("North");
  });

  it("presses every handle and drives every direction exactly once", () => {
    const { container } = render(<ResizeBorders />);
    const handles = Array.from(container.querySelectorAll("div"));
    for (const handle of handles) {
      handle.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    }

    const directions = startResizeDragging.mock.calls.map((call) => call[0]);
    expect(directions.sort()).toEqual(
      ["East", "North", "NorthEast", "NorthWest", "South", "SouthEast", "SouthWest", "West"].sort(),
    );
  });
});

describe("ResizeBorders outside Tauri", () => {
  it("never touches the window API when isTauri() is false", async () => {
    const ipc = await import("@/lib/ipc");
    vi.mocked(ipc.isTauri).mockReturnValue(false);

    const { container } = render(<ResizeBorders />);
    const handle = container.querySelector("div");
    handle?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));

    expect(startResizeDragging).not.toHaveBeenCalled();
    vi.mocked(ipc.isTauri).mockReturnValue(true);
  });
});
