/**
 * The handle on the edge of a resizable column.
 *
 * It resizes its parent element: the column renders the handle inside itself
 * and sets its own width from `width`. While the pointer is down the handle
 * writes the width straight to that element and tells the window only on
 * release. A drag is sixty events a second, and sending each through the
 * window would redraw the feed for every pixel.
 *
 * `edge` is the side of the column the handle sits on, which is also the side
 * that moves: the sidebar grows to the right, the panel to the left.
 */

import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";

import { clamp, type Bounds } from "./resize";

/** One press of an arrow key. */
const KEY_STEP = 16;

export function Splitter({
  edge,
  width,
  bounds,
  initial,
  label,
  onResize,
}: {
  edge: "left" | "right";
  width: number;
  bounds: Bounds;
  /** Where a double click returns the column. */
  initial: number;
  label: string;
  onResize: (width: number) => void;
}) {
  const drag = useRef<{ x: number; from: number; now: number } | null>(null);
  const [dragging, setDragging] = useState(false);
  const outward = edge === "right" ? 1 : -1;

  function start(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    // Without this the drag selects the text it passes over.
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { x: event.clientX, from: width, now: width };
    setDragging(true);
  }

  function move(event: PointerEvent<HTMLDivElement>) {
    const state = drag.current;
    const column = event.currentTarget.parentElement;
    if (state === null || column === null) return;
    state.now = clamp(state.from + outward * (event.clientX - state.x), bounds);
    column.style.width = `${state.now}px`;
  }

  function finish(event: PointerEvent<HTMLDivElement>, keep: boolean) {
    const state = drag.current;
    if (state === null) return;
    drag.current = null;
    setDragging(false);
    // A click that moved nothing is not a choice of width.
    if (keep) return state.now !== state.from ? onResize(state.now) : undefined;
    const column = event.currentTarget.parentElement;
    if (column !== null) column.style.width = `${state.from}px`;
  }

  function onKey(event: KeyboardEvent<HTMLDivElement>) {
    const next =
      event.key === "ArrowLeft"
        ? width - outward * KEY_STEP
        : event.key === "ArrowRight"
          ? width + outward * KEY_STEP
          : event.key === "Home"
            ? bounds.min
            : event.key === "End"
              ? bounds.max
              : null;
    if (next === null) return;
    event.preventDefault();
    onResize(clamp(next, bounds));
  }

  return (
    <div
      className={`splitter splitter-${edge}${dragging ? " splitter-on" : ""}`}
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={width}
      aria-valuemin={bounds.min}
      aria-valuemax={bounds.max}
      tabIndex={0}
      onPointerDown={start}
      onPointerMove={move}
      onPointerUp={(event) => finish(event, true)}
      onPointerCancel={(event) => finish(event, false)}
      onDoubleClick={() => onResize(clamp(initial, bounds))}
      onKeyDown={onKey}
    />
  );
}
