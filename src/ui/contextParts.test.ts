/**
 * The rows of the context window (§7.1).
 *
 * The check that matters is that the rows never disagree with the figure in
 * the header: whatever the core sent, they add up to the window.
 */

import { describe, expect, it } from "vitest";

import type { ContextBreakdown } from "../core/bindings/ContextBreakdown";
import type { UsageSnapshot } from "../core/client";
import { partsOf, shareOf } from "./contextParts";

const NOTHING: ContextBreakdown = {
  systemPrompt: 0,
  instructions: 0,
  memory: 0,
  skills: 0,
  tools: 0,
  mcpTools: 0,
  mcpNotes: 0,
  messages: 0,
  reserved: 0,
};

function usage(
  contextUsed: number,
  contextMax: number,
  breakdown: Partial<ContextBreakdown> = {},
): UsageSnapshot {
  const counts = { input: 0, output: 0, cacheWrite: 0, cacheRead: 0 };
  return {
    last: counts,
    total: counts,
    contextUsed,
    contextMax,
    breakdown: { ...NOTHING, ...breakdown },
    costUsd: 0,
  };
}

const total = (u: UsageSnapshot) => partsOf(u).reduce((sum, p) => sum + p.tokens, 0);

describe("context window rows", () => {
  it("lists what is occupied largest first, then the reserve, then what is free", () => {
    const parts = partsOf(
      usage(30_000, 100_000, {
        systemPrompt: 6_000,
        tools: 19_000,
        messages: 5_000,
        reserved: 10_000,
      }),
    );
    expect(parts).toEqual([
      { kind: "tools", tokens: 19_000 },
      { kind: "systemPrompt", tokens: 6_000 },
      { kind: "messages", tokens: 5_000 },
      { kind: "reserved", tokens: 10_000 },
      { kind: "free", tokens: 60_000 },
    ]);
  });

  it("adds up to the window", () => {
    const u = usage(30_000, 100_000, { tools: 25_000, messages: 5_000, reserved: 10_000 });
    expect(total(u)).toBe(100_000);
  });

  it("lets what is occupied eat into the reserve before it overflows", () => {
    const parts = partsOf(usage(95_000, 100_000, { messages: 95_000, reserved: 10_000 }));
    expect(parts).toEqual([
      { kind: "messages", tokens: 95_000 },
      { kind: "reserved", tokens: 5_000 },
      { kind: "free", tokens: 0 },
    ]);
  });

  it("shows one row for a figure that came without a split", () => {
    expect(partsOf(usage(24_800, 100_000))).toEqual([
      { kind: "used", tokens: 24_800 },
      { kind: "free", tokens: 75_200 },
    ]);
  });

  it("is all free before anything is measured", () => {
    expect(partsOf(usage(0, 100_000))).toEqual([{ kind: "free", tokens: 100_000 }]);
  });
});

describe("share of the window", () => {
  it("never passes the whole window", () => {
    expect(shareOf(usage(150, 100))).toBe(1);
    expect(shareOf(usage(25, 100))).toBe(0.25);
  });

  it("is zero without a window to measure against", () => {
    expect(shareOf(null)).toBe(0);
    expect(shareOf(usage(10, 0))).toBe(0);
  });
});
