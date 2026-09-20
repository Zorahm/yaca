/**
 * Scroll-intent decisions of the feed (§4), extracted as pure arithmetic.
 *
 * Virtualization (virtua) gives us measurements, but the *decision* of what to
 * do with them belongs to the feed. Keeping it free of DOM, scroll events and
 * the clock is what makes the streaming case provable in tests: jsdom/node
 * cannot lay out text, so the repeated-append scenario is checked as a frame
 * sequence over these functions instead of by eyeballing a browser window.
 */

export type ScrollState = {
  /** Distance scrolled from the top. */
  offset: number;
  /** Total scrollable content height. */
  scrollSize: number;
  /** Visible height of the scrollport. */
  viewportSize: number;
};

/** How close to the bottom counts as "the human is at the bottom". */
export const STICK_PX = 80;

/** Gap between the bottom edge of the scrollport and the content end. */
export function bottomGap(state: ScrollState): number {
  return Math.max(0, state.scrollSize - state.offset - state.viewportSize);
}

/**
 * Whether the human is at (or within `stickPx` of) the bottom right now.
 * While true the feed owns the scroll; while false scrolling is the human's.
 */
export function shouldStick(state: ScrollState, stickPx: number = STICK_PX): boolean {
  return bottomGap(state) <= stickPx;
}

/**
 * Where the viewport must sit to show the very end of the content.
 * This is what the feed applies on every frame while holding the bottom.
 */
export function followTarget(state: ScrollState): number {
  return Math.max(0, state.scrollSize - state.viewportSize);
}

/**
 * How far below the commanded spot a scroll reading may still be ours, not
 * the human's. Browser clamping and library-side adjustments move a few
 * pixels; a wheel notch moves tens.
 */
export const DRIFT_PX = 8;

/**
 * Whether a pinned viewport still belongs to the feed.
 *
 * Pinned release must NOT be a threshold over the measured bottom: that
 * measurement lags the true end by frames, and one stream batch bigger than
 * the margin would read as "the human scrolled up" and cut the pin mid-stream.
 * Instead, while the feed has commanded a position, only a reading this far
 * above that command counts as human motion. Without a command yet (first
 * frames, before anything was scrolled) the threshold applies. `shouldStick`
 * stays the re-engage test once the pin is off.
 */
export function shouldHold(
  state: ScrollState,
  commanded: number | null,
  driftPx: number = DRIFT_PX,
): boolean {
  if (commanded === null) return shouldStick(state);
  return state.offset >= commanded - driftPx || shouldStick(state);
}
