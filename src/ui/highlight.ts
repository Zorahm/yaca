/**
 * Syntax highlighting for rendered code (§6.3): one line in, styled runs out.
 *
 * Lezer trees and `highlightTree` are synchronous, so a line that is already
 * on screen needs no `await` — this module is pure and can run in the render
 * path and in node tests alike. Grammar loading (which is asynchronous) lives
 * in `languages.ts`; this file never resolves one.
 *
 * Ranges are given and returned as UTF-16 offsets, matching what Lezer emits
 * and what `String.slice` consumes. The core's word spans (§6.3) are counted
 * in code points, so `utf16Spans` converts them at the boundary.
 */

import { highlightTree, tagHighlighter, tags, type Highlighter } from "@lezer/highlight";
import type { Parser } from "@lezer/common";

import type { Span } from "../core/client";

/** One styled run of text. `from`/`to` are UTF-16 offsets, exact `[from, to)`. */
export interface Piece {
  readonly from: number;
  readonly to: number;
  readonly className: string | null;
}

/**
 * Token tags → CSS classes. The classes themselves are translated to the
 * `--syntax-*` design tokens by `syntax.css`, so a theme switch is a token
 * change, not a code change. `tagHighlighter` walks parent tags when a subtag
 * is not listed, which keeps the mapping short.
 */
export const codeHighlighter: Highlighter = tagHighlighter([
  { tag: tags.comment, class: "sy-comment" },
  { tag: [tags.keyword, tags.modifier], class: "sy-keyword" },
  { tag: [tags.typeName, tags.className, tags.namespace, tags.attributeName], class: "sy-type" },
  { tag: [tags.string, tags.special(tags.string)], class: "sy-string" },
  { tag: [tags.number, tags.bool, tags.atom], class: "sy-number" },
  {
    tag: [tags.function(tags.variableName), tags.function(tags.propertyName)],
    class: "sy-function",
  },
  { tag: [tags.operator, tags.operatorKeyword, tags.punctuation, tags.bracket], class: "sy-operator" },
  { tag: tags.tagName, class: "sy-tag" },
  { tag: [tags.propertyName, tags.definition(tags.variableName)], class: "sy-property" },
  { tag: [tags.heading, tags.emphasis, tags.strong], class: "sy-keyword" },
  { tag: [tags.link, tags.url, tags.monospace], class: "sy-string" },
  { tag: tags.meta, class: "sy-comment" },
  { tag: tags.invalid, class: "sy-invalid" },
]);

/**
 * Highlight a single line with a parser. Because the input never contains a
 * newline, no emitted range can cross one — per-line tags are correct by
 * construction, which is exactly what the diff's one-row-per-line grid needs.
 */
export function highlightLine(text: string, parser: Parser): Piece[] {
  const pieces: Piece[] = [];
  highlightTree(parser.parse(text), codeHighlighter, (from, to, className) => {
    pieces.push({ from, to, className });
  });
  return pieces;
}

/**
 * The core counts word-span offsets in code points, not UTF-16 units (§6.3).
 * This maps each boundary to its UTF-16 offset so the spans and Lezer pieces
 * can meet inside one `String.slice`.
 */
export function utf16Spans(
  text: string,
  spans: readonly Span[],
): ReadonlyArray<{ from: number; to: number }> {
  const starts = [0];
  for (let at = 0; at < text.length; ) {
    at += text.codePointAt(at)! >= 0x10000 ? 2 : 1;
    starts.push(at);
  }
  return spans.map((span) => ({
    from: starts[span.start] ?? text.length,
    to: starts[span.end] ?? text.length,
  }));
}

/** Pieces cut down to `[from, to)` of the original line and re-based to it. */
export function between(pieces: readonly Piece[], from: number, to: number): Piece[] {
  const out: Piece[] = [];
  for (const piece of pieces) {
    const start = Math.max(piece.from, from);
    const end = Math.min(piece.to, to);
    if (end > start) {
      out.push({ from: start - from, to: end - from, className: piece.className });
    }
  }
  return out;
}

/** Pieces for plain text: one unstyled run, or nothing for an empty line. */
export function plainLine(text: string): Piece[] {
  return text.length > 0 ? [{ from: 0, to: text.length, className: null }] : [];
}
