import { useRef, useState, type PointerEvent } from "react";

import { isClick, panned, pinched, type Point, type Size, type View } from "./zoom";

type Change = (next: (view: View, size: Size, box: Size) => View) => void;

export function useImageGestures(
  fromCentre: (event: { clientX: number; clientY: number }) => Point,
  change: Change,
  onTap: (at: Point) => boolean,
  onDoubleTap: (at: Point) => void,
) {
  const pointers = useRef(new Map<number, Point>());
  const pressed = useRef<Point | null>(null);
  const lastTap = useRef<{ at: Point; time: number } | null>(null);
  const [dragging, setDragging] = useState(false);
  const lastPointerType = useRef("");

  function reset() {
    pointers.current.clear();
    pressed.current = null;
    lastTap.current = null;
    setDragging(false);
  }

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    lastPointerType.current = event.pointerType;
    if (event.button !== 0 || pointers.current.size >= 2) return;
    if (event.pointerType !== "touch" && pointers.current.size > 0) return;
    const at = fromCentre(event);
    event.currentTarget.setPointerCapture(event.pointerId);
    pressed.current = pointers.current.size === 0 ? at : null;
    if (pressed.current === null || event.pointerType !== "touch") lastTap.current = null;
    pointers.current.set(event.pointerId, at);
    setDragging(true);
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>) {
    const from = pointers.current.get(event.pointerId);
    if (from === undefined) return;
    const to = fromCentre(event);
    if (pressed.current !== null) {
      if (isClick(pressed.current, to)) return;
      pressed.current = null;
      lastTap.current = null;
    }
    const other = Array.from(pointers.current.entries()).find(([id]) => id !== event.pointerId)?.[1];
    pointers.current.set(event.pointerId, to);
    change((v, s, b) => other === undefined
      ? panned(v, to.x - from.x, to.y - from.y, s, b)
      : pinched(v, [from, other], [to, other], s, b));
  }

  function release(event: PointerEvent<HTMLDivElement>) {
    if (!pointers.current.delete(event.pointerId)) return false;
    pressed.current = null;
    if (pointers.current.size === 0) setDragging(false);
    return true;
  }

  function onPointerUp(event: PointerEvent<HTMLDivElement>) {
    const from = pressed.current;
    const at = fromCentre(event);
    if (!release(event) || from === null || !isClick(from, at) || !onTap(at)) return;
    if (event.pointerType !== "touch") return;
    const last = lastTap.current;
    if (last !== null && event.timeStamp - last.time <= 300 &&
      Math.hypot(at.x - last.at.x, at.y - last.at.y) <= 24) {
      lastTap.current = null;
      onDoubleTap(at);
    } else {
      lastTap.current = { at, time: event.timeStamp };
    }
  }

  function cancel(event: PointerEvent<HTMLDivElement>) {
    if (release(event)) lastTap.current = null;
  }

  return {
    dragging,
    reset,
    canOpenMenu: () => pointers.current.size === 0 || pressed.current !== null,
    isTouch: () => lastPointerType.current === "touch",
    handlers: {
      onPointerDown,
      onPointerMove,
      onPointerUp,
      onPointerCancel: cancel,
      onLostPointerCapture: cancel,
    },
  };
}
