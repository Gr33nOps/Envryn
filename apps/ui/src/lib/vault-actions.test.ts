import { describe, expect, it, vi, beforeEach } from "vitest";
import { toast } from "sonner";
import { copyValue } from "./vault-actions";
import { IpcError } from "./ipc";

vi.mock("sonner", () => ({ toast: vi.fn() }));

const clipboardCopy = vi.fn();
const settingsGet = vi.fn();

vi.mock("./ipc", async () => {
  const actual = await vi.importActual<typeof import("./ipc")>("./ipc");
  return {
    ...actual,
    clipboardCopy: (...args: unknown[]) => clipboardCopy(...args),
    settingsGet: (...args: unknown[]) => settingsGet(...args),
  };
});

beforeEach(() => {
  vi.mocked(toast).mockClear();
  clipboardCopy.mockReset();
  settingsGet.mockReset();
});

describe("copyValue", () => {
  it("does nothing and tells the user there is nothing to copy for an empty value", async () => {
    await copyValue("");
    expect(toast).toHaveBeenCalledWith("Nothing to copy");
    expect(clipboardCopy).not.toHaveBeenCalled();
  });

  it("copies a real value and reports the configured clear time", async () => {
    clipboardCopy.mockResolvedValue(undefined);
    settingsGet.mockResolvedValue({ clipboard_clear_seconds: 45 });

    await copyValue("sk-live-abc");

    expect(clipboardCopy).toHaveBeenCalledWith("sk-live-abc");
    expect(toast).toHaveBeenCalledWith(
      "Secret copied",
      expect.objectContaining({ description: "Envryn clears the clipboard in 45 seconds." }),
    );
  });

  it("falls back to a 30 second message when settings cannot be read", async () => {
    clipboardCopy.mockResolvedValue(undefined);
    settingsGet.mockRejectedValue(new Error("locked"));

    await copyValue("sk-live-abc");

    expect(toast).toHaveBeenCalledWith(
      "Secret copied",
      expect.objectContaining({ description: "Envryn clears the clipboard in 30 seconds." }),
    );
  });

  it("shows the real IpcError message and never fetches settings when the copy itself fails", async () => {
    clipboardCopy.mockRejectedValue(new IpcError("internal", "Clipboard is unavailable."));

    await copyValue("sk-live-abc");

    expect(toast).toHaveBeenCalledWith("Clipboard is unavailable.");
    expect(settingsGet).not.toHaveBeenCalled();
  });

  it("shows a generic message for a non-IpcError failure", async () => {
    clipboardCopy.mockRejectedValue(new Error("boom"));

    await copyValue("sk-live-abc");

    expect(toast).toHaveBeenCalledWith("That could not be copied.");
  });
});
