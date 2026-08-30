import { describe, expect, it, vi } from "vitest";
import { preferredPeerAddresses, syncDiscoveredPeer } from "./sync-peer";

const peer = {
  device_id: "peer-1",
  fingerprint_hex: "00".repeat(32),
  addresses: ["100.98.136.91", "192.168.1.25"],
  port: 42_424,
};

describe("syncDiscoveredPeer", () => {
  it("prefers an ordinary LAN address over VPN and IPv6 adapters", () => {
    expect(
      preferredPeerAddresses(["fe80::1", "100.98.136.91", "192.168.1.25", "10.0.0.4"]),
    ).toEqual(["192.168.1.25", "10.0.0.4", "100.98.136.91", "fe80::1"]);
  });

  it("tries the next discovered address when the first cannot connect", async () => {
    const connect = vi
      .fn()
      .mockRejectedValueOnce(new Error("unreachable"))
      .mockResolvedValueOnce({ records_applied: 3, conflicts: 0 });

    const result = await syncDiscoveredPeer(
      { ...peer, addresses: ["192.168.1.25", "192.168.1.26"] },
      connect,
    );

    expect(connect).toHaveBeenNthCalledWith(1, "192.168.1.25", 42_424);
    expect(connect).toHaveBeenNthCalledWith(2, "192.168.1.26", 42_424);
    expect(result.records_applied).toBe(3);
  });

  it("does not retry duplicate addresses", async () => {
    const connect = vi.fn().mockRejectedValue(new Error("unreachable"));
    await expect(
      syncDiscoveredPeer({ ...peer, addresses: ["192.168.1.25", "192.168.1.25"] }, connect),
    ).rejects.toThrow("unreachable");
    expect(connect).toHaveBeenCalledTimes(1);
  });
});
