import type { DiscoveredPeer, SyncSummary } from "@envryn/contract";

type Connect = (address: string, port: number) => Promise<SyncSummary>;

function addressPriority(address: string): number {
  // Ordinary home/office IPv4 LANs are the most likely path between a phone
  // and PC. Carrier-grade/Tailscale space and IPv6 remain valid fallbacks,
  // but virtual adapters often publish them ahead of the reachable LAN IP.
  if (/^10\./.test(address) || /^192\.168\./.test(address)) return 0;
  const match = /^172\.(\d+)\./.exec(address);
  if (match && Number(match[1]) >= 16 && Number(match[1]) <= 31) return 0;
  if (/^100\.(6[4-9]|[789]\d|1[01]\d|12[0-7])\./.test(address)) return 2;
  if (address.includes(":")) return 3;
  return 1;
}

export function preferredPeerAddresses(addresses: readonly string[]): string[] {
  return [...new Set(addresses)].sort(
    (left, right) => addressPriority(left) - addressPriority(right),
  );
}

/**
 * Try every address advertised for a peer. Multi-homed PCs commonly expose
 * Ethernet, Wi-Fi, VPN, Hyper-V and IPv6 addresses together, and discovery
 * order is not a reachability guarantee.
 */
export async function syncDiscoveredPeer(
  peer: DiscoveredPeer,
  connect: Connect,
): Promise<SyncSummary> {
  let lastError: unknown = new Error("The device did not advertise a usable address.");
  for (const address of preferredPeerAddresses(peer.addresses)) {
    try {
      return await connect(address, peer.port);
    } catch (error) {
      lastError = error;
    }
  }
  throw lastError;
}
