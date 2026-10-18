/**
 * Golden tests of the feed — a mirror of the ones in the core.
 *
 * The core checks that the event stream has exactly this shape; here it is checked that
 * the feed is a function of it. Together they are the §15 promise, and it breaks
 * here, not by eye.
 */

import { describe, expect, it } from "vitest";

import type { AgentEvent } from "./bindings/AgentEvent";
import type { QueuedMessage } from "./bindings/QueuedMessage";
import type { QuestionReply } from "./bindings/QuestionReply";
import {
  diffOf,
  emptyFeed,
  factOf,
  flatRows,
  reduceAll,
  touchedFiles,
  visible,
  type Feed,
} from "./feed";

function run(...events: AgentEvent[]): Feed {
  return reduceAll(emptyFeed, events);
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

const started = (callId: string, tool = "Read", parent: string | null = null): AgentEvent => ({
  type: "toolCallStarted",
  turnId: "t1",
  callId,
  tool,
  description: "Read the file",
  target: "src/a.rs",
  input: {},
  parent,
});

const finished = (callId: string): AgentEvent => ({
  type: "toolCallFinished",
  callId,
  meta: { lines: 3, items: null, truncated: false, durationMs: 12 },
});

describe("feed", () => {
  it("joins the deltas of one block into one reply", () => {
    const feed = run(wrote("b1", "Looking "), wrote("b1", "at the parser."));

    expect(feed.blocks).toHaveLength(1);
    expect(feed.blocks[0]).toMatchObject({
      kind: "agent",
      text: "Looking at the parser.",
    });
  });

  it("a new model block is a new reply", () => {
    const feed = run(wrote("b1", "one"), wrote("b2", "two"));
    expect(feed.blocks).toHaveLength(2);
  });

  it("collects consecutive actions into one group", () => {
    const feed = run(started("c1"), started("c2"), started("c3"));

    expect(feed.blocks).toHaveLength(1);
    expect(feed.blocks[0]).toMatchObject({ kind: "actions" });
    if (feed.blocks[0].kind !== "actions") throw new Error("not a group");
    expect(feed.blocks[0].rows).toHaveLength(3);
  });

  it("a reply and reasoning between actions stand between them", () => {
    // The reducer keeps the order; the turn's work is gathered into blocks by `core/work`.
    const feed = run(
      started("c1"),
      started("c2"),
      thought("t1", "now the tests"),
      started("c3"),
      wrote("b1", "now the styles"),
      started("c4"),
    );

    expect(feed.blocks.map((b) => b.kind)).toEqual([
      "actions",
      "reasoning",
      "actions",
      "agent",
      "actions",
    ]);
    if (feed.blocks[0].kind !== "actions") throw new Error("not a group");
    expect(feed.blocks[0].rows).toHaveLength(2);
  });

  it("an action's outcome reaches its own row", () => {
    const feed = run(
      started("c1"),
      started("c2"),
      finished("c1"),
      { type: "toolCallFailed", callId: "c2", message: "not found" },
    );

    if (feed.blocks[0].kind !== "actions") throw new Error("not a group");
    const [one, two] = feed.blocks[0].rows;
    expect(one.state).toBe("done");
    expect(one.meta?.durationMs).toBe(12);
    expect(two.state).toBe("failed");
    expect(two.message).toBe("not found");
  });

  it("command output accumulates in its own row", () => {
    const feed = run(
      started("c1", "Bash"),
      { type: "toolCallProgress", callId: "c1", chunk: "one\n" },
      { type: "toolCallProgress", callId: "c1", chunk: "two\n" },
    );

    if (feed.blocks[0].kind !== "actions") throw new Error("not a group");
    expect(feed.blocks[0].rows[0].output).toBe("one\ntwo\n");
  });

  it("an edit arrives in its row with the diff and the counters", () => {
    const feed = run(started("c1", "Edit"), {
      type: "fileChanged",
      callId: "c1",
      fact: {
        path: "src/a.rs",
        kind: "modified",
        movedFrom: null,
        added: 4,
        removed: 2,
      },
      diff: { hunks: [], added: 4, removed: 2, truncated: false },
    });

    if (feed.blocks[0].kind !== "actions") throw new Error("not a group");
    expect(feed.blocks[0].rows[0].fact?.added).toBe(4);
    expect(feed.blocks[0].rows[0].diff).not.toBeNull();
  });

  it("a rename puts each file in a row under it", () => {
    const changed = (path: string, added: number): AgentEvent => ({
      type: "fileChanged",
      callId: "c1",
      fact: { path, kind: "modified", movedFrom: null, added, removed: added },
      diff: { hunks: [], added, removed: added, truncated: false },
    });
    const feed = run(
      started("c1", "Rename"),
      changed("src/a.rs", 2),
      changed("src/b.rs", 1),
      changed("src/a.rs", 3),
    );

    if (feed.blocks[0].kind !== "actions") throw new Error("not a group");
    const row = feed.blocks[0].rows[0];
    // The row is the rename; the files are under it, one row per file.
    expect(row.fact).toBeNull();
    expect(row.children.map((c) => c.fact?.path)).toEqual(["src/a.rs", "src/b.rs"]);
    expect(row.children[0].fact?.added).toBe(3);
    expect(feed.diffs["src/b.rs"]).toBeDefined();
  });

  it("a server form appears once, however many times it arrives, and closes on an answer", () => {
    const request = {
      id: "req_1",
      server: "billing",
      message: "Which account?",
      fields: [],
    };
    const feed = run(
      { type: "elicitationRequested", request },
      { type: "elicitationRequested", request },
      { type: "elicitationResolved", requestId: "req_1", reply: { type: "decline" } },
    );
    const forms = feed.blocks.filter((b) => b.kind === "elicitation");
    expect(forms).toHaveLength(1);
    expect(forms[0].kind === "elicitation" && forms[0].status).toBe("declined");
  });

  it("the todo list is updated in place", () => {
    // §5.6: no new block is created.
    const feed = run(
      { type: "todosUpdated", items: [{ content: "one", status: "pending" }] },
      wrote("b1", "working"),
      {
        type: "todosUpdated",
        items: [{ content: "one", status: "completed" }],
      },
    );

    const todos = feed.blocks.filter((b) => b.kind === "todos");
    expect(todos).toHaveLength(1);
    expect(feed.blocks[0].kind).toBe("todos");
    if (feed.blocks[0].kind !== "todos") throw new Error("not a todo list");
    expect(feed.blocks[0].items[0].status).toBe("completed");
  });

  it("an answered permission request leaves the feed but not the history", () => {
    const request = {
      id: "req_1",
      callId: "c1",
      kind: "runCommand" as const,
      target: "npm",
      detail: "npm ci",
      note: "/p",
      sizeBytes: null,
      files: null,
      unsaved: null,
      invariant: null,
    server: null,
      scopes: [],
      position: 1,
      total: 1,
    };
    const feed = run(
      { type: "permissionRequested", request },
      { type: "permissionResolved", requestId: "req_1" },
    );

    expect(feed.request).toBeNull();
    expect(visible(feed)).toHaveLength(0);
    // Blocks only get added: the indices and the replay from the journal rely on that.
    expect(feed.blocks).toHaveLength(1);
  });

  it("stopping a turn marks unfinished actions", () => {
    const feed = run(started("c1"), started("c2"), finished("c1"), {
      type: "turnCancelled",
      turnId: "t1",
      at: 0,
    });

    if (feed.blocks[0].kind !== "actions") throw new Error("not a group");
    expect(feed.blocks[0].rows.map((r) => r.state)).toEqual(["done", "failed"]);
    expect(feed.blocks[1].kind).toBe("stopped");
  });

  it("editing the summary does not add a second divider", () => {
    // §5.11: the summary is edited before continuing, not written again.
    const feed = run(
      { type: "contextCompacted", summary: "first", freed: 84000 },
      { type: "summaryEdited", summary: "edited" },
    );

    expect(feed.blocks).toHaveLength(1);
    expect(feed.blocks[0]).toMatchObject({
      kind: "compacted",
      summary: "edited",
      freed: 84000,
    });
  });

  it("the human's message comes from the stream, not from sending", () => {
    // Otherwise a reopened session would show answers without questions (§9).
    const feed = run(said("fix the date parsing"), wrote("b1", "looking"));
    expect(feed.blocks[0]).toMatchObject({
      kind: "human",
      text: "fix the date parsing",
    });
  });

  it("an empty batch creates no new snapshot", () => {
    // §12: redraw per frame, not per event.
    const feed = run(wrote("b1", "one"));
    expect(reduceAll(feed, [])).toBe(feed);
  });
});

describe("panels", () => {
  const changed = (callId: string, path: string, added = 2): AgentEvent => ({
    type: "fileChanged",
    callId,
    fact: { path, kind: "modified", movedFrom: null, added, removed: 1 },
    diff: { hunks: [], added, removed: 1, truncated: false },
  });

  it("an edit feeds the file panel's index", () => {
    const feed = run(started("c1", "Edit"), changed("c1", "a.rs"));

    expect(feed.diffs["a.rs"]).toBeTruthy();
    expect(diffOf(feed, "a.rs")).toBe(feed.diffs["a.rs"]);
  });

  it("diffOf falls back to the action row's diff", () => {
    const feed = run(started("c3", "Edit"), changed("c3", "src/a.rs"));

    // The index and the row hold one event; this checks the fallback itself, by dropping the index.
    expect(diffOf({ ...feed, diffs: {} }, "src/a.rs")).toBeTruthy();
    expect(diffOf({ ...feed, diffs: {} }, "src/other.rs")).toBeNull();
  });

  it("the last edit of a path wins", () => {
    // The file panel shows the file as it became, not its first edit.
    const feed = run(
      started("c1", "Edit"),
      changed("c1", "a.rs", 2),
      started("c2", "Edit"),
      changed("c2", "a.rs", 5),
    );

    expect(feed.diffs["a.rs"].added).toBe(5);
    expect(factOf(feed, "a.rs")?.added).toBe(5);
    expect(touchedFiles(feed)).toEqual(["a.rs"]);
  });

  it("process output: a reset puts the buffer, chunks are appended to their own id", () => {
    const feed = run(
      { type: "processOutputReset", id: 3, text: "ready\n", dropped: 0 },
      { type: "processOutput", id: 3, chunk: "compiled\n" },
    );

    expect(feed.processOutput).toEqual({ id: 3, text: "ready\ncompiled\n", dropped: 0 });
  });

  it("a chunk from another process does not replace the selected buffer", () => {
    // A selection is announced by a reset; a chunk without a reset is not the one being looked at.
    const feed = run(
      { type: "processOutputReset", id: 3, text: "ready\n", dropped: 0 },
      { type: "processOutput", id: 7, chunk: "other\n" },
    );

    expect(feed.processOutput).toEqual({ id: 3, text: "ready\n", dropped: 0 });
  });

  it("a new selection resets the whole buffer", () => {
    const feed = run(
      { type: "processOutputReset", id: 3, text: "ready\n", dropped: 0 },
      { type: "processOutput", id: 3, chunk: "compiled\n" },
      { type: "processOutputReset", id: 4, text: "listening\n", dropped: 12 },
    );

    expect(feed.processOutput).toEqual({ id: 4, text: "listening\n", dropped: 12 });
  });
});

describe("subagent", () => {
  const changed = (callId: string, path: string): AgentEvent => ({
    type: "fileChanged",
    callId,
    fact: { path, kind: "created", movedFrom: null, added: 3, removed: 0 },
    diff: { hunks: [], added: 3, removed: 0, truncated: false },
  });

  const asked = (callId: string): AgentEvent => ({
    type: "permissionRequested",
    request: {
      id: "req_1",
      callId,
      kind: "editFile",
      target: "notes.txt",
      detail: "",
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

  /** The top-level rows: what the group draws itself. */
  function top(feed: Feed) {
    return feed.blocks.flatMap((b) => (b.kind === "actions" ? b.rows : []));
  }

  it("its calls stand under the Task row, not next to it", () => {
    const feed = run(
      started("t1", "Task"),
      started("r1", "Read", "t1"),
      finished("r1"),
      started("r2", "Read", "t1"),
    );

    expect(top(feed).map((r) => r.callId)).toEqual(["t1"]);
    expect(top(feed)[0].children.map((r) => [r.callId, r.state])).toEqual([
      ["r1", "done"],
      ["r2", "running"],
    ]);
  });

  it("a permission request does not detach a call from its row", () => {
    // The request closes the group (§5.4), but a subagent's row is not pinned to the group:
    // it goes to its own Task, wherever that one was left.
    const feed = run(
      started("t1", "Task"),
      started("w1", "Write", "t1"),
      asked("w1"),
      { type: "permissionResolved", requestId: "req_1" },
      started("r1", "Read", "t1"),
    );

    expect(top(feed).map((r) => r.callId)).toEqual(["t1"]);
    expect(top(feed)[0].children.map((r) => r.callId)).toEqual(["w1", "r1"]);
  });

  it("two subagents do not mix up each other's rows", () => {
    const feed = run(
      started("t1", "Task"),
      started("t2", "Task"),
      started("a", "Read", "t2"),
      started("b", "Read", "t1"),
    );

    const [first, second] = top(feed);
    expect(first.children.map((r) => r.callId)).toEqual(["b"]);
    expect(second.children.map((r) => r.callId)).toEqual(["a"]);
  });

  it("a file created by a subagent is visible to the file panel and the touched list", () => {
    const feed = run(
      started("t1", "Task"),
      started("w1", "Write", "t1"),
      changed("w1", "notes.txt"),
      finished("w1"),
    );

    expect(top(feed)[0].children[0].fact?.path).toBe("notes.txt");
    expect(touchedFiles(feed)).toEqual(["notes.txt"]);
    expect(factOf(feed, "notes.txt")?.kind).toBe("created");
    expect(diffOf({ ...feed, diffs: {} }, "notes.txt")).toBeTruthy();
  });

  it("stopping a turn also stops its unfinished calls", () => {
    const feed = run(
      started("t1", "Task"),
      started("r1", "Read", "t1"),
      { type: "turnCancelled", turnId: "t1", at: 0 },
    );

    // A live dot on a call nobody is running any more is a lie (§0).
    expect(flatRows(top(feed)).map((r) => r.state)).toEqual(["failed", "failed"]);
  });

  it("a call without its own Task is not lost from the feed", () => {
    const feed = run(started("r1", "Read", "t-none"));
    expect(top(feed).map((r) => r.callId)).toEqual(["r1"]);
  });
});

describe("AskUser questions", () => {
  const request = {
    id: "q_1",
    callId: "c1",
    question: "Which path?",
    options: [
      { label: "Quick", description: "without tests" },
      { label: "Full", description: null },
    ],
    allowOther: true,
    multiSelect: false,
  };

  const asked = { type: "questionAsked" as const, request };
  const resolved = (reply: QuestionReply | null) => ({
    type: "questionResolved" as const,
    requestId: "q_1",
    reply,
  });

  function question(feed: Feed) {
    const block = feed.blocks.find((b) => b.kind === "question");
    if (!block || block.kind !== "question") throw new Error("no question block");
    return block;
  }

  it("a question appears in the feed open and takes the bottom row", () => {
    const feed = run(asked);

    expect(feed.question?.id).toBe("q_1");
    expect(question(feed).status).toBe("open");
    expect(question(feed).reply).toBeNull();
    expect(visible(feed)).toHaveLength(1);
  });

  it("an answer closes the question, but the row with the choice stays in the feed", () => {
    const feed = run(asked, resolved({ type: "answered", selected: [1], other: null }));

    expect(feed.question).toBeNull();
    expect(question(feed).status).toBe("answered");
    expect(question(feed).reply).toEqual({ type: "answered", selected: [1], other: null });
    // Blocks only get added: the outcome of an answer is part of the conversation.
    expect(visible(feed)).toHaveLength(1);
  });

  it("a refusal to answer is marked apart from an answer", () => {
    const feed = run(asked, resolved({ type: "dismissed" }));

    expect(feed.question).toBeNull();
    expect(question(feed).status).toBe("dismissed");
    expect(question(feed).reply).toEqual({ type: "dismissed" });
  });

  it("an answer to a question not on screen touches nothing", () => {
    const feed = run(asked, {
      type: "questionResolved",
      requestId: "other",
      reply: { type: "dismissed" },
    });

    expect(feed.question?.id).toBe("q_1");
    expect(question(feed).status).toBe("open");
  });

  it("a question with no answer from the journal is closed, not left open", () => {
    // This is how a replay of an unfinished run looks: the core closes the question
    // without an answer, and the feed draws no buttons that nobody listens to any more.
    const feed = run(asked, resolved(null));

    expect(feed.question).toBeNull();
    expect(question(feed).status).toBe("unanswered");
    expect(question(feed).reply).toBeNull();
  });

  it("a choice keeps its own text next to the ticked options", () => {
    const feed = run(
      asked,
      resolved({ type: "answered", selected: [0], other: "both at once" }),
    );

    expect(question(feed).reply).toEqual({
      type: "answered",
      selected: [0],
      other: "both at once",
    });
  });
});

describe("file panel (§6.4)", () => {
  const pending = (path: string, hunks: number) => ({
    fact: { path, kind: "modified" as const, movedFrom: null, added: hunks, removed: 0 },
  });

  it("each file write moves the counter the panel rereads its view by", () => {
    const change = (callId: string, path: string) => ({
      type: "fileChanged" as const,
      callId,
      fact: pending(path, 1).fact,
      diff: { hunks: [], added: 1, removed: 0, truncated: false },
    });
    const feed = run(change("c1", "a.txt"), change("c2", "a.txt"));
    expect(feed.writes).toBe(2);
  });

  it("the file view holds the core's latest answer for the path", () => {
    const view = {
      fact: pending("a.txt", 1).fact,
      segments: [],
      truncated: false,
    };
    const feed = run({ type: "fileViewLoaded", path: "a.txt", view });
    expect(feed.fileView).toEqual({ path: "a.txt", view });
  });

  it("\"nothing to decide\" arrives as null, not as a missing answer", () => {
    const feed = run({ type: "fileViewLoaded", path: "a.txt", view: null });
    expect(feed.fileView).toEqual({ path: "a.txt", view: null });
  });

  it("a new view replaces the previous one, even for another file", () => {
    const view = { fact: pending("b.txt", 1).fact, segments: [], truncated: false };
    const feed = run(
      { type: "fileViewLoaded", path: "a.txt", view: null },
      { type: "fileViewLoaded", path: "b.txt", view },
    );
    expect(feed.fileView?.path).toBe("b.txt");
  });
});

describe("queue and file search (§7.4, §7.2)", () => {
  const waiting = (id: string, text: string): QueuedMessage => ({
    id,
    text,
    attachments: [],
  });

  it("shows exactly the list the core last sent, in its order", () => {
    const feed = run(
      { type: "queueUpdated", queued: [waiting("q1", "first"), waiting("q2", "second")] },
    );
    expect(feed.queued.map((m) => m.id)).toEqual(["q1", "q2"]);
  });

  it("drops a message the core no longer lists, and the empty list clears all", () => {
    const after = run(
      { type: "queueUpdated", queued: [waiting("q1", "a"), waiting("q2", "b")] },
      { type: "queueUpdated", queued: [waiting("q2", "b")] },
    );
    expect(after.queued.map((m) => m.id)).toEqual(["q2"]);

    const empty = reduceAll(after, [{ type: "queueUpdated", queued: [] }]);
    expect(empty.queued).toEqual([]);
  });

  it("takes the message into the conversation without touching the queue itself", () => {
    const feed = run(
      { type: "queueUpdated", queued: [waiting("q1", "later")] },
      { type: "queueUpdated", queued: [] },
      said("later"),
    );
    expect(feed.queued).toEqual([]);
    expect(visible(feed).some((b) => b.kind === "human" && b.text === "later")).toBe(true);
  });

  it("keeps the answer to a search with the query it answers", () => {
    const feed = run({ type: "filesFound", query: "app", paths: ["src/app.rs"] });
    expect(feed.found).toEqual({ query: "app", paths: ["src/app.rs"] });
  });

  it("the last answer wins, whichever query it is for", () => {
    const feed = run(
      { type: "filesFound", query: "a", paths: ["a.txt"] },
      { type: "filesFound", query: "ab", paths: ["ab.txt"] },
    );
    expect(feed.found?.query).toBe("ab");
  });
});

describe("turn time", () => {
  const begin: AgentEvent = { type: "turnStarted", turnId: "t1", at: 1_000 };

  it("a running turn carries its start time, a finished one clears it", () => {
    expect(run(begin).turn).toEqual({ startedAt: 1_000 });
    const done = run(begin, { type: "turnFinished", turnId: "t1", stopReason: "endTurn", at: 5_000 });
    expect(done.turn).toBeNull();
  });

  it("a line with the duration and the end time stands under the answer", () => {
    const feed = run(begin, wrote("b1", "done"), {
      type: "turnFinished",
      turnId: "t1",
      stopReason: "endTurn",
      at: 134_000,
    });
    const last = feed.blocks[feed.blocks.length - 1];
    expect(last).toMatchObject({ kind: "turnTime", startedAt: 1_000, endedAt: 134_000 });
  });

  it("a stopped turn also shows how long it ran", () => {
    const feed = run(begin, { type: "turnCancelled", turnId: "t1", at: 4_000 });
    expect(feed.blocks.map((b) => b.kind)).toEqual(["stopped", "turnTime"]);
  });

  it("without an end time (a journal from an older build) there is no line", () => {
    const feed = run(begin, { type: "turnFinished", turnId: "t1", stopReason: "endTurn", at: 0 });
    expect(feed.blocks.some((b) => b.kind === "turnTime")).toBe(false);
  });
});

describe("shown file text", () => {
  const shown = (text: string): AgentEvent => ({
    type: "textShown",
    path: "notes.md",
    command: null,
    firstLine: 2,
    lines: 2,
    total: 5,
    text,
  });

  it("appears as its own block with the path and the lines", () => {
    const feed = run(shown("two\nthree"));
    expect(feed.blocks).toMatchObject([
      { kind: "shown", path: "notes.md", firstLine: 2, lines: 2, total: 5, text: "two\nthree" },
    ]);
  });

  it("does not merge with the agent's text around it", () => {
    const feed = run(wrote("b1", "Here is the file:"), shown("two"), wrote("b1", "That's all."));
    expect(feed.blocks.map((b) => b.kind)).toEqual(["agent", "shown", "agent"]);
    expect(feed.blocks[2]).toMatchObject({ text: "That's all." });
  });
});

describe("plugin hooks in the feed", () => {
  it("a hook that blocked a message stands under that message", () => {
    const feed = run(said("deploy"), {
      type: "hookReported",
      plugin: "tools",
      event: "UserPromptSubmit",
      text: "not today",
      blocked: true,
    });
    expect(feed.blocks.map((b) => b.kind)).toEqual(["human", "hook"]);
    const hook = feed.blocks[1];
    expect(hook.kind === "hook" && hook.blocked && hook.text).toBe("not today");
  });
});

describe("the model router in the feed", () => {
  const routed = (model: string, route: string | null): AgentEvent => ({
    type: "routeDecided",
    route,
    provider: "gateway",
    model,
    matched: [],
  });

  it("puts which model answers ahead of its text", () => {
    const feed = run(said("hi"), routed("big", "hard-code"), wrote("b1", "answer"));
    expect(feed.blocks.map((b) => b.kind)).toEqual(["human", "route", "agent"]);

    const note = feed.blocks[1];
    expect(note.kind === "route" && [note.route, note.provider, note.model]).toEqual([
      "hard-code",
      "gateway",
      "big",
    ]);
  });

  it("keeps the default route, which has no name", () => {
    const feed = run(said("hi"), routed("big", null));
    const note = feed.blocks[1];
    expect(note.kind === "route" && note.route).toBeNull();
  });

  it("shows each turn's model, since the next message may go elsewhere", () => {
    const feed = run(said("one"), routed("fast", null), said("two"), routed("big", "hard-code"));
    expect(feed.blocks.filter((b) => b.kind === "route")).toHaveLength(2);
  });
});
