/**
 * Tests for the feed's scroll-intent decisions, run as arithmetic (§4).
 *
 * The node test environment has no layout engine: jsdom cannot measure text,
 * ResizeObserver does not observe anything and `scrollTop` stays 0 no matter
 * what is assigned. So the streaming-growth property — "tokens appended to the
 * last message must not push the last visible line out of view while the
 * human is at the bottom, and must not steal the scroll when they are not" —
 * is proven here by walking a frame sequence over the pure decision functions
 * in feedScroll.ts and checking the offsets the component applies while the
 * virtua measurements flow in. The pinned release itself is deliberately not
 * a threshold over the measured bottom — a stream batch larger than the
 * margin between commands would read as a human scroll and cut the pin — but
 * motion of the human above the last commanded position (`shouldHold`).
 * What this still does not cover (real text measuring, ResizeObserver
 * timing, browser scroll anchoring) is verified by hand against the window,
 * and is stated as such in README.
 */

import { describe, expect, it } from "vitest";

import {
  bottomGap,
  followTarget,
  shouldHold,
  shouldStick,
  STICK_PX,
} from "./feedScroll";

const VIEWPORT = 600;

/** One "batch" per frame: the bridge streams deltas in frame batches (§12). */
const TOKEN = 14;

function state(offset: number, scrollSize: number) {
  return { offset, scrollSize, viewportSize: VIEWPORT };
}

describe("shouldStick", () => {
  it("holds when the viewport sits at the very bottom", () => {
    expect(shouldStick(state(400, 1000))).toBe(true);
  });

  it("holds within the stick margin and lets go beyond it", () => {
    // 1000 total - 600 offset - 520 viewport = -120: deeper than the bottom.
    expect(shouldStick({ offset: 600, scrollSize: 1000, viewportSize: 520 })).toBe(true);
    expect(shouldStick({ offset: 600, scrollSize: 1000, viewportSize: 400 })).toBe(true);

    // 1000 total - 600 viewport - 300 offset = 100 of gap, one token-pair past.
    expect(shouldStick({ offset: 300, scrollSize: 1000, viewportSize: 600 })).toBe(false);
    const at = 1000 - 600 - STICK_PX;
    expect(shouldStick(state(at, 1000))).toBe(true);
    expect(shouldStick(state(at - 1, 1000))).toBe(false);
  });
});

describe("shouldHold", () => {
  it("releases on human motion above the commanded spot without a stick check", () => {
    // Commanded clamp lands the viewport at the end; the human wheels a
    // wheel-notch (tens of px, more than the few-px clamp/adjustment drift)
    // up into the text while the feed streams. Held until that motion.
    const end = 5000;
    const commanded = end;
    expect(shouldHold(state(end - 200, 5600), commanded)).toBe(false);
  });

  it("does not release when the measured tail lags the commanded one", () => {
    // The exact failure mode a threshold over the measured bottom had: one
    // stream batch bigger than STICK_PX arrives between commands, and the
    // viewport, still at its clamped spot near the old end, reads a large
    // gap. This is lag, not the human, so the pin must hold — and in these
    // two readings it is the measured-bottom arm that decides: the clamp
    // put the viewport within the stick margin of the bottom as measured,
    // whatever the commanded end thought.
    const end = 5000;
    const commanded = end + 240; // the END_PAD the component requests beyond the cached end
    expect(shouldHold(state(end, 5240), commanded)).toBe(true);
    expect(shouldHold(state(4780, 5440), commanded)).toBe(true);
    // A reading below the clamp AND far from the measured bottom is the
    // human: neither arm of shouldHold keeps it.
    expect(shouldHold(state(4600, 5440), commanded)).toBe(false);
  });

  it("holds by the command arm alone when growth pushed the tail away", () => {
    // offset sits within DRIFT_PX of the commanded position while the
    // measured gap (305px) is far past the stick margin. The command arm
    // alone keeps the pin: this is the frame right after our own scroll
    // where the measured tail lags, and a bottom-gapped threshold would
    // have cut the pin mid-stream.
    expect(shouldHold(state(995, 5000), 1000)).toBe(true);
  });

  it("falls back to the stick threshold before anything was commanded", () => {
    // Mount, before the first follow: the old threshold decides, so a feed
    // opened scrolled away from the bottom does not grab the pin.
    expect(shouldHold(state(200, 2000), null)).toBe(false);
    expect(shouldHold(state(1360, 2000), null)).toBe(true);
  });

  it("still holds at the measured bottom regardless of the command lag", () => {
    // Growth landed the clamp below the command, but the tail itself sits
    // under the stick margin: the tail is in view and the feed may keep
    // following.
    expect(shouldHold(state(4860, 5440), 5000 + 240)).toBe(true);
  });
});

