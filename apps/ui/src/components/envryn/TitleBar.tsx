import * as React from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import * as ipc from "@/lib/ipc";
import { LogoMark } from "@/components/envryn/Logo";

/**
 * Custom window chrome. `decorations: false` in `tauri.conf.json` removes
 * the native Windows title bar and border entirely -- this replaces it, the
 * same way most modern desktop apps (VS Code, Discord, Slack) do, rather
 * than leaving the window looking unfinished.
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
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
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

  async function withWindow(fn: (win: Awaited<ReturnType<typeof currentWindow>>) => Promise<void>) {
    if (!ipc.isTauri()) return;
    const win = await currentWindow();
    await fn(win).catch(() => {});
  }

  async function currentWindow() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow();
  }

  return (
    <div
      data-tauri-drag-region
      onDoubleClick={() => void withWindow((win) => win.toggleMaximize())}
      className="flex h-8 shrink-0 select-none items-center justify-between border-b border-border bg-surface pl-3"
    >
      <div data-tauri-drag-region className="flex flex-1 items-center gap-2">
        <LogoMark size={13} />
        <span className="text-[11.5px] font-medium tracking-tight text-muted-foreground">
          Envryn
        </span>
      </div>
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
