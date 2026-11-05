import type { AgentEvent } from "./bindings/AgentEvent";
import type { Command } from "./bindings/Command";
import type { FileView } from "./bindings/FileView";

const retryPrompt = "Find the description of the segment date format in docs/.";
let next = 0;

/** A recorded retry, used only by the browser stand's normal message command. */
export function demoAgentRetry(command: Command): { started: AgentEvent[]; finished: AgentEvent[] } | null {
  if (command.type !== "sendMessage" || !command.text.includes(retryPrompt) || !command.text.includes("Task")) return null;
  const id = `demo_retry_${++next}`;
  const child = `${id}_read`;
  return {
    started: [
      { type: "userMessage", turnId: "t1", text: command.text, attachments: [] },
      { type: "toolCallStarted", turnId: "t1", callId: id, tool: "Task", description: "Looking up the date format in the docs", target: null, input: { prompt: retryPrompt }, parent: null },
      { type: "toolCallStarted", turnId: "t1", callId: child, tool: "Read", description: null, target: "docs/segments.md", input: { path: "docs/segments.md" }, parent: id },
    ],
    finished: [
      { type: "toolCallProgress", callId: child, chunk: "The segment date format uses ISO 8601 with an explicit time zone.\n" },
      { type: "toolCallFinished", callId: child, meta: { lines: 1, items: null, truncated: false, durationMs: 14 } },
      { type: "subagentUsage", callId: id, tokens: { input: 3800, output: 420, cacheWrite: 0, cacheRead: 0 } },
      { type: "toolCallProgress", callId: id, chunk: "The date format is documented in `docs/segments.md:3`.\n\nDates use ISO 8601 with an explicit time zone; the parser must preserve that zone." },
      { type: "toolCallFinished", callId: id, meta: { lines: 3, items: null, truncated: false, durationMs: 900 } },
    ],
  };
}

/** Read-only files referenced by the stand's recorded reports. */
export function demoAgentFileView(path: string): FileView | null {
  const files: Record<string, string[]> = {
    "src/zone.ts": ["export function offset(raw: string): number {", "  return 0;", "}"],
    "src/segments.test.ts": ['import { expect, it } from "vitest";', 'import { parseSegment } from "./segments";', "", 'it("preserves the time zone", () => {', '  expect(parseSegment("2026-11-03T14:00:00+03:00").toISOString()).toBe("2026-11-03T11:00:00.000Z");', "});"],
    "docs/segments.md": ["# Segment dates", "", "Dates use ISO 8601 with an explicit time zone.", "The parser must preserve that zone."],
  };
  const lines = files[path];
  if (!lines) return null;
  return {
    fact: { path, kind: "modified", movedFrom: null, added: 0, removed: 0 },
    segments: [{ kind: "unchanged", lines: lines.map((text, index) => ({ kind: "context", oldLine: index + 1, newLine: index + 1, text, spans: [] })) }],
    truncated: false,
  };
}
