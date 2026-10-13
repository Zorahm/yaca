/**
 * Widths of the two columns a person can drag: the sidebar on the left and
 * the panel on the right.
 *
 * The feed is what the window is for, so it is the one column with a floor
 * nobody can drag through: both others give way to it. When the window is too
 * narrow to keep that floor with the panel beside the feed, the panel stops
 * being a column and lies over the feed instead (`panelBounds` says so with
 * `null`).
 *
 * The widths are the window's own business, like a draft in the composer: the
 * core does not know them, and they are kept in `localStorage`.
 */

import { useCallback, useEffect, useState } from "react";

/** The narrowest the feed is ever left: a diff line still reads at this. */
export const FEED_MIN = 480;

export const SIDE = { min: 200, max: 420, initial: 248 } as const;

export const PANEL = { min: 320, initial: 380 } as const;

/** A file is opened to be read, so its first width is a share of the window. */
export const FILE_SHARE = 0.55;

export type Bounds = { min: number; max: number };

/** `min` wins when the bounds cross: a column never collapses to nothing. */
export function clamp(width: number, { min, max }: Bounds): number {
  return Math.round(Math.max(min, Math.min(max, width)));
}

export function sideBounds(windowWidth: number): Bounds {
  return { min: SIDE.min, max: Math.min(SIDE.max, windowWidth - FEED_MIN) };
}

/** `null` — there is no room beside the feed, and the panel lies over it. */
export function panelBounds(windowWidth: number, sideWidth: number): Bounds | null {
  const max = windowWidth - sideWidth - FEED_MIN;
  return max < PANEL.min ? null : { min: PANEL.min, max };
}

/** A width a person chose, or `null` until they choose one. */
export function useStoredWidth(key: string): [number | null, (width: number) => void] {
  const [width, setWidth] = useState(() => parseWidth(localStorage.getItem(key)));
  const store = useCallback(
    (next: number) => {
      setWidth(next);
      localStorage.setItem(key, String(next));
    },
    [key],
  );
  return [width, store];
}

export function parseWidth(raw: string | null): number | null {
  if (raw === null) return null;
  const width = Number(raw);
  return Number.isFinite(width) && width > 0 ? width : null;
}

export function useWindowWidth(): number {
  const [width, setWidth] = useState(() => window.innerWidth);
  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return width;
}
