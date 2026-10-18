import { describe, expect, it } from "vitest";

import {
  MAX_SCALE,
  actual,
  clamp,
  fitScale,
  fitted,
  hits,
  isClick,
  isFitted,
  panned,
  percent,
  turned,
  zoomAt,
  type Size,
} from "./zoom";

const box: Size = { width: 1000, height: 600 };
const wide: Size = { width: 4000, height: 1200 };
const icon: Size = { width: 64, height: 64 };

describe("viewer: zoom", () => {
  it("a large picture opens whole and centred", () => {
    const view = fitted(wide, box);
    expect(view).toEqual({ scale: 0.25, x: 0, y: 0, turns: 0 });
    expect(isFitted(view, wide, box)).toBe(true);
  });

  it("does not stretch a small one to the whole window", () => {
    // Enlarging does not make an icon any more detailed.
    expect(fitScale(icon, box, 0)).toBe(1);
  });

  it("what is under the cursor stays under the cursor", () => {
    const start = fitted(wide, box);
    const at = { x: 300, y: -100 };
    // The picture point under the cursor: (at - shift) / scale.
    const before = { x: (at.x - start.x) / start.scale, y: (at.y - start.y) / start.scale };

    // Eight times: the picture is larger than the frame on both axes, and neither of them
    // pins it to the centre.
    const view = zoomAt(start, 8, at, wide, box);

    expect(view.scale).toBe(2);
    expect((at.x - view.x) / view.scale).toBeCloseTo(before.x);
    expect((at.y - view.y) / view.scale).toBeCloseTo(before.y);
  });

  it("never goes below the fitted scale or above the limit", () => {
    const start = fitted(wide, box);
    expect(zoomAt(start, 0.1, { x: 0, y: 0 }, wide, box).scale).toBe(0.25);
    expect(zoomAt(start, 1000, { x: 0, y: 0 }, wide, box).scale).toBe(MAX_SCALE);
  });

  it("zooming out brings the picture back to the centre", () => {
    // Otherwise, having moved away from a corner, you get a picture pinned to the edge of the window.
    const corner = zoomAt(fitted(wide, box), 8, { x: 400, y: 250 }, wide, box);
    const back = zoomAt(corner, 1 / 8, { x: 400, y: 250 }, wide, box);
    expect(isFitted(back, wide, box)).toBe(true);
  });

  it("1:1 is one hundred percent", () => {
    expect(percent(actual(fitted(wide, box), wide, box))).toBe(100);
  });
});

describe("viewer: pan", () => {
  it("a picture smaller than the frame cannot be panned", () => {
    const view = fitted(icon, box);
    expect(panned(view, 200, 200, icon, box)).toEqual(view);
  });

  it("the edge of a large picture does not move inside the frame", () => {
    const view = actual(fitted(wide, box), wide, box);
    const far = panned(view, 99_999, 99_999, wide, box);
    // 4000 with a frame 1000 wide: a thousand and a half to each side.
    expect(far.x).toBe(1500);
    expect(far.y).toBe(300);
    expect(panned(far, -99_999 * 2, 0, wide, box).x).toBe(-1500);
  });

  it("on an axis where the picture fits, it stays centred", () => {
    // Twice the fitted scale: width 2000 does not fit, height 600 fits exactly.
    const view = clamp({ scale: 0.5, x: 300, y: 300, turns: 0 }, wide, box);
    expect(view.x).toBe(300);
    expect(view.y).toBe(0);
  });
});

describe("viewer: rotation", () => {
  it("a picture turned on its side fits along the other side", () => {
    const view = turned(fitted(wide, box), wide, box);
    // Now it is 1200×4000 in a 1000×600 frame.
    expect(view.turns).toBe(1);
    expect(view.scale).toBeCloseTo(600 / 4000);
    expect(view.x).toBe(0);
  });

  it("four turns bring it back as it was", () => {
    let view = fitted(wide, box);
    for (let i = 0; i < 4; i += 1) view = turned(view, wide, box);
    expect(view).toEqual(fitted(wide, box));
  });
});

describe("viewer: click on empty space", () => {
  it("a fitted picture's empty space is the margins around it", () => {
    // 4000×1200 in a 1000×600 frame: a 1000×300 band in the centre.
    const view = fitted(wide, box);
    expect(hits(view, wide, { x: 0, y: 0 })).toBe(true);
    expect(hits(view, wide, { x: 499, y: 149 })).toBe(true);
    expect(hits(view, wide, { x: 0, y: 200 })).toBe(false);
    expect(hits(view, wide, { x: 0, y: -200 })).toBe(false);
  });

  it("a picture enlarged to fill the frame leaves no empty space", () => {
    const view = actual(fitted(wide, box), wide, box);
    for (const at of [
      { x: -500, y: -300 },
      { x: 500, y: 300 },
      { x: 0, y: 0 },
    ]) {
      expect(hits(view, wide, at)).toBe(true);
    }
  });

  it("a panned picture takes its space with it", () => {
    // A 64×64 icon in the centre; after the pan it is no longer where it was.
    const moved = { ...fitted(icon, box), x: 200, y: 0 };
    expect(hits(moved, icon, { x: 0, y: 0 })).toBe(false);
    expect(hits(moved, icon, { x: 210, y: 10 })).toBe(true);
  });

  it("a picture turned on its side occupies a different space", () => {
    const view = turned(fitted(wide, box), wide, box);
    // 1200×4000 at scale 0.15: a 180×600 column.
    expect(hits(view, wide, { x: 0, y: 290 })).toBe(true);
    expect(hits(view, wide, { x: 200, y: 0 })).toBe(false);
  });

  it("a shaky hand is still a click, a hand that drags is not", () => {
    const from = { x: 100, y: 100 };
    expect(isClick(from, { x: 102, y: 103 })).toBe(true);
    expect(isClick(from, { x: 110, y: 100 })).toBe(false);
  });
});