describe("streaming growth appended to the last message", () => {
  it("stays pinned every frame while the human sits at the bottom", () => {
    // One-screen feed at mount; every frame the last message grows by a token
    // batch. The scroll position applied each frame is followTarget, and the
    // decision computed on the *next* frame (measurements lag a frame) must
    // still hold the bottom — nothing pushed the tail out of view.
    let scrollSize = VIEWPORT;
    let offset = 0;
    expect(shouldStick(state(offset, scrollSize))).toBe(true);

    for (let frame = 1; frame <= 40; frame += 1) {
      scrollSize += TOKEN;
      offset = followTarget(state(offset, scrollSize));
      expect(shouldStick(state(offset, scrollSize))).toBe(true);
      expect(bottomGap(state(offset, scrollSize))).toBe(0);
    }
  });

  it("never reports non-positive scroll positions while following", () => {
    // A feed shorter than the viewport must not scroll; growth from tiny
    // starts should not produce negative targets.
    let scrollSize = 0;
    let offset = 0;
    for (let frame = 1; frame <= 10; frame += 1) {
      scrollSize += TOKEN;
      offset = followTarget(state(offset, scrollSize));
      expect(offset).toBeGreaterThanOrEqual(0);
      if (scrollSize > VIEWPORT) {
        expect(offset + VIEWPORT).toBe(scrollSize);
      }
    }
  });

  it("releases the scroll as soon as the human reads above the margin", () => {
    // First token arrives while pinned, then the human scrolls up to read.
    // The feed must stop owning the scroll: the pinned offset freezes and the
    // growing tail does not pull them away from their reading position.
    let scrollSize = VIEWPORT;
    let offset = 0;
    let stuck = true;

    const advance = () => {
      scrollSize += TOKEN;
      if (stuck) offset = followTarget(state(offset, scrollSize));
      stuck = shouldStick(state(offset, scrollSize));
      return stuck;
    };

    for (let frame = 1; frame <= 10; frame += 1) {
      expect(advance()).toBe(true);
    }

    // The human scrolled up by 120px of content while reading: past the
    // margin (80px), so the feed lets go and stops owning the scroll.
    offset -= 120;
    stuck = shouldStick(state(offset, scrollSize));
    expect(stuck).toBe(false);
    const readOffset = offset;

    // Growth past the margin from that position keeps letting go ...
    for (let frame = 1; frame <= 6; frame += 1) {
      const now = advance();
      if (now) break;
    }
    expect(stuck).toBe(false);
    // ... and frozen offsets stay frozen while the feed grows beneath them.
    for (let frame = 1; frame <= 10; frame += 1) {
      advance();
      expect(offset).toBe(readOffset);
    }
  });

  it("re-engages when the human returns to the bottom", () => {
    let scrollSize = VIEWPORT;
    let offset = 0;
    let stuck = shouldStick(state(offset, scrollSize));
    expect(stuck).toBe(true);

    // Machine streams 120px and the human scrolls well up meanwhile.
    for (let frame = 1; frame <= 12; frame += 1) {
      scrollSize += TOKEN;
      if (stuck) offset = followTarget(state(offset, scrollSize));
      stuck = shouldStick(state(offset, scrollSize));
    }
    offset -= 200;
    stuck = shouldStick(state(offset, scrollSize));
    expect(stuck).toBe(false);

    // Returning to the bottom re-engages the pin at the new tail.
    offset = followTarget(state(offset, scrollSize));
    expect(shouldStick(state(offset, scrollSize))).toBe(true);
    expect(bottomGap(state(offset, scrollSize))).toBe(0);
  });
});
