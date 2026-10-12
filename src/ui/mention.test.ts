import { describe, expect, it } from "vitest";

import { answerFor, mentionAt, mentionKey, step, withoutMention } from "./mention";

describe("mentionAt", () => {
  it("opens on an @ that starts the field", () => {
    expect(mentionAt("@app", 4)).toEqual({ start: 0, query: "app" });
  });

  it("opens on an @ that follows whitespace", () => {
    expect(mentionAt("read @src/ma", 12)).toEqual({ start: 5, query: "src/ma" });
    expect(mentionAt("line\n@x", 7)).toEqual({ start: 5, query: "x" });
  });

  it("reads the query up to the caret, not to the end of the field", () => {
    expect(mentionAt("@app and more", 4)).toEqual({ start: 0, query: "app" });
  });

  it("opens on a bare @, with an empty query", () => {
    expect(mentionAt("@", 1)).toEqual({ start: 0, query: "" });
  });

  it("does not open inside a word, such as an address", () => {
    expect(mentionAt("me@host", 7)).toBeNull();
  });

  it("closes at the first space after the @", () => {
    expect(mentionAt("@app ", 5)).toBeNull();
    expect(mentionAt("@app two", 8)).toBeNull();
  });

  it("finds nothing without an @", () => {
    expect(mentionAt("plain text", 10)).toBeNull();
    expect(mentionAt("", 0)).toBeNull();
  });

  it("uses the @ before the caret, not one after it", () => {
    expect(mentionAt("@a b @c", 2)).toEqual({ start: 0, query: "a" });
  });
});

describe("withoutMention", () => {
  it("takes the @query out and puts the caret where it began", () => {
    const text = "read @src/ma please";
    const mention = mentionAt(text, 12);
    expect(mention).not.toBeNull();
    expect(withoutMention(text, mention!, 12)).toEqual({
      text: "read  please",
      caret: 5,
    });
  });

  it("leaves what is typed after the caret", () => {
    const text = "@aprest";
    expect(withoutMention(text, { start: 0, query: "ap" }, 3)).toEqual({
      text: "rest",
      caret: 0,
    });
  });
});

describe("answerFor", () => {
  it("returns the paths of the answer to this query", () => {
    expect(answerFor({ query: "app", paths: ["src/app.rs"] }, "app")).toEqual(["src/app.rs"]);
  });

  it("drops an answer to another query while this one is on its way", () => {
    expect(answerFor({ query: "ap", paths: ["x"] }, "app")).toBeNull();
  });

  it("is null before any answer", () => {
    expect(answerFor(null, "app")).toBeNull();
  });
});

describe("step", () => {
  it("moves down and wraps to the top", () => {
    expect(step(0, 1, 3)).toBe(1);
    expect(step(2, 1, 3)).toBe(0);
  });

  it("moves up and wraps to the bottom", () => {
    expect(step(0, -1, 3)).toBe(2);
    expect(step(1, -1, 3)).toBe(0);
  });

  it("stays on the only row", () => {
    expect(step(0, 1, 1)).toBe(0);
    expect(step(0, -1, 1)).toBe(0);
  });
});

describe("mentionKey", () => {
  it("is the same for the same mention and different once the query changes", () => {
    expect(mentionKey({ start: 4, query: "ap" })).toBe(mentionKey({ start: 4, query: "ap" }));
    expect(mentionKey({ start: 4, query: "ap" })).not.toBe(
      mentionKey({ start: 4, query: "app" }),
    );
  });

  it("differs for the same query at another place in the field", () => {
    expect(mentionKey({ start: 0, query: "a" })).not.toBe(mentionKey({ start: 5, query: "a" }));
  });
});
