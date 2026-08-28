import * as React from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as ipc from "@/lib/ipc";
import { Wordmark } from "./Logo";

/**
 * Custom window chrome. `decorations: false` in `tauri.conf.json` removes
 * the native Windows title bar and border entirely -- this replaces it, the
 * same way most modern desktop apps (VS Code, Discord, Slack) do, rather
 * than leaving the window looking unfinished.
 *
 * The one branding mark in the whole app lives here, small and restrained
 * (17px mark, 13px wordmark -- `Wordmark`'s own defaults) at the far left,
 * ahead of the drag region. `Sidebar.tsx` used to carry this instead; moved
 * here so the sidebar can lead with the user's own vault, not the app's
 * name, and so the window reads as one native-feeling piece of chrome with
 * its identity in the conventional title-bar position rather than repeated
 * below it.
 *
 * Outside Tauri (`npm run dev` in a plain browser) there is no real window
 * to control, so every handler below is a no-op guarded by `ipc.isTauri()`
 * -- matching the same guard other Tauri-only features use elsewhere.
 */
export function TitleBar() {
  const [maximized, setMaximized] = React.useState(false);

  React.useEffect(() => {
    if (!ipc.isTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    (async () => {
      const win = getCurrentWindow();
      const refresh = () => {
        win
          .isMaximized()
          .then((v) => !cancelled && setMaximized(v))
          .catch(() => {});
      };
      refresh();
      const stop = await win.onResized(refresh);
      if (cancelled) {
        stop();
      } else {
        unlisten = stop;
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  async function withWindow(fn: (win: ReturnType<typeof getCurrentWindow>) => Promise<void>) {
    if (!ipc.isTauri()) return;
    await fn(getCurrentWindow()).catch(() => {});
  }

  return (
    <div
      data-tauri-drag-region
      onDoubleClick={() => void withWindow((win) => win.toggleMaximize())}
      className="desktop-titlebar flex h-8 shrink-0 select-none items-center justify-between border-b border-border bg-surface"
    >
      <div className="flex h-full shrink-0 items-center pl-3">
        <Wordmark size={17} />
      </div>
      <div data-tauri-drag-region className="h-full flex-1" />
      <div className="flex h-full shrink-0 items-stretch">
        <button
          type="button"
          aria-label="Minimize"
          onClick={() => void withWindow((win) => win.minimize())}
          className="inline-flex w-11 items-center justify-center text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
        >
          <Minus className="size-3.5" />
        </button>
        <button
          type="button"
          aria-label={maximized ? "Restore" : "Maximize"}
          onClick={() => void withWindow((win) => win.toggleMaximize())}
          className="inline-flex w-11 items-center justify-center text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
        >
          {maximized ? (
            <Copy className="size-3" />
          ) : (
            <Square className="size-3" strokeWidth={1.75} />
          )}
        </button>
        <button
          type="button"
          aria-label="Close"
          onClick={() => void withWindow((win) => win.close())}
          className="inline-flex w-11 items-center justify-center text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
        >
          <X className="size-3.5" />
        </button>
      </div>
    </div>
  );
}
