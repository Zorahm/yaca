/**
 * The window's own frame: whether Yaca draws its title bar, and the three
 * things a title bar does to a window.
 *
 * Only on Windows. There the system bar is a strip that holds a name and
 * nothing else, and a menu can only go under it as a second strip; drawn by
 * the window itself, the two are one. On macOS the bar is where the traffic
 * lights live and the menu is at the top of the screen, and on Linux a window
 * without decorations resizes unreliably — both keep the system's
 * (`src-tauri/tauri.windows.conf.json` is where the decorations come off, and
 * it is read on Windows alone).
 */

import { inTauri } from "./demo";

/** Does this window draw its own title bar? True in the stand too, on
    Windows, so the bar can be looked at without building the shell. */
export const ownTitleBar =
  typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");

export type WindowControls = {
  minimize: () => void;
  toggleMaximize: () => void;
  close: () => void;
  /** Tells `handler` whether the window is maximized, now and on every change;
      returns the way to stop. */
  watchMaximized: (handler: (maximized: boolean) => void) => () => void;
};

/**
 * The buttons' side of the frame, or `null` where there is no window to
 * control: in the stand the browser has its own, and three buttons that did
 * nothing would be the interface lying about what it can do (§0).
 */
export async function windowControls(): Promise<WindowControls | null> {
  if (!inTauri || !ownTitleBar) return null;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const current = getCurrentWindow();
  const failed = (e: unknown) => console.error("window failed to subscribe", e);

  return {
    minimize: () => void current.minimize().catch(failed),
    toggleMaximize: () => void current.toggleMaximize().catch(failed),
    close: () => void current.close().catch(failed),
    watchMaximized: (handler) => {
      let live = true;
      const tell = () =>
        void current.isMaximized().then((maximized) => {
          if (live) handler(maximized);
        }, failed);
      tell();
      // Maximizing is a resize like any other; there is no event of its own.
      const off = current.onResized(tell);
      return () => {
        live = false;
        void off.then((stop) => stop(), failed);
      };
    },
  };
}
