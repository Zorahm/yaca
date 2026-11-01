/**
 * What the `＋` menu puts into the field (§7.1): a server or a plugin picked from
 * its list becomes a phrase in the message, at the caret.
 *
 * The phrase is plain text on purpose: the person sees, edits or deletes exactly
 * what the model will read, and the core needs no new command. It is addressed to
 * the model, so it is English whatever the interface language is.
 */

import type { McpStatus } from "../core/client";

export type MentionKind = "mcp" | "plugin";

export function mentionText(kind: MentionKind, name: string): string {
  return kind === "mcp" ? `MCP server "${name}"` : `plugin "${name}"`;
}

/** A server that is not up has nothing to be asked for: it is listed, but not offered. */
export function pickable(status: McpStatus): boolean {
  return status === "ready";
}

/**
 * Put `insertion` at the caret, spaced from its neighbours, and say where the caret
 * goes: after the phrase, so the person goes on typing.
 */
export function insertAt(
  text: string,
  caret: number,
  insertion: string,
): { text: string; caret: number } {
  const at = Math.min(Math.max(caret, 0), text.length);
  const before = text.slice(0, at);
  const after = text.slice(at);
  const lead = before === "" || /\s$/.test(before) ? "" : " ";
  const trail = /^\s/.test(after) ? "" : " ";
  const inserted = `${lead}${insertion}${trail}`;
  return { text: before + inserted + after, caret: at + inserted.length };
}
