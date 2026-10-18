import { describe, expect, it } from "vitest";

import {
  bothFit,
  BROWSER,
  browserBounds,
  clamp,
  FEED_MIN,
  PANEL,
  panelBounds,
  parseWidth,
  SIDE,
  sideBounds,
} from "./resize";

describe("clamp", () => {
  it("keeps a width inside its bounds", () => {
    expect(clamp(100, { min: 200, max: 420 })).toBe(200);
    expect(clamp(300.4, { min: 200, max: 420 })).toBe(300);
    expect(clamp(900, { min: 200, max: 420 })).toBe(420);
  });

  it("falls back to the minimum when the bounds cross", () => {
    expect(clamp(300, { min: 200, max: 120 })).toBe(200);
  });
});

describe("sideBounds", () => {
  it("allows the full range in a wide window", () => {
    expect(sideBounds(1600)).toEqual({ min: SIDE.min, max: SIDE.max });
  });

  it("gives way to the feed in a narrow window", () => {
    expect(sideBounds(FEED_MIN + 300).max).toBe(300);
  });
});

describe("panelBounds", () => {
  it("leaves the feed its minimum", () => {
    expect(panelBounds(1600, 248)).toEqual({ min: PANEL.min, max: 1600 - 248 - FEED_MIN });
  });

  it("docks the panel when exactly its minimum fits", () => {
    expect(panelBounds(FEED_MIN + PANEL.min, 0)).toEqual({ min: PANEL.min, max: PANEL.min });
  });

  it("reports no room when the panel cannot fit beside the feed", () => {
    expect(panelBounds(FEED_MIN + PANEL.min - 1, 0)).toBeNull();
    expect(panelBounds(1000, 248)).toBeNull();
  });
});

describe("parseWidth", () => {
  it("reads a stored width", () => {
    expect(parseWidth("312")).toBe(312);
  });

  it("ignores what is not a width", () => {
    expect(parseWidth(null)).toBeNull();
    expect(parseWidth("")).toBeNull();
    expect(parseWidth("wide")).toBeNull();
    expect(parseWidth("-40")).toBeNull();
  });
});

describe("browserBounds", () => {
  it("keeps room for the panel beside it", () => {
    expect(browserBounds(1600, 248, PANEL.min)).toEqual({
      min: BROWSER.min,
      max: 1600 - 248 - FEED_MIN - PANEL.min,
    });
  });

  it("reports no room when the browser cannot fit beside the feed", () => {
    expect(browserBounds(FEED_MIN + BROWSER.min, 0)).toEqual({ min: BROWSER.min, max: BROWSER.min });
    expect(browserBounds(FEED_MIN + BROWSER.min - 1, 0)).toBeNull();
  });
});

describe("bothFit", () => {
  it("is true only when the browser and the panel fit at their narrowest", () => {
    expect(bothFit(FEED_MIN + BROWSER.min + PANEL.min, 0)).toBe(true);
    expect(bothFit(FEED_MIN + BROWSER.min + PANEL.min, 1)).toBe(false);
  });
});
