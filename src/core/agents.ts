/**
 * Subagents, as the feed's rows tell of them.
 *
 * A subagent is not a second state: it is the `Task` row that sent it. The
 * row's input holds the prompt, its children are what the subagent did, its
 * output is the report it handed back, and `tokens` is the newest
 * `subagentUsage` the core sent for it. Everything the plate in the feed and
 * the agents panel show is read from those rows here.
 */

import type { ActionRow, Feed } from "./feed";

/** The tool that sends a subagent. */
export const AGENT_TOOL = "Task";

export function isAgent(row: ActionRow): boolean {
  return row.tool === AGENT_TOOL;
}

/** Every subagent of the conversation, in the order they were sent. */
export function agentsOf(feed: Feed): ActionRow[] {
  return feed.blocks.flatMap((block) =>
    block.kind === "actions" ? block.rows.filter(isAgent) : [],
  );
}

/** The task the agent wrote for it, or nothing if the call carried none. */
export function agentPrompt(row: ActionRow): string {
  const input = row.input;
  if (typeof input !== "object" || input === null) return "";
  const prompt = (input as Record<string, unknown>).prompt;
  return typeof prompt === "string" ? prompt.trim() : "";
}

/**
 * What a subagent has spent: every token that went through the model for it,
 * cached or not. `null` until the first figure arrives, which is not the same
 * as zero — a journal written before the figure existed never has one.
 */
export function agentTokens(row: ActionRow): number | null {
  const spent = row.tokens;
  if (spent === null) return null;
  return spent.input + spent.output + spent.cacheWrite + spent.cacheRead;
}

export type AgentsTally = {
  count: number;
  running: number;
  /** Ended without a report: failed, refused or stopped. */
  failed: number;
  /** `null` when none of the agents has reported a figure. */
  tokens: number | null;
};

export function tally(rows: ActionRow[]): AgentsTally {
  let tokens: number | null = null;
  for (const row of rows) {
    const spent = agentTokens(row);
    if (spent !== null) tokens = (tokens ?? 0) + spent;
  }
  return {
    count: rows.length,
    running: rows.filter((row) => row.state === "running").length,
    failed: rows.filter((row) => row.state !== "running" && row.state !== "done").length,
    tokens,
  };
}
