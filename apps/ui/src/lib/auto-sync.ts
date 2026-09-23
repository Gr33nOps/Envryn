/**
 * Background sync with paired devices, plus the shared result wording.
 *
 * Why this exists: a phone often cannot open a connection *into* a PC (the PC
 * advertises virtual-adapter addresses, or a firewall blocks inbound), while
 * the PC can always reach the phone. Sync sessions are two-way, so whenever
 * either device manages to connect, both end up with each other's changes.
 * Running sync automatically on both sides means two open, paired devices
 * simply agree -- whichever direction works carries the other device's edits
 * too -- instead of depending on someone pressing "Sync now" on the one side
 * that happens to be reachable.
 */
import * as React from "react";
import * as ipc from "./ipc";

/** How often to look for paired devices while the app is open and visible. */
export const AUTO_SYNC_INTERVAL_MS = 30_000;

/**
 * Count both directions. The old message counted only what this device
 * *received*, so the device that pushed changes always said "0 updated".
 */
export function syncResultMessage(sent: number, received: number): string {
  const plural = (n: number) => `${n} secret${n === 1 ? "" : "s"}`;
  if (sent === 0 && received === 0) return "Everything is already up to date";
  if (sent > 0 && received > 0) return `Sent ${plural(sent)}, received ${plural(received)}`;
  if (sent > 0) return `Sent ${plural(sent)}`;
  return `Received ${plural(received)}`;
}

/**
 * One background pass: find trusted devices on the network and sync with
 * each. Silent by design -- a device that is off or unreachable is the
 * normal case, not an error. Received changes still surface, through the
 * "Vault updated from another device" notice the backend triggers whenever a
 * sync applies anything.
 */
export async function runAutoSyncPass(): Promise<void> {
  const settings = await ipc.settingsGet();
  if (!settings.auto_sync) return;
  const devices = await ipc.trustedDeviceList();
  if (devices.length === 0) return;
  const peers = await ipc.discoveryBrowse();
  for (const device of devices) {
    const peer = peers.find((p) => p.device_id === device.device_id);
    if (!peer || peer.addresses.length === 0) continue;
    try {
      await ipc.syncPeer(peer.addresses, peer.port);
    } catch {
      // Unreachable right now; the next pass tries again.
    }
  }
}

/**
 * Run [`runAutoSyncPass`] shortly after unlock, every
 * [`AUTO_SYNC_INTERVAL_MS`] while the app is visible, and again whenever it
 * comes back to the foreground. Never runs while hidden and never overlaps
 * with itself. The setting is re-read on every pass, so turning it off in
 * Settings takes effect without a restart.
 */
export function useAutoSync(): void {
  React.useEffect(() => {
    if (!ipc.isTauri()) return;
    let stopped = false;
    let running = false;

    const tick = async () => {
      if (stopped || running || document.hidden) return;
      running = true;
      try {
        await runAutoSyncPass();
      } catch {
        // Settings or discovery unavailable this time; try again later.
      } finally {
        running = false;
      }
    };

    const first = window.setTimeout(() => void tick(), 2_000);
    const interval = window.setInterval(() => void tick(), AUTO_SYNC_INTERVAL_MS);
    const onVisibility = () => {
      if (!document.hidden) void tick();
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      stopped = true;
      window.clearTimeout(first);
      window.clearInterval(interval);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, []);
}
