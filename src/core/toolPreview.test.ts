import { describe, expect, it } from "vitest";

import type { AgentEvent } from "./bindings/AgentEvent";
import { emptyFeed, flatRows, reduceAll, type Feed } from "./feed";
import { fold } from "./work";

const preview = (callId: string, target: string | null, parent: string | null = null): Extract<AgentEvent, { type: "toolCallPreparing" }> => ({
  type: "toolCallPreparing", turnId: "t", callId, tool: "Write",
  description: null, target, input: target ? { path: target } : {}, parent,
});
const start = (callId: string, parent: string | null = null): Extract<AgentEvent, { type: "toolCallStarted" }> => ({
  ...preview(callId, "file.ts", parent), type: "toolCallStarted",
  input: { path: "file.ts", content: "complete body" },
});
const rows = (feed: Feed) => flatRows(feed.blocks.flatMap((block) => block.kind === "actions" ? block.rows : []));

describe("tool previews", () => {
  it("appears before execution and keeps its position and work key across deltas", () => {
    const first = reduceAll(emptyFeed, [preview("c1", null)]);
    const key = fold(first.blocks)[0].key;
    const streamed = reduceAll(first, [
      preview("c1", "file.ts"),
      { type: "textDelta", turnId: "t", blockId: "b", text: "Another step" },
      preview("c2", "second.ts"),
      start("c1"),
    ]);
    expect(rows(first)).toMatchObject([{ target: null, preparing: true, diff: null }]);
    expect(rows(streamed)).toMatchObject([
      { callId: "c1", target: "file.ts", preparing: false, diff: null, output: "" },
      { callId: "c2", preparing: true, diff: null },
    ]);
    expect(fold(streamed.blocks)[0].key).toBe(key);
    expect(streamed.blocks.map((block) => block.kind)).toEqual(["actions", "agent", "actions"]);
  });

  it("updates nested calls without adding a duplicate child at execution", () => {
    const feed = reduceAll(emptyFeed, [
      { ...start("task"), tool: "Task" },
      preview("child", null, "task"),
      preview("child", "file.ts", "task"),
      start("child", "task"),
    ]);
    expect(rows(feed).map((row) => row.callId)).toEqual(["task", "child"]);
    expect(rows(feed)[0].children).toHaveLength(1);
    expect(rows(feed)[1]).toMatchObject({ preparing: false, input: { content: "complete body" } });
  });

  it("ends unfinished previews on a failed stream while leaving background execution running", () => {
    const feed = reduceAll(emptyFeed, [
      { type: "turnStarted", turnId: "t", at: 100 },
      start("background"),
      preview("cut", "file.ts"),
      { type: "turnFinished", turnId: "t", stopReason: "failed", at: 200 },
    ]);
    expect(rows(feed)).toMatchObject([
      { callId: "background", state: "running" },
      { callId: "cut", state: "failed", preparing: false },
    ]);
  });

  it("cancels a preview when the user stops generation", () => {
    const feed = reduceAll(emptyFeed, [preview("cut", "file.ts"), { type: "turnCancelled", turnId: "t", at: 200 }]);
    expect(rows(feed)[0].state).toBe("failed");
  });

  it("stops an unfinished child preview when its subagent ends", () => {
    const feed = reduceAll(emptyFeed, [
      { ...start("task"), tool: "Task" },
      preview("child", "file.ts", "task"),
      { type: "toolCallFailed", callId: "task", message: "Provider disconnected" },
    ]);
    expect(rows(feed)).toMatchObject([
      { callId: "task", state: "failed", message: "Provider disconnected" },
      { callId: "child", state: "failed", preparing: false },
    ]);
  });
});
