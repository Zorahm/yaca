/**
 * The `＋` menu's phrases: what lands in the field, and where the caret goes after.
 */

import { describe, expect, it } from "vitest";

import { insertAt, mentionText, pickable } from "./attachMention";

describe("mention text", () => {
  it("names the kind and the name, for the model", () => {
    expect(mentionText("mcp", "github")).toBe('MCP server "github"');
    expect(mentionText("plugin", "superpowers")).toBe('plugin "superpowers"');
  });
});

describe("which servers can be picked", () => {
  it("only one that is up", () => {
    expect(pickable("ready")).toBe(true);
    for (const status of ["connecting", "failed", "disabled", "needsApproval", "needsLogin", "loggingIn"] as const) {
      expect(pickable(status)).toBe(false);
    }
  });
});

describe("inserting at the caret", () => {
  it("into an empty field, with a space left for what follows", () => {
    expect(insertAt("", 0, "X")).toEqual({ text: "X ", caret: 2 });
  });

  it("after a word, spaced from it", () => {
    expect(insertAt("use", 3, "X")).toEqual({ text: "use X ", caret: 6 });
  });

  it("does not double a space that is already there", () => {
    expect(insertAt("use ", 4, "X")).toEqual({ text: "use X ", caret: 6 });
    expect(insertAt("use  now", 4, "X")).toEqual({ text: "use X now", caret: 5 });
  });

  it("in the middle of a word, spaced on both sides", () => {
    expect(insertAt("ab", 1, "X")).toEqual({ text: "a X b", caret: 4 });
  });

  it("clamps a caret that is out of range", () => {
    expect(insertAt("ab", 99, "X")).toEqual({ text: "ab X ", caret: 5 });
    expect(insertAt("ab", -3, "X")).toEqual({ text: "X ab", caret: 2 });
  });
});
