/**
 * The mark at the left of an action row: what kind of thing the action is.
 *
 * It repeats what the caption already says, so it is hidden from assistive
 * technology: it is a second channel, never the only one (§14).
 *
 * A row that is running shows a spinner in the same box instead of its glyph:
 * "something is happening right now" is the one state worth a moving mark.
 * Everything else is grey and differs by shape; a failure is a cross in the
 * error colour, and is also its own shape, because a state resting on colour
 * alone is a state half the readers cannot see.
 */

import type { ReactNode } from "react";

import type { ActionRow } from "../core/feed";
import { rowCounts } from "./captions";

/** What a tool does to the project, as the legend groups it. */
type SignKind = "read" | "edit" | "run" | "web" | "other";

/** Three steps of magnitude: a sign grows with the edit it stands for. */
type SignSize = 1 | 2 | 3;

const KINDS: Record<string, SignKind> = {
  Read: "read",
  Glob: "read",
  Grep: "read",
  SearchMessage: "read",
  Write: "edit",
  Edit: "edit",
  Move: "edit",
  Delete: "edit",
  DeleteDir: "edit",
  ExtractMessage: "edit",
  Bash: "run",
  BashOutput: "run",
  BashKill: "run",
  BashRestart: "run",
  WebFetch: "web",
  WebSearch: "web",
};

function kindOf(tool: string): SignKind {
  return KINDS[tool] ?? "other";
}

/** The ramp is fixed: under 10 lines, under 100, more. */
function sizeOf(lines: number): SignSize {
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
  if (row.state === "running") return <Spinner />;

  const failed = row.state === "failed";
  const refused = row.state === "rejected" || row.state === "denied" || row.state === "stopped";
  const classes = ["sign", failed ? "sign-failed" : ""];
  if (refused) classes.push("sign-refused");
  return (
    <span className={classes.filter(Boolean).join(" ")} aria-hidden="true">
      {failed ? <Cross /> : <ToolGlyph kind={kindOf(row.tool)} size={rowSize(row)} />}
    </span>
  );
}

/** Running right now. Same 14 px box as a glyph, so the caption does not move. */
export function Spinner() {
  return (
    <span className="sign" aria-hidden="true">
      <span className="spinner" />
    </span>
  );
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
      return (
        <Svg>
          <circle cx="7" cy="7" r="2.25" fill="currentColor" />
        </Svg>
      );
  }
}

function Cross() {
  return (
    <Svg>
      <path d="M3.5 3.5l7 7M10.5 3.5l-7 7" strokeLinecap="round" />
    </Svg>
  );
}
