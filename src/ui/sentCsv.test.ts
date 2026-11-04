import { describe, expect, it } from "vitest";
import { parseCsv } from "./sentCsv";

describe("CSV previews", () => {
  it("preserves quoted commas, escaped quotes, embedded lines, BOM and empty cells", () => {
    expect(parseCsv('\uFEFFa,b,c\r\n"one,two","say ""hi""","line\nnext"\r\n,,\r\n'))
      .toEqual([["a", "b", "c"], ["one,two", 'say "hi"', "line\nnext"], ["", "", ""]]);
  });
  it("keeps a final empty field and reports malformed quoted data", () => {
    expect(parseCsv("a,b\n1,")).toEqual([["a", "b"], ["1", ""]]);
    expect(parseCsv("")).toEqual([]);
    expect(() => parseCsv('a\n"unfinished')).toThrow("Unclosed");
  });
});
