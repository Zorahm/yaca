import { describe, expect, it } from "vitest";

import type { AgentEvent } from "../../core/bindings/AgentEvent";
import { emptyFeed, reduceAll } from "../../core/feed";
import { galleryOf } from "./gallery";

const said = (attachments: string[]): AgentEvent => ({
  type: "userMessage",
  turnId: "t1",
  text: "here",
  attachments,
});

const looked = (callId: string, path: string, parent: string | null = null): AgentEvent[] => [
  {
    type: "toolCallStarted",
    turnId: "t1",
    callId,
    tool: "ReadImage",
    description: null,
    target: path,
    input: { path },
    parent,
  },
  {
    type: "toolCallFinished",
    callId,
    meta: { lines: 1, items: null, truncated: false, durationMs: 3 },
  },
];

describe("viewer gallery", () => {
  it("walks the conversation: both attached and opened by the model", () => {
    const feed = reduceAll(emptyFeed, [
      said(["shot.png", "notes.md"]),
      ...looked("i1", "docs/mock.jpg"),
      said(["C:/Users/me/Desktop/after.png"]),
    ]);

    // A file that is not a picture does not enter the paging: there is nothing to look at in it.
    expect(galleryOf(feed)).toEqual([
      "shot.png",
      "docs/mock.jpg",
      "C:/Users/me/Desktop/after.png",
    ]);
  });

  it("one picture is one stop, however many times it was shown", () => {
    const feed = reduceAll(emptyFeed, [said(["shot.png"]), ...looked("i1", "shot.png")]);
    expect(galleryOf(feed)).toEqual(["shot.png"]);
  });

  it("takes what a subagent looked at too", () => {
    const feed = reduceAll(emptyFeed, [
      {
        type: "toolCallStarted",
        turnId: "t1",
        callId: "t1",
        tool: "Task",
        description: null,
        target: null,
        input: {},
        parent: null,
      },
      ...looked("i1", "ui/home.png", "t1"),
    ]);
    expect(galleryOf(feed)).toEqual(["ui/home.png"]);
  });

  it("takes the pictures a tool returned", () => {
    const feed = reduceAll(emptyFeed, [
      said(["shot.png"]),
      {
        type: "toolCallStarted",
        turnId: "t1",
        callId: "b1",
        tool: "mcp__browser__screenshot",
        description: null,
        target: "browser · screenshot",
        input: {},
        parent: null,
      },
      {
        type: "toolCallFinished",
        callId: "b1",
        meta: {
          lines: 4,
          items: null,
          truncated: false,
          durationMs: 120,
          pictures: [".yaca/pasted/0a1b2c3d4e5f6a7b.png"],
        },
      },
    ]);
    expect(galleryOf(feed)).toEqual(["shot.png", ".yaca/pasted/0a1b2c3d4e5f6a7b.png"]);
  });

  it("a failed call has nothing to show", () => {
    const [started] = looked("i1", "gone.png");
    const feed = reduceAll(emptyFeed, [
      started,
      { type: "toolCallFailed", callId: "i1", message: "not found" },
    ]);
    expect(galleryOf(feed)).toEqual([]);
    expect(galleryOf(null)).toEqual([]);
  });
});
