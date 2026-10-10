/**
 * Marks in the margin of the feed (redesign trial: the proof).
 *
 * The feed has a ruled margin, and a block that did something leaves a small
 * mark there saying what kind of thing it was. The mark repeats what the
 * caption already says, so it is hidden from assistive technology: it is a
 * second channel, never the only one (§14).
 *
 * Marks are grey and differ by shape. Only two are red — a failure and a
 * request still waiting for an answer — and both are also their own shape,
 * because a state resting on colour alone is a state half the readers cannot
 * see. Plain prose leaves no mark at all: a margin full of dots says nothing.
 */

import type { ReactNode } from "react";

import type { ActionRow, FeedBlock } from "../core/feed";
import { rowCounts } from "./captions";

/** What a tool does to the project, as the legend groups it. */
export type SignKind = "read" | "edit" | "run" | "web" | "other";

/** Three steps of magnitude: a sign grows with the edit it stands for. */
export type SignSize = 1 | 2 | 3;

const KINDS: Record<string, SignKind> = {
  Read: "read",
  Glob: "read",
  Grep: "read",
  Write: "edit",
  Edit: "edit",
  Move: "edit",
  Delete: "edit",
  DeleteDir: "edit",
  Bash: "run",
  BashOutput: "run",
  WebFetch: "web",
  WebSearch: "web",
};

export function kindOf(tool: string): SignKind {
  return KINDS[tool] ?? "other";
}

/** The ramp is fixed: under 10 lines, under 100, more. */
export function sizeOf(lines: number): SignSize {
  if (lines >= 100) return 3;
  if (lines >= 10) return 2;
  return 1;
}

function rowSize(row: ActionRow): SignSize {
  const counts = rowCounts(row);
  return counts ? sizeOf(counts.added + counts.removed) : 1;
}

/** The sign of one action row, in the row's own left column. */
export function RowSign({ row }: { row: ActionRow }) {
  const failed = row.state === "failed";
  const refused = row.state === "rejected" || row.state === "denied" || row.state === "stopped";
  return (
    <span
      className={signClass(kindOf(row.tool), { failed, refused, live: row.state === "running" })}
      aria-hidden="true"
    >
      {failed ? <Cross /> : <ToolGlyph kind={kindOf(row.tool)} size={rowSize(row)} />}
    </span>
  );
}

/** The mark of a feed block, on the margin rule. `null` where prose speaks for itself. */
export function RouteSign({ block, live }: { block: FeedBlock; live: boolean }) {
  switch (block.kind) {
    case "human":
      return wrap("sign sign-human", <Settlement />);
    case "agent":
      return null;
    case "reasoning":
      return live ? wrap("sign sign-step sign-live", <Dot filled={false} />) : null;
    case "actions": {
      const kind = dominant(block.rows);
      const failed = block.rows.some((r) => r.state === "failed");
      const running = block.rows.some((r) => r.state === "running");
      const lines = block.rows.reduce((sum, row) => {
        const counts = rowCounts(row);
        return counts ? sum + counts.added + counts.removed : sum;
      }, 0);
      return wrap(
        signClass(kind, { failed: false, refused: false, live: running }),
        <>
          <ToolGlyph kind={kind} size={sizeOf(lines)} />
          {failed && <span className="sign-flag" />}
        </>,
      );
    }
    case "todos":
    case "plan":
      return null;
    case "permission":
      return wrap(block.open ? "sign sign-ask sign-live" : "sign sign-step", <Rings />);
    case "checkpoint":
      return wrap("sign sign-step", <Tick />);
    case "error":
      return wrap("sign sign-failed", <Cross />);
    case "restored":
    case "compacted":
    case "retry":
    case "stopped":
      return wrap("sign sign-step", <Tick />);
  }
}

function wrap(className: string, glyph: ReactNode) {
  return (
    <span className={`route-sign ${className}`} aria-hidden="true">
      {glyph}
    </span>
  );
}

function signClass(
  kind: SignKind,
  state: { failed: boolean; refused: boolean; live: boolean },
): string {
  const parts = ["sign", state.failed ? "sign-failed" : `sign-${kind}`];
  if (state.refused) parts.push("sign-refused");
  if (state.live) parts.push("sign-live");
  return parts.join(" ");
}

/** A group stands under the heaviest thing it did: change, then run, then look. */
function dominant(rows: ActionRow[]): SignKind {
  const kinds = new Set(rows.map((row) => kindOf(row.tool)));
  for (const kind of ["edit", "run", "web", "read"] as const) {
    if (kinds.has(kind)) return kind;
  }
  return "other";
}

/* --- glyphs: one 14 px box, one 1.5 px stroke ----------------------------- */

function Svg({ children }: { children: ReactNode }) {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5">
      {children}
    </svg>
  );
}

function ToolGlyph({ kind, size }: { kind: SignKind; size: SignSize }) {
  switch (kind) {
    case "read":
      // Looking: the open ring. It changes nothing.
      return (
        <Svg>
          <circle cx="7" cy="7" r="3.25" />
        </Svg>
      );
    case "web":
      // The ring with a bar: looking, but outside the project.
      return (
        <Svg>
          <circle cx="7" cy="7" r="4.25" />
          <path d="M2.75 7h8.5" />
        </Svg>
      );
    case "edit": {
      // A change: the filled dot, and it grows with the edit.
      const r = size === 3 ? 5.25 : size === 2 ? 4 : 2.75;
      return (
        <Svg>
          <circle cx="7" cy="7" r={r} fill="currentColor" stroke="none" />
        </Svg>
      );
    }
    case "run":
      // A command is a direction of travel.
      return (
        <Svg>
          <path d="M5 3.25 9.25 7 5 10.75" strokeLinecap="round" strokeLinejoin="round" />
        </Svg>
      );
    case "other":
      return <Dot filled />;
  }
}

function Dot({ filled }: { filled: boolean }) {
  return (
    <Svg>
      <circle cx="7" cy="7" r="2.25" fill={filled ? "currentColor" : undefined} />
    </Svg>
  );
}

/** Where a person spoke. */
function Settlement() {
  return (
    <Svg>
      <circle cx="7" cy="7" r="5.25" />
      <circle cx="7" cy="7" r="2.5" fill="currentColor" stroke="none" />
    </Svg>
  );
}

/** Two rings: the place where the agent waits for you. */
function Rings() {
  return (
    <Svg>
      <circle cx="7" cy="7" r="5.25" />
      <circle cx="7" cy="7" r="2" />
    </Svg>
  );
}

function Cross() {
  return (
    <Svg>
      <path d="M3.5 3.5l7 7M10.5 3.5l-7 7" strokeLinecap="round" />
    </Svg>
  );
}

function Tick() {
  return (
    <Svg>
      <path d="M3 7h8" strokeLinecap="round" />
    </Svg>
  );
}
