import { describe, expect, it } from "vitest";

import { parser } from "@lezer/javascript";

import { between, highlightLine, plainLine, utf16Spans } from "./highlight";
import type { Span } from "../core/client";

describe("highlightLine", () => {
  it("styles the keyword and the string of a JS line", () => {
    const pieces = highlightLine('const x = "hi";', parser);
    const keyword = pieces.find((piece) => piece.className === "sy-keyword");
    expect(keyword).toEqual({ from: 0, to: 5, className: "sy-keyword" });
    const string = pieces.find((piece) => piece.className === "sy-string");
    // indices: `const` 0..5, the string literal is 10..14.
    expect(string).toEqual({ from: 10, to: 14, className: "sy-string" });
  });

  it("leaves a line with no tags unstyled", () => {
    const pieces = highlightLine("plain words", parser);
    expect(pieces).toHaveLength(0);
  });

  it("ties every range to the line itself", () => {
    // Ranges must stay inside the line: the renderer slices a span per row,
    // and a range running past its text would bleed into the next row.
    const text = 'const a = "b";';
    for (const piece of highlightLine(text, parser)) {
      expect(piece.from).toBeGreaterThanOrEqual(0);
      expect(piece.to).toBeLessThanOrEqual(text.length);
      expect(piece.to).toBeGreaterThan(piece.from);
    }
  });

  it("keeps UTF-16 offsets: an emoji is one boundary, not two", () => {
    const pieces = highlightLine('"ok😀";', parser);
    // The string token is one piece over the literal; a violated pair would
    // show as a boundary at the middle of the surrogate (offset 4 of 7).
    for (const piece of pieces) {
      expect(piece.from !== 4 && piece.to !== 4, JSON.stringify(piece)).toBe(true);
    }
  });
});

describe("utf16Spans", () => {
  it("converts code-point bounds to UTF-16 offsets", () => {
    // "😀" is one code point but two UTF-16 units, so the span covering it
    // is [1, 2) in code points and [1, 3) in UTF-16.
    const spans = utf16Spans("a😀b", [
      { start: 1, end: 2 } satisfies Span,
    ]);
    expect(spans).toEqual([{ from: 1, to: 3 }]);
  });

  it("points past the string when the core reports nothing", () => {
    // A defensive bound: a span the core never filled stays at the end.
    const spans = utf16Spans("abc", [{ start: 9, end: 9 } satisfies Span]);
    expect(spans).toEqual([{ from: 3, to: 3 }]);
  });
});

describe("between", () => {
  it("cuts pieces to a window and re-bases them", () => {
    const pieces = [
      { from: 0, to: 5, className: "sy-keyword" },
      { from: 5, to: 10, className: "sy-string" },
    ];
    expect(between(pieces, 5, 8)).toEqual([
      { from: 0, to: 3, className: "sy-string" },
    ]);
  });

  it("drops pieces outside the window entirely", () => {
    const pieces = [{ from: 0, to: 2, className: null }];
    expect(between(pieces, 3, 5)).toEqual([]);
  });
});

describe("plainLine", () => {
  it("returns one unstyled run for text and none for the empty line", () => {
    expect(plainLine("code")).toEqual([{ from: 0, to: 4, className: null }]);
    expect(plainLine("")).toEqual([]);
  });
});
