/**
 * Turns a language id into a line-highlighter in React state.
 *
 * Grammar loading is asynchronous; the hook starts with plain text and swaps
 * in highlighting when the parser arrives. The pending promise is abandoned,
 * not awaited in state: by the time it resolves the component may be gone,
 * so the arrival is guarded instead of scheduled.
 */

import { useEffect, useMemo, useState } from "react";

import { highlightLine, type Piece } from "./highlight";
import { parserOf, type LanguageId } from "./languages";

/** Highlights one line, UTF-16 offsets on the way out. */
export type LineHighlighter = (line: string) => Piece[];

/**
 * Highlighter for a language, or `null` while the grammar is not loaded.
 *
 * The closure is identity-stable per resolution (`useMemo` on the parser
 * state): downstream memos key their parsing work on the return value, so a
 * scroll tick or an unrelated state change must not force a re-parse.
 */
export function useHighlight(language: LanguageId | null): LineHighlighter | null {
  const [parser, setParser] = useState<{ id: LanguageId; value: ParserLike } | null>(null);

  useEffect(() => {
    setParser(null);
    if (language === null) return;
    let alive = true;
    parserOf(language).then((loaded) => {
      if (alive && loaded !== null) setParser({ id: language, value: loaded });
    });
    return () => {
      alive = false;
    };
  }, [language]);

  return useMemo(
    () =>
      parser === null || parser.id !== language
        ? null
        : (line) => highlightLine(line, parser.value),
    [parser, language],
  );
}

/**
 * What the hook needs of a grammar. It flows in typed from `languages.ts`,
 * but re-folded through state it keeps no import of parser machinery here.
 */
type ParserLike = Parameters<typeof highlightLine>[1];
