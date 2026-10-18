/**
 * The fold of a turn's work into one feed item, checked on the stream itself:
 * events go through the reducer and then through `fold`, the way the feed
 * draws them.
 */

import { describe, expect, it } from "vitest";

import type { AgentEvent } from "./bindings/AgentEvent";
import { emptyFeed, reduceAll, visible } from "./feed";
import { closesReply, fold, isLiveWork, type FeedItem } from "./work";

function items(...events: AgentEvent[]): FeedItem[] {
  return fold(visible(reduceAll(emptyFeed, events)));
}

const said = (text: string): AgentEvent => ({
  type: "userMessage",
  turnId: "t1",
  text,
  attachments: [],
});

const wrote = (blockId: string, text: string): AgentEvent => ({
  type: "textDelta",
  turnId: "t1",
  blockId,
  text,
});

const thought = (blockId: string, text: string): AgentEvent => ({
  type: "thinkingDelta",
  turnId: "t1",
  blockId,
  text,
});

const started = (callId: string): AgentEvent => ({
  type: "toolCallStarted",
  turnId: "t1",
  callId,
  tool: "Read",
  description: "Read the file",
  target: "src/a.rs",
  input: {},
  parent: null,
});

const asked = (id: string, callId: string): AgentEvent => ({
  type: "permissionRequested",
  request: {
    id,
    callId,
    kind: "runCommand",
    target: "npm",
    detail: "npm test",
    note: null,
    sizeBytes: null,
    files: null,
    unsaved: null,
    invariant: null,
    server: null,
    scopes: [],
    position: 1,
    total: 1,
  },
});

const sent = (callId: string): AgentEvent => ({
  type: "toolCallStarted",
  turnId: "t1",
  callId,
  tool: "Task",
  description: "Looked around",
  target: null,
  input: { prompt: "look around" },
  parent: null,
});

const under = (parent: string, callId: string): AgentEvent => ({
  type: "toolCallStarted",
  turnId: "t1",
  callId,
  tool: "Read",
  description: null,
  target: "src/a.rs",
  input: {},
  parent,
});

function steps(item: FeedItem): string[] {
  if (item.kind !== "work") throw new Error("not a work item");
  return item.steps.map((step) => step.kind);
}

describe("fold", () => {
  it("folds calls and thoughts into one item, and text divides the work", () => {
    const feed = items(
      said("fix it"),
      thought("t1", "where is it"),
      started("c1"),
      started("c2"),
      thought("t2", "one more"),
      started("c3"),
      wrote("b1", "Now the tests."),
      thought("t3", "which ones"),
      started("c4"),
      wrote("b2", "Done."),
    );

    expect(feed.map((item) => item.kind)).toEqual(["human", "work", "agent", "work", "agent"]);
    expect(steps(feed[1])).toEqual(["thought", "action", "action", "thought", "action"]);
    expect(steps(feed[3])).toEqual(["thought", "action"]);
  });

  it("takes subagents out of the work, as one item for those sent together", () => {
    const feed = items(
      said("survey it"),
      started("c1"),
      sent("a1"),
      sent("a2"),
      under("a1", "a1c1"),
      started("c2"),
      wrote("b1", "Done."),
    );

    expect(feed.map((item) => item.kind)).toEqual(["human", "work", "agents", "work", "agent"]);
    expect(steps(feed[1])).toEqual(["action"]);
    const agents = feed[2];
    if (agents.kind !== "agents") throw new Error("not an agents item");
    expect(agents.rows.map((row) => row.callId)).toEqual(["a1", "a2"]);
    // What a subagent did stays on its row, for the panel; it is no step here.
    expect(agents.rows[0].children.map((row) => row.callId)).toEqual(["a1c1"]);
    expect(steps(feed[3])).toEqual(["action"]);
  });

  it("gives agents sent after other work an item of their own", () => {
    const feed = items(sent("a1"), started("c1"), sent("a2"));
    expect(feed.map((item) => item.kind)).toEqual(["agents", "work", "agents"]);
    expect(new Set(feed.map((item) => item.key)).size).toBe(3);
  });

  it("keeps the item's key while it grows", () => {
    // One feed, reduced further: keys are minted per block, not per content.
    const one = reduceAll(emptyFeed, [thought("t1", "hm"), started("c1")]);
    const two = reduceAll(one, [thought("t2", "next"), started("c2")]);
    expect(fold(visible(two))[0].key).toBe(fold(visible(one))[0].key);
  });

  it("folds a thought with no calls", () => {
    const feed = items(said("why"), thought("t1", "because"), wrote("b1", "Because."));
    expect(feed.map((item) => item.kind)).toEqual(["human", "work", "agent"]);
    expect(steps(feed[1])).toEqual(["thought"]);
  });

  it("stands the todo list under the work it appeared in", () => {
    const feed = items(
      started("c1"),
      { type: "todosUpdated", items: [{ content: "one", status: "pending" }] },
      started("c2"),
      wrote("b1", "Done."),
    );
    expect(feed.map((item) => item.kind)).toEqual(["work", "todos", "agent"]);
    expect(steps(feed[0])).toEqual(["action", "action"]);
  });

  it("is cut by an open permission request and joined once it is answered", () => {
    const waiting = reduceAll(emptyFeed, [started("c1"), asked("r1", "c2"), started("c2")]);
    const open = fold(visible(waiting));
    expect(open.map((item) => item.kind)).toEqual(["work", "permission", "work"]);

    const answered = reduceAll(waiting, [{ type: "permissionResolved", requestId: "r1" }]);
    const closed = fold(visible(answered));
    expect(closed.map((item) => item.kind)).toEqual(["work"]);
    expect(closed[0].key).toBe(open[0].key);
  });
});

describe("isLiveWork", () => {
  it("is the last work of a running turn, until the agent speaks under it", () => {
    const feed = items(started("c1"), wrote("b1", "Next."), thought("t1", "hm"), started("c2"));
    expect(isLiveWork(feed, 2, true)).toBe(true);
    expect(isLiveWork(feed, 0, true)).toBe(false);
    expect(isLiveWork(feed, 2, false)).toBe(false);
  });
});

describe("closesReply", () => {
  it("is the last text of a reply, not a remark between stretches of work", () => {
    const feed = items(wrote("b1", "Looking."), started("c1"), wrote("b2", "Done."), said("more"));
    expect(closesReply(feed, 0, false)).toBe(false);
    expect(closesReply(feed, 2, false)).toBe(true);
  });

  it("waits for the turn to end", () => {
    const feed = items(said("hi"), wrote("b1", "Half a sen"));
    expect(closesReply(feed, 1, true)).toBe(false);
    expect(closesReply(feed, 1, false)).toBe(true);
  });

  it("is not closed by text a shown file follows", () => {
    const feed = items(wrote("b1", "Here it is:"), {
      type: "textShown",
      path: "a.md",
      command: null,
      firstLine: 1,
      lines: 1,
      total: 1,
      text: "draft",
    });
    expect(closesReply(feed, 0, false)).toBe(false);
  });

  it("holds for an earlier reply while the next turn runs", () => {
    const feed = items(wrote("b1", "Done."), said("more"), started("c1"));
    expect(closesReply(feed, 0, true)).toBe(true);
  });
});
