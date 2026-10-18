import { describe, expect, it } from "vitest";

import { parseTable, splitRow } from "./markdownTable";

const lines = (s: string) => s.split("\n");

describe("markdown tables", () => {
  it("parses header, alignment and body", () => {
    const t = parseTable(lines("| a | b | c |\n| :-- | :-: | --: |\n| 1 | 2 | 3 |\ntext"), 0)!;
    expect(t.header).toEqual(["a", "b", "c"]);
    expect(t.aligns).toEqual(["left", "center", "right"]);
    expect(t.rows).toEqual([["1", "2", "3"]]);
    expect(t.end).toBe(3);
  });

  it("accepts a delimiter without colons and rows without outer pipes", () => {
    const t = parseTable(lines("a | b\n--- | ---\n1 | 2"), 0)!;
    expect(t.aligns).toEqual([null, null]);
    expect(t.rows).toEqual([["1", "2"]]);
  });

  it("is not a table without a delimiter row (streaming)", () => {
    expect(parseTable(lines("| a | b |"), 0)).toBeNull();
    expect(parseTable(lines("| a | b |\n| 1 | 2 |"), 0)).toBeNull();
  });

  it("rejects a delimiter whose column count differs", () => {
    expect(parseTable(lines("| a | b |\n| --- |"), 0)).toBeNull();
  });

  it("pads short rows and cuts long ones", () => {
    const t = parseTable(lines("| a | b |\n|---|---|\n| 1 |\n| 1 | 2 | 3 |"), 0)!;
    expect(t.rows).toEqual([["1", ""], ["1", "2"]]);
  });

  it("keeps escaped pipes and pipes in code", () => {
    expect(splitRow("| a \\| b | `x|y` |")).toEqual(["a | b", "`x|y`"]);
  });
});
