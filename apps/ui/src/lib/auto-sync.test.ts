import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import {
  AUTO_SYNC_INTERVAL_MS,
  runAutoSyncPass,
  syncResultMessage,
  useAutoSync,
} from "./auto-sync";

const settingsGet = vi.fn();
const trustedDeviceList = vi.fn();
const discoveryBrowse = vi.fn();
const syncPeer = vi.fn();
const isTauri = vi.fn();

vi.mock("./ipc", async () => {
  const actual = await vi.importActual<typeof import("./ipc")>("./ipc");
  return {
    ...actual,
    settingsGet: (...args: unknown[]) => settingsGet(...args),
    trustedDeviceList: (...args: unknown[]) => trustedDeviceList(...args),
    discoveryBrowse: (...args: unknown[]) => discoveryBrowse(...args),
    syncPeer: (...args: unknown[]) => syncPeer(...args),
    isTauri: () => isTauri(),
  };
});

beforeEach(() => {
  for (const mock of [settingsGet, trustedDeviceList, discoveryBrowse, syncPeer, isTauri]) {
    mock.mockReset();
  }
  settingsGet.mockResolvedValue({ auto_sync: true });
  trustedDeviceList.mockResolvedValue([{ device_id: "phone" }, { device_id: "laptop" }]);
  discoveryBrowse.mockResolvedValue([
    { device_id: "phone", addresses: ["192.168.1.20"], port: 7000 },
    { device_id: "stranger", addresses: ["192.168.1.30"], port: 7000 },
  ]);
  syncPeer.mockResolvedValue({ records_applied: 0, records_sent: 0, conflicts: 0 });
});

describe("syncResultMessage", () => {
  it("counts both directions instead of reporting 0 after a push", () => {
    expect(syncResultMessage(0, 0)).toBe("Everything is already up to date");
    expect(syncResultMessage(2, 0)).toBe("Sent 2 secrets");
    expect(syncResultMessage(0, 1)).toBe("Received 1 secret");
    expect(syncResultMessage(1, 3)).toBe("Sent 1 secret, received 3 secrets");
  });
});

describe("runAutoSyncPass", () => {
  it("syncs only trusted devices that are currently on the network", async () => {
    await runAutoSyncPass();
    expect(syncPeer).toHaveBeenCalledTimes(1);
    expect(syncPeer).toHaveBeenCalledWith(["192.168.1.20"], 7000);
  });

  it("does nothing when automatic sync is turned off", async () => {
    settingsGet.mockResolvedValue({ auto_sync: false });
    await runAutoSyncPass();
    expect(trustedDeviceList).not.toHaveBeenCalled();
    expect(syncPeer).not.toHaveBeenCalled();
  });

  it("skips discovery when no device is paired", async () => {
    trustedDeviceList.mockResolvedValue([]);
    await runAutoSyncPass();
    expect(discoveryBrowse).not.toHaveBeenCalled();
  });

  it("skips a peer with no address and keeps going after an unreachable one", async () => {
    discoveryBrowse.mockResolvedValue([
      { device_id: "phone", addresses: [], port: 7000 },
      { device_id: "laptop", addresses: ["192.168.1.40"], port: 7001 },
    ]);
    trustedDeviceList.mockResolvedValue([
      { device_id: "phone" },
      { device_id: "laptop" },
      { device_id: "tablet" },
    ]);
    syncPeer.mockRejectedValueOnce(new Error("unreachable"));
    await expect(runAutoSyncPass()).resolves.toBeUndefined();
    expect(syncPeer).toHaveBeenCalledTimes(1);
    expect(syncPeer).toHaveBeenCalledWith(["192.168.1.40"], 7001);
  });
});

describe("useAutoSync", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("syncs shortly after mounting and then on every interval, until unmounted", async () => {
    isTauri.mockReturnValue(true);
    const { unmount } = renderHook(() => useAutoSync());
    await vi.advanceTimersByTimeAsync(2_000);
    expect(syncPeer).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(AUTO_SYNC_INTERVAL_MS);
    expect(syncPeer).toHaveBeenCalledTimes(2);
    unmount();
    await vi.advanceTimersByTimeAsync(AUTO_SYNC_INTERVAL_MS * 2);
    expect(syncPeer).toHaveBeenCalledTimes(2);
  });

  it("stays silent when a pass fails, and never runs outside the desktop or phone app", async () => {
    isTauri.mockReturnValue(true);
    settingsGet.mockRejectedValue(new Error("locked"));
    const { unmount } = renderHook(() => useAutoSync());
    await vi.advanceTimersByTimeAsync(2_000);
    expect(syncPeer).not.toHaveBeenCalled();
    unmount();

    isTauri.mockReturnValue(false);
    settingsGet.mockClear();
    renderHook(() => useAutoSync());
    await vi.advanceTimersByTimeAsync(AUTO_SYNC_INTERVAL_MS);
    expect(settingsGet).not.toHaveBeenCalled();
  });

  it("does not sync while the app is in the background, and catches up when it returns", async () => {
    isTauri.mockReturnValue(true);
    const hidden = vi.spyOn(document, "hidden", "get").mockReturnValue(true);
    renderHook(() => useAutoSync());
    await vi.advanceTimersByTimeAsync(AUTO_SYNC_INTERVAL_MS);
    expect(syncPeer).not.toHaveBeenCalled();

    hidden.mockReturnValue(false);
    document.dispatchEvent(new Event("visibilitychange"));
    await vi.advanceTimersByTimeAsync(0);
    expect(syncPeer).toHaveBeenCalledTimes(1);
    hidden.mockRestore();
  });
});
