/**
 * Widths of the columns a person can drag: the sidebar on the left, and the
 * browser and the panel on the right.
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

/**
 * Below this the window is a phone, or a desktop window dragged that thin, and
 * the columns stop being columns: the sidebar is a drawer that slides over the
 * feed and a panel takes the whole screen. Beside each other they would leave
 * the feed less than a line of code wide.
 */
export const COMPACT_BELOW = 720;

export const isCompact = (windowWidth: number): boolean => windowWidth < COMPACT_BELOW;

/** The narrowest the feed is ever left: a diff line still reads at this. */
export const FEED_MIN = 480;

export const SIDE = { min: 200, max: 420, initial: 248 } as const;

export const PANEL = { min: 320, initial: 380 } as const;

/** A file is opened to be read, so its first width is a share of the window. */
export const FILE_SHARE = 0.55;

/** The browser: narrow enough to sit beside the feed, wide enough for a page. */
export const BROWSER = { min: 360 } as const;

/** A page starts at a share of the window, so a desktop layout looks like one. */
export const BROWSER_SHARE = 0.4;

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

/**
 * The browser column's bounds, keeping `beside` for the panel next to it.
 * `null`: no room beside the feed, and the browser lies over it.
 */
export function browserBounds(windowWidth: number, sideWidth: number, beside = 0): Bounds | null {
  const max = windowWidth - sideWidth - FEED_MIN - beside;
  return max < BROWSER.min ? null : { min: BROWSER.min, max };
}

/**
 * Whether the browser and the panel both fit beside the feed. Where they do
 * not, one of them is open at a time.
 */
export function bothFit(windowWidth: number, sideWidth: number): boolean {
  return windowWidth - sideWidth - FEED_MIN >= BROWSER.min + PANEL.min;
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
