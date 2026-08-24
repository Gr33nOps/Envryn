import { toast } from "sonner";

const CLIPBOARD_CLEAR_SECONDS = 30;

let clearTimer: ReturnType<typeof setTimeout> | undefined;
/** The value we put there, so we never clear a clipboard someone else has since filled. */
let ourClipboardValue: string | undefined;

/**
 * Copy a secret value, then clear it again after a delay.
 *
 * Two caveats worth stating plainly rather than implying they are solved:
 *
 * 1. Clearing only works while Envryn is running. Quitting before the timer
 *    fires leaves the value on the clipboard.
 * 2. Windows clipboard-history and third-party clipboard managers may already
 *    have archived the value. Suppressing that requires the
 *    `ExcludeClipboardContentFromMonitorProcessing` format, which has to be set
 *    from the Rust side -- that is M5, and until it lands this function reduces
 *    exposure rather than eliminating it (THREAT_MODEL.md V-08).
 */
export async function copyValue(value: string): Promise<void> {
  if (!value || typeof navigator === "undefined" || !navigator.clipboard) {
    toast("Nothing to copy");
    return;
  }

  await navigator.clipboard.writeText(value);
  ourClipboardValue = value;

  if (clearTimer) clearTimeout(clearTimer);
  clearTimer = setTimeout(() => {
    void (async () => {
      try {
        // Only clear if the clipboard still holds what we put there.
        const current = await navigator.clipboard.readText();
        if (current === ourClipboardValue) await navigator.clipboard.writeText("");
      } catch {
        // Reading the clipboard can be denied. Clearing unconditionally would
        // then destroy whatever the user copied since, so we leave it alone.
      } finally {
        ourClipboardValue = undefined;
      }
    })();
  }, CLIPBOARD_CLEAR_SECONDS * 1000);

  toast("Secret copied", {
    description: `Envryn clears the clipboard in ${CLIPBOARD_CLEAR_SECONDS} seconds.`,
  });
}

/** Cancel a pending clear and forget the value. Called on lock. */
export function forgetClipboardTimer(): void {
  if (clearTimer) clearTimeout(clearTimer);
  clearTimer = undefined;
  ourClipboardValue = undefined;
}
