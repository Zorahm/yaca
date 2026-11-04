import { describe, expect, it } from "vitest";
import type { AgentEvent } from "./bindings/AgentEvent";
import { emptyFeed, reduceAll, visible } from "./feed";
import { fold } from "./work";
import { backgroundProcessId, processTitles } from "./backgroundCommands";

const start = (callId: string, tool: string, description: string | null = null): Extract<AgentEvent, { type: "toolCallStarted" }> => ({
  type: "toolCallStarted", turnId: "t", callId, tool, description, target: "npm test", input: { background: true }, parent: null,
});
const output = (callId: string, chunk: string): AgentEvent => ({ type: "toolCallProgress", callId, chunk });
const done = (callId: string): AgentEvent => ({ type: "toolCallFinished", callId, meta: { lines: 1, items: null, truncated: false, durationMs: 10 } });

describe("background commands in the feed", () => {
  it("keeps a pending wait out of work groups and shows the finished result", () => {
    const waiting = reduceAll(emptyFeed, [start("wait", "BashWait")]);
    expect(fold(visible(waiting))).toEqual([]);
    const finished = reduceAll(waiting, [output("wait", "Tests passed"), done("wait")]);
    expect(fold(visible(finished)).map((block) => block.kind)).toEqual(["work"]);
  });

  it("moves a successful background launch to the panel but keeps failures visible", () => {
    const launched = reduceAll(emptyFeed, [start("bg", "Bash"), output("bg", "Running in background with id 1.\n"), done("bg")]);
    expect(fold(visible(launched))).toEqual([]);
    const failed = reduceAll(emptyFeed, [start("bg", "Bash"), { type: "toolCallFailed", callId: "bg", message: "failed to start" }]);
    expect(fold(visible(failed)).map((block) => block.kind)).toEqual(["work"]);
  });

  it("associates names by process id even when commands are identical or nested", () => {
    const feed = reduceAll(emptyFeed, [start("one", "Bash", "First test run"), output("one", "Running in background with id 1.\n"), done("one"), start("two", "Bash", "Second test run"), output("two", "Running in background with id 2.\n"), done("two")]);
    const nested = reduceAll(feed, [start("task", "Task"), { ...start("three", "Bash", "Nested test run"), parent: "task" }, output("three", "Running in background with id 3.\n"), done("three")]);
    expect([...processTitles(nested)]).toEqual([[1, "First test run"], [2, "Second test run"], [3, "Nested test run"]]);
  });

  it("recognizes a timeout handoff without hiding unrelated foreground output", () => {
    const feed = reduceAll(emptyFeed, [start("fg", "Bash", "Long build"), output("fg", "Build started\nDid not complete within 120 s — continues running in background with id 4\n"), done("fg")]);
    const block = feed.blocks[0];
    if (block.kind !== "actions") throw new Error("missing action");
    expect(backgroundProcessId(block.rows[0])).toBe(4);
    expect(fold(visible(feed))).toEqual([]);
    const foreground = reduceAll(emptyFeed, [start("fg", "Bash"), output("fg", "12 tests passed"), done("fg")]);
    expect(fold(visible(foreground))).toHaveLength(1);
  });
});
