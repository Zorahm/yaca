/**
 * Subagents read back from the stream: events go through the reducer, and the
 * helpers see what the plate and the panel will draw.
 */

import { describe, expect, it } from "vitest";

import { agentPrompt, agentsOf, agentTokens, tally } from "./agents";
import type { AgentEvent } from "./bindings/AgentEvent";
import { emptyFeed, reduceAll } from "./feed";

const sent = (callId: string, prompt: unknown = "look around"): AgentEvent => ({
  type: "toolCallStarted",
  turnId: "t1",
  callId,
  tool: "Task",
  description: null,
  target: null,
  input: { prompt },
  parent: null,
});

const read = (callId: string, parent: string | null): AgentEvent => ({
  type: "toolCallStarted",
  turnId: "t1",
  callId,
  tool: "Read",
  description: null,
  target: "src/a.rs",
  input: {},
  parent,
});

const spent = (callId: string, input: number, output: number): AgentEvent => ({
  type: "subagentUsage",
  callId,
  tokens: { input, output, cacheWrite: 0, cacheRead: 0 },
});

const finished = (callId: string): AgentEvent => ({
  type: "toolCallFinished",
  callId,
  meta: { lines: 1, items: null, truncated: false, durationMs: 5 },
});

describe("agentsOf", () => {
  it("lists the Task rows in the order they were sent, and nothing else", () => {
    const feed = reduceAll(emptyFeed, [read("c1", null), sent("a1"), read("a1c1", "a1"), sent("a2")]);
    expect(agentsOf(feed).map((row) => row.callId)).toEqual(["a1", "a2"]);
  });
});

describe("agentPrompt", () => {
  it("is the prompt the call carried", () => {
    const [row] = agentsOf(reduceAll(emptyFeed, [sent("a1", "  find the dates\n")]));
    expect(agentPrompt(row)).toBe("find the dates");
  });

  it("is empty when the call carried none", () => {
    const [row] = agentsOf(reduceAll(emptyFeed, [sent("a1", 42)]));
    expect(agentPrompt(row)).toBe("");
  });
});

describe("tokens", () => {
  it("shows the newest figure, which is already the total", () => {
    const feed = reduceAll(emptyFeed, [sent("a1"), spent("a1", 100, 30), spent("a1", 200, 50)]);
    expect(agentTokens(agentsOf(feed)[0])).toBe(250);
  });

  it("is unknown, not zero, until a figure arrives", () => {
    const feed = reduceAll(emptyFeed, [sent("a1")]);
    expect(agentTokens(agentsOf(feed)[0])).toBeNull();
    expect(tally(agentsOf(feed)).tokens).toBeNull();
  });
});

describe("tally", () => {
  it("counts who is at work, who ended badly and what all of them spent", () => {
    const feed = reduceAll(emptyFeed, [
      sent("a1"),
      sent("a2"),
      sent("a3"),
      spent("a1", 100, 30),
      spent("a2", 400, 20),
      finished("a1"),
      { type: "toolCallFailed", callId: "a3", message: "no report" },
    ]);
    expect(tally(agentsOf(feed))).toEqual({ count: 3, running: 1, failed: 1, tokens: 550 });
  });
});
