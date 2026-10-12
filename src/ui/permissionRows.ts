/**
 * The decisions of the permission block that do not need the DOM (§5.7, §8).
 *
 * The row of answers is a function of the request: an invariant comes with no
 * scopes, so the row is refusal and consent only, and the digits shift. Keeping
 * that mapping here lets it be tested without rendering the block.
 */

import type { Invariant, PermissionRequest, Unsaved } from "../core/client";
import type { Key } from "../i18n";

/** Why an invariant asks at all (§5.7, §8), in the muted line above the detail. */
export const INVARIANT_LINE: Record<Invariant, Key> = {
  gitCommit: "ask.invariant.git",
  gitPush: "ask.invariant.git",
  gitUnresolved: "ask.invariant.git_unresolved",
  gitConfig: "ask.invariant.git_config",
};

/**
 * "Always" is offered only when there is a scope to grant (§5.7). An invariant
 * has none: no rule the human could grant would apply to it.
 */
export function alwaysOffered(request: Pick<PermissionRequest, "scopes">): boolean {
  return request.scopes.length > 0;
}

/** The digit of the consent button: `3` beside "always", `2` without it. */
export function allowDigit(request: Pick<PermissionRequest, "scopes">): string {
  return alwaysOffered(request) ? "3" : "2";
}

/** One key press while the row of answers is up. */
export type Press = { key: string; ctrl: boolean; shift: boolean };

export type RowAnswer = "deny" | "always" | "allow" | "none";

/**
 * What a press means on the row. `Esc` is not here: it refuses at once in every
 * step, and the component handles it before it asks for the row.
 */
export function rowAnswer(press: Press, offered: boolean, allowKey: string): RowAnswer {
  // `⌃⇧⏎` is "always". Without "always" it must not fall through to the plain
  // consent below, which a Ctrl+Enter would otherwise reach.
  if (press.ctrl && press.shift && press.key === "Enter") {
    return offered ? "always" : "none";
  }
  if (press.key === "1") return "deny";
  if (offered && press.key === "2") return "always";
  if (press.key === allowKey || (press.ctrl && press.key === "Enter")) return "allow";
  return "none";
}

/** What a directory delete takes away for good (§5.7), split for the list. */
export type UnsavedLines = {
  dirs: string[];
  /** Skipped directories not named in `dirs`. */
  moreDirs: number;
  /** Files the snapshot does not keep. */
  files: number;
  /** Additional roots the rollback does not cover at all. */
  roots: string[];
};

export function unsavedLines(unsaved: Unsaved): UnsavedLines {
  return {
    dirs: unsaved.dirs,
    moreDirs: Math.max(0, unsaved.dirCount - unsaved.dirs.length),
    files: unsaved.files,
    roots: unsaved.roots,
  };
}
