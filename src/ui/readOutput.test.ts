/** The output of `Read`, parsed back into numbered lines for the feed row. */

import { describe, expect, it } from "vitest";

import { parseRead } from "./readOutput";

describe("parseRead", () => {
  it("takes the number out of every line and keeps the text as it was", () => {
    const parsed = parseRead("     9\tfn main() {\n    10\t\tlet a = 1;\n    11\t}\n");

    expect(parsed?.lines.map((line) => [line.newLine, line.text])).toEqual([
      [9, "fn main() {"],
      [10, "\tlet a = 1;"],
      [11, "}"],
    ]);
    expect(parsed?.note).toBe("");
  });

  it("keeps the tool's remark after the lines as a note", () => {
    const parsed = parseRead("     1\ta\n     2\tb\n\n[showing lines 1–2 of 40]\n");

    expect(parsed?.lines).toHaveLength(2);
    expect(parsed?.note).toBe("[showing lines 1–2 of 40]");
  });

  it("gives up on output with no numbered line", () => {
    expect(parseRead("[file is empty]\n")).toBeNull();
    expect(parseRead("")).toBeNull();
  });
});
