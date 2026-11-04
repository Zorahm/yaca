import { expect, it } from "vitest";
import type { AgentEvent } from "./bindings/AgentEvent";
import { emptyFeed, reduceAll } from "./feed";
import { fold, closesReply } from "./work";

it("replays delivery metadata as a reply and does not report a missing response", () => {
  const events: AgentEvent[] = [
    { type: "turnStarted", turnId: "t", at: 0 },
    { type: "textDelta", turnId: "t", blockId: "b", text: "Here is your file" },
    { type: "fileSent", file: { id: "f", name: "report.pdf", title: "Report", caption: null, mime: "application/pdf", size: 42, sha256: "abc", renderType: "artifact" } },
    { type: "turnFinished", turnId: "t", at: 1, stopReason: "endTurn" },
  ];
  const feed = reduceAll(emptyFeed, events);
  expect(feed.blocks.filter((b) => b.kind === "sent")).toEqual([{ kind: "sent", key: expect.any(String), file: events[2].type === "fileSent" ? events[2].file : null }]);
  expect(feed.blocks.some((b) => b.kind === "error")).toBe(false);
  expect(closesReply(fold(feed.blocks), 0, false)).toBe(false);
  const replay = reduceAll(emptyFeed, events);
  expect(replay.blocks.map(({ key: _key, ...block }) => block)).toEqual(feed.blocks.map(({ key: _key, ...block }) => block));
});
