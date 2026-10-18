/**
 * The work of a turn, folded into feed items.
 *
 * The reducer (`core/feed`) keeps blocks in the order things happened: a
 * thought, two calls, another thought, three more calls. Drawn as is, that is
 * a ladder of one-line groups with "reasoning" lines between them. Here every
 * stretch of calls and thoughts becomes one `work` item, in order.
 *
 * What the agent says divides the work: a line of text between calls stands in
 * the feed as text, with the work before it above and the work after it
 * below. Nothing else the agent does on its own cuts a stretch — a thought
 * least of all, since a model that thinks before every call would be back to
 * the ladder.
 *
 * Sending subagents divides the work too. The `Task` calls of one stretch
 * leave it and stand in the feed as one `agents` item: what a subagent does
 * is read in the agents panel, and a row buried in a folded card would hide
 * that several agents are at work right now.
 *
 * It is a view over the blocks, not a second state: the reducer's list still
 * only grows, and this is recomputed from it on every frame.
 */

import { isAgent } from "./agents";
import type { ActionRow, FeedBlock } from "./feed";

/** One line of the work item, in the order it happened. */
export type WorkStep =
  | { kind: "thought"; key: string; text: string }
  | { kind: "action"; key: string; row: ActionRow }
  /** A snapshot taken mid-turn (before a command) keeps its place in time. */
  | { kind: "checkpoint"; key: string; block: Extract<FeedBlock, { kind: "checkpoint" }> };

export type WorkItem = { kind: "work"; key: string; steps: WorkStep[] };

/** Subagents sent one after another: the `Task` rows, in order. */
export type AgentsItem = { kind: "agents"; key: string; rows: ActionRow[] };

/** What the feed draws: reasoning and actions exist only inside a work item. */
export type FeedItem =
  | Exclude<FeedBlock, { kind: "reasoning" | "actions" }>
  | WorkItem
  | AgentsItem;

/** Blocks a stretch of work may run through without being cut in two. */
const IN_RUN = new Set<FeedBlock["kind"]>([
  "reasoning",
  "actions",
  "todos",
  "checkpoint",
]);

/** Folds every stretch of agent work into one item; the rest passes through. */
export function fold(blocks: FeedBlock[]): FeedItem[] {
  const out: FeedItem[] = [];
  let at = 0;
  while (at < blocks.length) {
    let end = at;
    while (end < blocks.length && IN_RUN.has(blocks[end].kind)) end += 1;
    if (end === at) {
      pass(out, blocks[at]);
      at += 1;
      continue;
    }
    foldRun(out, blocks.slice(at, end));
    at = end;
  }
  return out;
}

function foldRun(out: FeedItem[], run: FeedBlock[]): void {
  // A checkpoint or the todo list at either end is not inside the work.
  const first = run.findIndex(isWork);
  if (first === -1) {
    for (const block of run) pass(out, block);
    return;
  }
  let last = first;
  for (let i = run.length - 1; i > first; i -= 1) {
    if (isWork(run[i])) {
      last = i;
      break;
    }
  }

  for (const block of run.slice(0, first)) pass(out, block);

  let steps: WorkStep[] = [];
  let agents: ActionRow[] = [];
  // Each piece is keyed by what it starts with, so it keeps its identity
  // while it grows.
  const flushSteps = () => {
    if (steps.length > 0) out.push({ kind: "work", key: `work_${steps[0].key}`, steps });
    steps = [];
  };
  const flushAgents = () => {
    if (agents.length > 0) {
      out.push({ kind: "agents", key: `agents_${agents[0].callId}`, rows: agents });
    }
    agents = [];
  };
  const step = (next: WorkStep) => {
    flushAgents();
    steps.push(next);
  };

  // The todo list is not a step: it is one block for the whole session,
  // updated in place. It stands right under the work it appeared in.
  const after: FeedBlock[] = [];
  for (const block of run.slice(first, last + 1)) {
    switch (block.kind) {
      case "reasoning":
        step({ kind: "thought", key: block.key, text: block.text });
        break;
      case "actions":
        for (const row of block.rows) {
          if (isAgent(row)) {
            flushSteps();
            agents.push(row);
          } else {
            step({ kind: "action", key: row.callId, row });
          }
        }
        break;
      case "checkpoint":
        step({ kind: "checkpoint", key: block.key, block });
        break;
      default:
        after.push(block);
    }
  }
  flushSteps();
  flushAgents();

  for (const block of after) pass(out, block);
  for (const block of run.slice(last + 1)) pass(out, block);
}

function isWork(block: FeedBlock): boolean {
  return block.kind === "reasoning" || block.kind === "actions";
}

function pass(out: FeedItem[], block: FeedBlock): void {
  // Unreachable for these two: a run that holds either always folds it.
  if (block.kind === "reasoning" || block.kind === "actions") return;
  out.push(block);
}

/** The action rows of a work item, for its summary line. */
export function workRows(steps: WorkStep[]): ActionRow[] {
  return steps.flatMap((step) => (step.kind === "action" ? [step.row] : []));
}

/**
 * Whether the work item at `index` is the one the running turn is adding to.
 *
 * It is while nothing the agent said stands under it: text closes the stretch,
 * and whatever comes next is a new one. The todo list and a request waiting
 * for the person do not close it.
 */
export function isLiveWork(items: FeedItem[], index: number, busy: boolean): boolean {
  if (!busy) return false;
  for (let i = index + 1; i < items.length; i += 1) {
    const kind = items[i].kind;
    if (kind !== "todos" && kind !== "permission" && kind !== "question" && kind !== "elicitation") return false;
  }
  return true;
}

/** What the agent still adds to a reply after a piece of its text. */
const REPLY_GOES_ON = new Set<FeedItem["kind"]>([
  "agent",
  "work",
  "agents",
  "plan",
  "shown",
]);

/**
 * Whether the text at `index` is the last thing the agent said in its reply.
 *
 * A remark between two stretches of work is not the answer, and neither is
 * text of a turn that is still running: it may be mid-sentence, or more may
 * follow it.
 */
export function closesReply(items: FeedItem[], index: number, busy: boolean): boolean {
  for (let i = index + 1; i < items.length; i += 1) {
    const kind = items[i].kind;
    if (kind === "human") return true;
    if (REPLY_GOES_ON.has(kind)) return false;
  }
  return !busy;
}

/**
 * Everything the model said in the reply that the text at `index` closes: its
 * own words from the human's message up to here, in order, joined by a blank
 * line. A reply is cut into pieces by the work between them, and the copy
 * button sits under the last piece, so it has to gather the others.
 *
 * Only the model's text. Tool rows, reasoning, plans and files the agent
 * showed (`shown`) are not what the model wrote to the human, and are left out.
 */
export function replyText(items: FeedItem[], index: number): string {
  const parts: string[] = [];
  for (let i = index; i >= 0; i -= 1) {
    const item = items[i];
    if (item.kind === "human") break;
    if (item.kind === "agent" && item.text.trim() !== "") parts.push(item.text.trim());
  }
  return parts.reverse().join("\n\n");
}
