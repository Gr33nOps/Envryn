import { toast } from "sonner";
import { clipboardCopy, settingsGet, IpcError } from "./ipc";

/**
 * Copy a secret value to the clipboard.
 *
 * The actual copy, tagging, and timed clear all happen in the Rust process
 * (`clipboard_copy` in src-tauri/src/ipc.rs), not here. That matters for two
 * reasons `navigator.clipboard` cannot provide: only the native Win32 call
 * can tag the write so Windows clipboard history and cloud clipboard sync
 * skip it (THREAT_MODEL.md V-08), and a timer living in the Rust process
 * keeps running regardless of what this webview's JS event loop is doing --
 * including while a modal, a slow render, or a background tab throttles it.
 */
export async function copyValue(value: string): Promise<void> {
  if (!value) {
    toast("Nothing to copy");
    return;
  }

  try {
    await clipboardCopy(value);
  } catch (err) {
    toast(err instanceof IpcError ? err.message : "That could not be copied.");
    return;
  }

  const { clipboard_clear_seconds } = await settingsGet().catch(() => ({
    clipboard_clear_seconds: 30,
  }));

  toast("Secret copied", {
    description: `Envryn clears the clipboard in ${clipboard_clear_seconds} seconds.`,
  });
}

/**
 * Nothing to cancel any more -- the clear timer lives in the Rust process and
 * outlives this function's old JS-side timer entirely. Kept as a no-op call
 * site so `lockVault()` does not need to change shape, and so a future
 * lock-triggered clipboard wipe (clearing immediately on lock, rather than
 * waiting out the timer) has an obvious place to go.
 */
export function forgetClipboardTimer(): void {}
