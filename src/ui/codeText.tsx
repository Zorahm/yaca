/**
 * Rendering of highlighted code (§6.3): a line slice plus Lezer ranges → React.
 *
 * Single shared point for the diff (§6.3) and the code block (§5.2), so both
 * render tokens the same way: gaps between ranges are flat text, ranges are
 * `<span class="sy-*">` spans whose colors come from syntax.css.
 */

import type { Piece } from "./highlight";
import type { ReactNode } from "react";

/** Text with its ranges, cut down and re-based to that slice. */
export function runsOf(text: string, pieces: readonly Piece[]): ReactNode[] {
  const out: ReactNode[] = [];
  let at = 0;
  for (const [index, piece] of pieces.entries()) {
    if (piece.from > at) out.push(text.slice(at, piece.from));
    const slice = text.slice(piece.from, piece.to);
    // Uncolored runs stay plain text: a classless span in the DOM is noise
    // only, and the diff's word spans nest enough layers already.
    out.push(
      piece.className ? (
        <span className={piece.className} key={index}>
          {slice}
        </span>
      ) : (
        slice
      ),
    );
    at = piece.to;
  }
  if (at < text.length) out.push(text.slice(at));
  return out;
}
