import { describe, expect, it } from "vitest";

import { windowOf } from "./window";

const ROW = 24;

/** What the scroll container is told the list is worth in pixels. */
function totalHeight(w: ReturnType<typeof windowOf>): number {
  return w.padTop + (w.end - w.start) * ROW + w.padBottom;
}

describe("windowOf", () => {
  it("renders a short list whole, without filler", () => {
    const w = windowOf(10, ROW, 0, 380, 8);
    expect(w).toEqual({ start: 0, end: 10, padTop: 0, padBottom: 0 });
  });

  it("keeps the scrollbar honest at any offset", () => {
    // Filler plus rendered rows must always add up to the full list, or the
    // scrollbar reports a height the list does not have.
    for (const scrollTop of [0, 1, 239, 240, 5000, 47_999, 48_000]) {
      const w = windowOf(2000, ROW, scrollTop, 380, 8);
      expect(totalHeight(w)).toBe(2000 * ROW);
    }
  });

  it("renders the rows the viewport is looking at", () => {
    // 380px of viewport at row 100: rows 100..115 are on screen.
    const w = windowOf(2000, ROW, 100 * ROW, 380, 8);
    expect(w.start).toBe(92);
    expect(w.end).toBeGreaterThanOrEqual(116);
    expect(w.padTop).toBe(92 * ROW);
  });

  it("does not run past either end", () => {
    const top = windowOf(2000, ROW, 0, 380, 8);
    expect(top.start).toBe(0);
    expect(top.padTop).toBe(0);

    const bottom = windowOf(2000, ROW, 2000 * ROW, 380, 8);
    expect(bottom.end).toBe(2000);
    expect(bottom.padBottom).toBe(0);
  });

  it("gives a usable first paint before anything is measured", () => {
    // Viewport zero is the frame before the observer fires, not an error.
    const w = windowOf(2000, ROW, 0, 0, 8);
    expect(w.start).toBe(0);
    expect(w.end).toBe(17);
    expect(totalHeight(w)).toBe(2000 * ROW);
  });

  it("survives an empty list and an unmeasured row height", () => {
    expect(windowOf(0, ROW, 0, 380, 8)).toEqual({
      start: 0,
      end: 0,
      padTop: 0,
      padBottom: 0,
    });
    expect(windowOf(50, 0, 0, 380, 8)).toEqual({
      start: 0,
      end: 50,
      padTop: 0,
      padBottom: 0,
    });
  });
});
