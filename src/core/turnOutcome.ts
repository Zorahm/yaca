import type { FeedBlock } from "./feed";

/** Older journals and incomplete event delivery can end a turn without an error. */
export function missingOutcome(blocks: FeedBlock[], failed: boolean): string | null {
  if (blocks.some((b) => b.kind === "error")) return null;
  if (failed) return "turn_failed";
  const visible = blocks.some((b) =>
    (b.kind === "agent" && b.text.trim().length > 0) ||
    (b.kind === "actions" && b.rows.length > 0) ||
    ["shown", "sent", "plan", "question", "permission", "elicitation", "hook"].includes(b.kind),
  );
  return visible ? null : "response_missing";
}
