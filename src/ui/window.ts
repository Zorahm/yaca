/**
 * Which rows of a long list are worth rendering (§12).
 *
 * Pure on purpose: the decision has no DOM, no scroll events and no clock, so
 * it can be read as arithmetic and tested as arithmetic. The component around
 * it only measures and renders.
 *
 * Rows are assumed to be the same height. That is a constraint on the diff, not
 * an assumption about it: hunk headers and gaps are laid out on the same grid
 * as the lines, which is what lets this be four lines of arithmetic instead of
 * a table of prefix sums.
 */

export type Window = {
  /** First item to render. */
  start: number;
  /** One past the last item to render. */
  end: number;
  /** Filler above the rendered slice, in pixels. */
  padTop: number;
  /** Filler below it. */
  padBottom: number;
};

/**
 * `viewport` of zero means "not measured yet" and is not a special case: the
 * formula then yields the overscan alone, which is a reasonable first paint,
 * and the measurement that follows corrects it on the next frame.
 */
export function windowOf(
  count: number,
  rowHeight: number,
  scrollTop: number,
  viewport: number,
  overscan: number,
): Window {
  if (count <= 0 || rowHeight <= 0) {
    return { start: 0, end: Math.max(0, count), padTop: 0, padBottom: 0 };
  }

  const first = clamp(Math.floor(scrollTop / rowHeight) - overscan, 0, count);
  const span = Math.ceil(Math.max(0, viewport) / rowHeight) + overscan * 2 + 1;
  const last = clamp(first + span, first, count);

  return {
    start: first,
    end: last,
    padTop: first * rowHeight,
    padBottom: (count - last) * rowHeight,
  };
}

function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}
