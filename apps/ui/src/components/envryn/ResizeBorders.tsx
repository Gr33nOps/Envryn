import type * as React from "react";
import * as ipc from "@/lib/ipc";

/**
 * `decorations: false` (see `TitleBar.tsx`'s doc comment) also removes
 * Windows' native resize hit-testing at the window edges -- without this,
 * an undecorated window could only be resized by dragging the maximize
 * button, which is not how anyone expects a desktop window to behave.
 * Eight thin, invisible strips reintroduce it via Tauri's own
 * `startResizeDragging`, the same mechanism the OS would otherwise provide.
 */
const EDGE = "6px";
const CORNER = "12px";

type ResizeDirection =
  "North" | "South" | "East" | "West" | "NorthEast" | "NorthWest" | "SouthEast" | "SouthWest";

function startResize(direction: ResizeDirection) {
  if (!ipc.isTauri()) return;
  void import("@tauri-apps/api/window").then(({ getCurrentWindow }) =>
    getCurrentWindow()
      .startResizeDragging(direction)
      .catch(() => {}),
  );
}

function Handle({
  direction,
  cursor,
  style,
}: Readonly<{ direction: ResizeDirection; cursor: string; style: React.CSSProperties }>) {
  return (
    <div
      className="fixed z-50"
      style={{ ...style, cursor }}
      onMouseDown={() => startResize(direction)}
    />
  );
}

export function ResizeBorders() {
  return (
    <>
      <Handle
        direction="North"
        cursor="n-resize"
        style={{ top: 0, left: CORNER, right: CORNER, height: EDGE }}
      />
      <Handle
        direction="South"
        cursor="s-resize"
        style={{ bottom: 0, left: CORNER, right: CORNER, height: EDGE }}
      />
      <Handle
        direction="West"
        cursor="w-resize"
        style={{ left: 0, top: CORNER, bottom: CORNER, width: EDGE }}
      />
      <Handle
        direction="East"
        cursor="e-resize"
        style={{ right: 0, top: CORNER, bottom: CORNER, width: EDGE }}
      />
      <Handle
        direction="NorthWest"
        cursor="nw-resize"
        style={{ top: 0, left: 0, width: CORNER, height: CORNER }}
      />
      <Handle
        direction="NorthEast"
        cursor="ne-resize"
        style={{ top: 0, right: 0, width: CORNER, height: CORNER }}
      />
      <Handle
        direction="SouthWest"
        cursor="sw-resize"
        style={{ bottom: 0, left: 0, width: CORNER, height: CORNER }}
      />
      <Handle
        direction="SouthEast"
        cursor="se-resize"
        style={{ bottom: 0, right: 0, width: CORNER, height: CORNER }}
      />
    </>
  );
}
