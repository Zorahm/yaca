/**
 * The drawer follows a finger dragging it left and, let go, either shuts or
 * settles back open, the way an Android navigation drawer does.
 *
 * The drag writes the drawer's offset and the scrim's opacity straight to the
 * two elements, not through React state: a render per pointer move is what
 * makes a dragged surface lag behind the finger. Letting go clears both, and
 * the CSS transition carries the drawer from wherever the finger left it.
 *
 * Only touch drags it. A vertical swipe is the list's scroll and is left to the
 * browser (`touch-action: pan-y` in sidebar.css); the gesture is decided once,
 * by whichever way the finger first travels further.
 *
 * There is no edge swipe to open it: the left edge of the screen is Android's
 * own Back gesture, and a page that fights it for the edge loses either way.
 */

import { useRef, type PointerEvent, type RefObject } from "react";

/** How far the finger travels before the gesture counts as a drag or a scroll. */
const SLOP_PX = 10;
/** A drag past this share of the drawer's width shuts it when let go. */
const SHUT_SHARE = 0.35;
/** A flick faster than this shuts it however short it was, in px per ms. */
const FLICK = 0.5;

type Drag = { id: number; x: number; y: number; at: number; dx: number; axis: "x" | "y" | null };

export function useSwipeShut(
  drawer: RefObject<HTMLElement | null>,
  scrim: RefObject<HTMLElement | null>,
  onShut: () => void,
) {
  const drag = useRef<Drag | null>(null);

  function paint(dx: number | null) {
    const el = drawer.current;
    const veil = scrim.current;
    if (el === null) return;
    if (dx === null) {
      el.style.removeProperty("transform");
      el.style.removeProperty("transition");
      veil?.style.removeProperty("opacity");
      veil?.style.removeProperty("transition");
      return;
    }
    el.style.transition = "none";
    el.style.transform = `translateX(${dx}px)`;
    if (veil !== null) {
      veil.style.transition = "none";
      veil.style.opacity = String(1 + dx / el.offsetWidth);
    }
  }

  function release(event: PointerEvent, shut: boolean) {
    const now = drag.current;
    if (now === null || now.id !== event.pointerId) return;
    drag.current = null;
    if (now.axis !== "x") return;
    paint(null);
    if (shut) onShut();
  }

  return {
    onPointerDown(event: PointerEvent) {
      if (event.pointerType !== "touch" || drag.current !== null) return;
      drag.current = {
        id: event.pointerId,
        x: event.clientX,
        y: event.clientY,
        at: event.timeStamp,
        dx: 0,
        axis: null,
      };
    },
    onPointerMove(event: PointerEvent) {
      const now = drag.current;
      if (now === null || now.id !== event.pointerId || now.axis === "y") return;
      const dx = event.clientX - now.x;
      const dy = event.clientY - now.y;
      if (now.axis === null) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) < SLOP_PX) return;
        now.axis = Math.abs(dx) > Math.abs(dy) ? "x" : "y";
        if (now.axis === "y") return;
        // The rows under the finger must not take the lift as a tap.
        event.currentTarget.setPointerCapture(event.pointerId);
      }
      now.dx = Math.min(0, dx);
      paint(now.dx);
    },
    onPointerUp(event: PointerEvent) {
      const now = drag.current;
      const width = drawer.current?.offsetWidth ?? 1;
      const speed = now === null ? 0 : now.dx / Math.max(1, event.timeStamp - now.at);
      release(event, now !== null && (now.dx < -width * SHUT_SHARE || speed < -FLICK));
    },
    onPointerCancel(event: PointerEvent) {
      release(event, false);
    },
  };
}
