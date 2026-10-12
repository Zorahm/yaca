/**
 * The `@` menu (§7.2): the project's files above the field, as a list.
 *
 * Built from the same classes as the slash menu, because it is the same list
 * over the same field. The keyboard and the query live in `useMentions`, so
 * the composer only forwards keys and says what picking a file means.
 *
 * Picking happens on `mousedown`, not `click`: a click would move focus out of
 * the field first, and the caret would be lost.
 */

import { useEffect, useRef, useState, type KeyboardEvent } from "react";

import type { Feed } from "../core/feed";
import { t } from "../i18n";
import { baseName } from "./format";
import { answerFor, mentionAt, mentionKey, step, type Mention } from "./mention";

/** Quiet time after the last keystroke before a search goes out. */
const SEARCH_DEBOUNCE_MS = 120;

type Found = Feed["found"];

export function MentionMenu({
  paths,
  at,
  onPick,
}: {
  /** `null` while the answer is on its way. */
  paths: string[] | null;
  /** The highlighted row: `⏎` and `⇥` take it. */
  at: number;
  onPick: (path: string) => void;
}) {
  return (
    <div className="menu menu-slash" role="listbox" aria-label={t("mention.label")}>
      {paths === null && <div className="menu-item menu-note">{t("mention.searching")}</div>}
      {paths !== null && paths.length === 0 && (
        <div className="menu-item menu-note">{t("mention.none")}</div>
      )}
      {paths?.map((path, index) => (
        <button
          className="menu-item"
          key={path}
          role="option"
          aria-selected={index === at}
          onMouseDown={(event) => {
            event.preventDefault();
            onPick(path);
          }}
        >
          <span className="mono-target">{baseName(path)}</span>
          <span className="menu-note">{dirOf(path)}</span>
        </button>
      ))}
    </div>
  );
}

/** The folder part of a project-relative path, or nothing for a file at the root. */
function dirOf(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash < 0 ? "" : path.slice(0, slash);
}

/**
 * The mention menu's state, as the composer needs it.
 *
 * `search` is absent on the home screen (§9.1), where there is no session to
 * answer: then no mention opens at all, rather than a menu that cannot fill.
 * Esc closes the open mention for its text only; what is typed next reopens
 * it, because a new query is a new question.
 */
export function useMentions({
  text,
  caret,
  found,
  search,
  onPick,
}: {
  text: string;
  caret: number;
  found: Found;
  search: ((query: string) => void) | undefined;
  onPick: (path: string, mention: Mention) => void;
}) {
  const [dismissed, setDismissed] = useState<string | null>(null);
  const [at, setAt] = useState(0);

  const typed = mentionAt(text, caret);
  const open = search !== undefined && typed !== null && mentionKey(typed) !== dismissed ? typed : null;
  const query = open?.query ?? null;
  const paths = query === null ? null : answerFor(found, query);
  const count = paths?.length ?? 0;

  // A new query starts at the top of its list.
  useEffect(() => {
    setAt(0);
  }, [query]);

  useFileSearch(query, search);

  /**
   * Handles a key while a mention is open. `true` when the key was the menu's:
   * the composer then stops there. With no matches, `⏎` and `⇥` are not the
   * menu's, and they go on to send or move focus as they always do.
   */
  function keyDown(event: KeyboardEvent<HTMLTextAreaElement>): boolean {
    if (open === null) return false;
    if (event.key === "Escape") {
      // Stopped here, so the same key does not stop the agent behind it.
      event.preventDefault();
      event.stopPropagation();
      setDismissed(mentionKey(open));
      return true;
    }
    if (paths === null) {
      // The answer is on its way: `⏎` now would send the literal `@query`
      // instead of the file the human is in the middle of naming.
      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        return true;
      }
      return false;
    }
    if (count === 0) return false;
    const index = Math.min(at, count - 1);
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setAt(step(index, 1, count));
      return true;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setAt(step(index, -1, count));
      return true;
    }
    if ((event.key === "Tab" || event.key === "Enter") && !event.shiftKey) {
      event.preventDefault();
      onPick(paths[index], open);
      return true;
    }
    return false;
  }

  /** Takes `path` for the open mention: the composer removes the `@query` and attaches it. */
  function pick(path: string): void {
    if (open !== null) onPick(path, open);
  }

  return {
    open,
    paths,
    at: count === 0 ? 0 : Math.min(at, count - 1),
    keyDown,
    pick,
  };
}

/**
 * Asks the core for the files of `query`, a little after the typing stops.
 *
 * The search function is read through a ref: the composer hands in a new one
 * on every render, and a timer restarted by each render would never fire while
 * the agent is streaming.
 */
function useFileSearch(query: string | null, search: ((query: string) => void) | undefined): void {
  const latest = useRef(search);
  latest.current = search;

  useEffect(() => {
    if (query === null) return;
    const timer = window.setTimeout(() => latest.current?.(query), SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [query]);
}
