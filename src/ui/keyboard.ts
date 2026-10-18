/**
 * Keyboard.
 *
 * Only the shortcuts that have an action behind them. `⌘K`, `⌘N`, `⌘F` and
 * `⌘,` lead to the command palette, the session list, search and settings; none
 * of those is in this slice, and a shortcut that does nothing is no better than
 * a button that does nothing.
 *
 * `Esc Esc` rolls back to the previous message: it finds the last checkpoint
 * taken before that message and shows the file list. The person confirms the
 * rollback itself. `Esc` pressed inside the right panel does not reach here: the
 * panel closes itself and stops the event.
 */

import { useEffect, useRef } from "react";

/** How soon a second `Esc` must follow the first to count as the second. */
const DOUBLE_ESC_MS = 600;

export type Shortcuts = {
  stop: () => void;
  rollbackToPreviousMessage: () => void;
  nextMode: () => void;
  /** While a permission request is pending, the keys belong to it. */
  suspended: boolean;
  /** The right panel is open. */
  panelOpen: boolean;
  /** `⌘⇧O` — close the right panel. */
  closePanel: () => void;
  /** `⌥←` / `⌥→` — the adjacent file in the file panel. */
  prevFile: () => void;
  nextFile: () => void;
};

export function useShortcuts(shortcuts: Shortcuts): void {
  const lastEsc = useRef(0);
  const current = useRef(shortcuts);
  current.current = shortcuts;

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const s = current.current;
      if (s.suspended) return;

      const meta = event.ctrlKey || event.metaKey;

      if (event.key === "Escape") {
        const now = Date.now();
        if (now - lastEsc.current < DOUBLE_ESC_MS) {
          lastEsc.current = 0;
          event.preventDefault();
          s.rollbackToPreviousMessage();
          return;
        }
        lastEsc.current = now;
        s.stop();
        return;
      }

      if (s.panelOpen && meta && event.shiftKey && event.key.toUpperCase() === "O") {
        event.preventDefault();
        s.closePanel();
        return;
      }

      if (s.panelOpen && event.altKey && !meta) {
        if (event.key === "ArrowLeft") {
          event.preventDefault();
          s.prevFile();
          return;
        }
        if (event.key === "ArrowRight") {
          event.preventDefault();
          s.nextFile();
          return;
        }
      }

      // A bare `⇥` is not intercepted: only one mode axis is left, so there is
      // no longer a reason to take the plain focus move away from the field.
      if (event.key === "Tab" && event.shiftKey && inComposer(event.target)) {
        event.preventDefault();
        s.nextMode();
      }
    }

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

/** `⇧⇥` switches the mode only from the composer: in a list it is focus traversal. */
function inComposer(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && target.tagName === "TEXTAREA";
}
