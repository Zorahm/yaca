/**
 * Layers of the window that lie over the browser's page.
 *
 * The page is a native webview over the window's own, and nothing the
 * window draws can be put on top of it: a dialog over the page would be
 * drawn under it. So whatever covers the window — the settings, the image
 * viewer, a file opened wide — says so here, and the page steps out of
 * sight until the last of them is gone.
 */

import { useEffect, useSyncExternalStore } from "react";

let covers = 0;
const listeners = new Set<() => void>();

function change(delta: number) {
  covers += delta;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Covers the page while `active`. */
export function useBrowserCover(active: boolean) {
  useEffect(() => {
    if (!active) return;
    change(1);
    return () => change(-1);
  }, [active]);
}

/** Whether anything covers the page now. */
export function useCovered(): boolean {
  return useSyncExternalStore(subscribe, () => covers > 0);
}
