/**
 * The context window as rows (§7.1): what it holds, largest first, then what
 * is kept back, then what is left.
 *
 * A pure function of the snapshot the core sends. The core does the counting
 * (`session::window`); here the numbers are only put in order and the empty
 * ones dropped — a row of zero tokens says nothing.
 */

import type { ContextBreakdown } from "../core/bindings/ContextBreakdown";
import type { UsageSnapshot } from "../core/client";

/** A part of what is occupied. `used` stands in when the core sent no split. */
export type Occupied = Exclude<keyof ContextBreakdown, "reserved"> | "used";

export type PartKind = Occupied | "reserved" | "free";

export type Part = { kind: PartKind; tokens: number };

const OCCUPIED: Exclude<Occupied, "used">[] = [
  "messages",
  "systemPrompt",
  "tools",
  "mcpTools",
  "mcpNotes",
  "instructions",
  "memory",
  "skills",
];

/**
 * The rows of the window. Their tokens add up to `contextMax`, except when the
 * window is overfull — then nothing is free and the occupied rows say by how
 * much.
 */
export function partsOf(usage: UsageSnapshot): Part[] {
  const { breakdown, contextUsed, contextMax } = usage;

  const occupied: Part[] = OCCUPIED.map((kind) => ({ kind, tokens: breakdown[kind] }))
    .filter((part) => part.tokens > 0)
    // Stable, so equal parts keep the order they are listed in above.
    .sort((a, b) => b.tokens - a.tokens);

  // A snapshot from a journal written before the split existed: the figure
  // is still true, and one honest row beats eight invented ones.
  if (occupied.length === 0 && contextUsed > 0) {
    occupied.push({ kind: "used", tokens: contextUsed });
  }

  const left = Math.max(0, contextMax - contextUsed);
  // The reserve is counted from the top of the window, so what is occupied
  // eats into it last.
  const reserved = Math.min(breakdown.reserved, left);

  const parts = [...occupied];
  if (reserved > 0) parts.push({ kind: "reserved", tokens: reserved });
  parts.push({ kind: "free", tokens: left - reserved });
  return parts;
}

/**
 * From here the window counts as nearly full: the ring changes colour and
 * §6.1.1's bar says the context is about to be compacted.
 */
export const FULL_RATIO = 0.8;

/** How full the window is, 0 to 1. Zero when the window's size is unknown. */
export function shareOf(usage: UsageSnapshot | null): number {
  if (usage === null || usage.contextMax <= 0) return 0;
  return Math.min(1, usage.contextUsed / usage.contextMax);
}
