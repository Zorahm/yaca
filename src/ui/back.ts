/**
 * The system Back closes the layer on top: the drawer, a panel over the feed,
 * the settings. That is what Back does in every Android app, and a phone's
 * browser hands it to the page as a step back through its history.
 *
 * Each open layer holds one history entry the page pushed itself. Back pops
 * the entry, and the layer on top closes. A layer closed from the page (its
 * own button, a tap on the scrim) gives its entry back, so entries never
 * outnumber layers and Back never lands on a step that closes nothing.
 *
 * The entries are squared with the layers once per turn of the event loop,
 * not per layer. One layer often closes as the next opens — the drawer shuts
 * as the settings it led to come up — and a step back taken for the first
 * while the second pushes its entry is a race the browser settles its own
 * way: the second layer lost its entry, and the next Back left the page. Done
 * together, the two cancel out and the history is not touched at all.
 */

import { useEffect, useRef } from "react";

type Layer = { close: () => void };

const stack: Layer[] = [];
/** Entries pushed for the layers and not yet popped. */
let depth = 0;
/** Steps back the page took itself, whose `popstate` is not the human's. */
let own = 0;
let pending = false;
let listening = false;

function onPop() {
  if (own > 0) {
    own -= 1;
    return;
  }
  const layer = stack.pop();
  if (layer === undefined) return;
  depth -= 1;
  layer.close();
}

function square() {
  pending = false;
  const want = stack.length;
  if (want > depth) {
    for (let i = depth; i < want; i += 1) history.pushState(history.state, "");
  } else if (want < depth) {
    own += 1;
    history.go(want - depth);
  }
  depth = want;
}

function schedule() {
  if (pending) return;
  pending = true;
  queueMicrotask(square);
}

/** While `open`, Back calls `close` instead of leaving the page. */
export function useBackClose(open: boolean, close: () => void): void {
  const latest = useRef(close);
  latest.current = close;

  useEffect(() => {
    if (!open) return;
    if (!listening) {
      window.addEventListener("popstate", onPop);
      listening = true;
    }
    let popped = false;
    const layer: Layer = {
      close: () => {
        popped = true;
        latest.current();
      },
    };
    stack.push(layer);
    schedule();
    return () => {
      if (popped) return;
      const at = stack.indexOf(layer);
      if (at === -1) return;
      stack.splice(at, 1);
      schedule();
    };
  }, [open]);
}
