/**
 * Diff §6.3 in the expanded action row — and the same block of lines without the
 * diff in the file panel (§6.4), which shows the file as it is.
 *
 * The hunks, line numbers and word ranges are computed by the core (§12): the
 * result is the same everywhere a diff is shown, and parsing the text in the
 * webview in parallel with the deltas would not work anyway.
 *
 * The diff is shown whole: a disclosure that needs a second click is not a
 * disclosure (§16.34). A long diff is drawn with a window (§12), not cut: the
 * scroll is on the block itself, and beyond its edges there are simply no rows in
 * the markup.
 *
 * Syntax is highlighted line by line (§6.3): the grammar is picked by the file's
 * extension and loaded lazily from the bundle (`languages.ts`) until it arrives —
 * a line is rendered as plain text until then. Highlighting does not cut a line
 * into different heights: the §12 window still counts by the `--diff-line` token.
 */

import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactElement,
  type ReactNode,
} from "react";

import type { DiffLine, FileDiff, Span } from "../core/client";
import { t } from "../i18n";
import { runsOf } from "./codeText";
import { utf16Spans, between, plainLine } from "./highlight";
import { languageOf } from "./languages";
import { useHighlight, type LineHighlighter } from "./useHighlight";
import { windowOf } from "./window";

/**
 * Shorter than this, the diff is drawn whole: spacers and a scroll listener on a
 * ten-line diff are a cost with no gain.
 */
const WINDOW_FROM = 200;

/** How many rows to keep drawn beyond the edges of the window. */
const OVERSCAN = 8;

/**
 * A file's lines as they are, nothing marked as added or removed: the whole
 * file in the panel (§6.4), or what a `Read` returned in its feed row (with
 * `soft`).
 */
export type FileBody = {
  lines: DiffLine[];
  /** The view stops before the end of the file. */
  truncated: boolean;
  /** Lines the panel's search found, by index in `lines`. */
  hits?: ReadonlySet<number>;
  /** The hit the search stands on: marked stronger and scrolled into view. */
  current?: number | null;
};

export function Diff({
  diff,
  body,
  soft = false,
  path,
}: {
  /** The stream's diff of one call (§5.5.1). Absent when `body` is given. */
  diff?: FileDiff;
  /** The whole file, for the panel. Absent in the feed. */
  body?: FileBody;
  /** In an action row the backing is softer than in the panel (§2). */
  soft?: boolean;
  /** The file path: the highlighting grammar is picked from it (§6.3). */
  path?: string;
}) {
  const language = useMemo(() => (path === undefined ? null : languageOf(path)), [path]);
  const highlight = useHighlight(language);
  const items = useMemo(
    () =>
      body
        ? body.lines.map((line, at) => (
            <Line
              key={at}
              line={line}
              highlight={highlight}
              mark={at === body.current ? "now" : body.hits?.has(at) ? "hit" : null}
            />
          ))
        : diff
          ? rows(diff, highlight)
          : [],
    [diff, body, highlight],
  );
  const truncated = body ? body.truncated : (diff?.truncated ?? false);
  // The gutter is as wide as the longest number in it and no wider: a column
  // sized for five digits pushes a ten-line file's text away from its edge.
  const digits = useMemo(() => {
    const shown = body ? body.lines : (diff?.hunks.flatMap((hunk) => hunk.lines) ?? []);
    const top = shown.reduce((max, line) => Math.max(max, line.newLine ?? 0, line.oldLine ?? 0), 0);
    return String(top).length;
  }, [diff, body]);

  const box = useRef<HTMLDivElement>(null);
  const [view, setView] = useState({ scrollTop: 0, height: 0, row: 0 });
  const windowed = items.length > WINDOW_FROM;

  useEffect(() => {
    const node = box.current;
    if (!node || !windowed) return;

    const row = rowHeight(node);

    const measure = () =>
      setView({ scrollTop: node.scrollTop, height: node.clientHeight, row });

    measure();
    node.addEventListener("scroll", measure, { passive: true });
    const resize = new ResizeObserver(measure);
    resize.observe(node);
    return () => {
      node.removeEventListener("scroll", measure);
      resize.disconnect();
    };
  }, [windowed]);

  // The search steps from hit to hit: the one it stands on comes into view, a
  // third of the way down, so the lines before it are there to read too.
  const current = body?.current ?? null;
  useEffect(() => {
    const node = box.current;
    if (!node || current === null) return;
    const top = current * rowHeight(node);
    if (top < node.scrollTop || top > node.scrollTop + node.clientHeight - rowHeight(node)) {
      node.scrollTop = Math.max(0, top - node.clientHeight / 3);
    }
  }, [current]);

  const slice = windowed
    ? windowOf(items.length, view.row, view.scrollTop, view.height, OVERSCAN)
    : { start: 0, end: items.length, padTop: 0, padBottom: 0 };

  return (
    <div
      // A diff in a feed row wraps long lines like the rest of the tool's
      // output. Windowing needs rows of one height, so a long diff that is
      // windowed keeps single lines and scrolls sideways.
      className={
        soft
          ? `diff diff-soft${windowed ? "" : " diff-wrap"}${body ? " diff-read" : ""}`
          : "diff diff-whole"
      }
      style={{ "--digits": digits } as CSSProperties}
      ref={box}
    >
      {slice.padTop > 0 && <div style={{ height: slice.padTop }} />}
      {items.slice(slice.start, slice.end)}
      {slice.padBottom > 0 && <div style={{ height: slice.padBottom }} />}
      {truncated && (
        <div className="diff-more">{t(body ? "file.truncated" : "row.diff_truncated")}</div>
      )}
    </div>
  );
}

/**
 * The row height comes from the token, not from a constant: §2 promises 20 px in
 * compact mode, and a window counted with 24 would miss the screen. It is read
 * from the block itself: the file panel sets its own height on the rows.
 */
function rowHeight(node: HTMLElement): number {
  return parseFloat(getComputedStyle(node).getPropertyValue("--diff-line"));
}

/**
 * The diff as one flat list of rows of the same height.
 *
 * Flat — because the window counts by index, and nested hunks would get in its
 * way; of the same height — because a hunk's header sits on the same grid as the
 * lines (§6.3).
 *
 * The grammar is resolved once for the whole diff, not per line: the hook has one
 * subscription, and the lines do not re-render each other in a race.
 */
function rows(
  diff: FileDiff,
  highlight: LineHighlighter | null,
): ReactElement[] {
  const out: ReactElement[] = [];

  for (const [index, hunk] of diff.hunks.entries()) {
    // In a feed row the `@@ -a,b +c,d @@` header is noise: the reader wants
    // the lines. Hunks are told apart by a bare fold mark on the same grid.
    if (index > 0) {
      out.push(
        <div className="hunk hunk-fold" key={`h${index}`}>
          ⋯
        </div>,
      );
    }
    for (const [at, line] of hunk.lines.entries()) {
      out.push(<Line key={`h${index}l${at}`} line={line} highlight={highlight} />);
    }
  }

  return out;
}

function Line({
  line,
  highlight,
  mark = null,
}: {
  line: DiffLine;
  /** Found by the panel's search; `now` is the hit it stands on. */
  mark?: "hit" | "now" | null;
  /** Until the grammar arrives — `null`, and the line is rendered without highlighting. */
  highlight: LineHighlighter | null;
}) {
  const kind =
    line.kind === "add" ? "diff-add" : line.kind === "del" ? "diff-del" : "diff-ctx";
  const sign = line.kind === "add" ? "+" : line.kind === "del" ? "−" : " ";
  // The gutter shows the side the line belongs to (§6.3).
  const number = line.kind === "del" ? line.oldLine : line.newLine;

  return (
    <div className={mark ? `diff-line ${kind} diff-hit diff-hit-${mark}` : `diff-line ${kind}`}>
      <span className="diff-gutter">{number ?? ""}</span>
      <span className="diff-sign">{sign}</span>
      <span className="diff-text">{styled(line.text, line.spans, highlight)}</span>
    </div>
  );
}

/**
 * The text of a line: Lezer syntax highlighting (§6.3) under the word-level one.
 *
 * The core's word ranges are counted in code points, not in byte units: their
 * bounds are converted to UTF-16 to agree with the Lezer ranges — otherwise a
 * surrogate pair would cut both the word and the token.
 */
function styled(text: string, spans: Span[], highlight: LineHighlighter | null): ReactNode {
  const pieces = highlight === null ? plainLine(text) : highlight(text);
  const bounds = utf16Spans(text, spans);

  if (bounds.length === 0) return runsOf(text, pieces);

  const out: ReactNode[] = [];
  let at = 0;
  for (const [index, bound] of bounds.entries()) {
    if (bound.from > at) {
      out.push(runsOf(text.slice(at, bound.from), between(pieces, at, bound.from)));
    }
    out.push(
      <span className="word" key={index}>
        {runsOf(
          text.slice(bound.from, bound.to),
          between(pieces, bound.from, bound.to),
        )}
      </span>,
    );
    at = bound.to;
  }
  if (at < text.length) {
    out.push(runsOf(text.slice(at), between(pieces, at, text.length)));
  }
  return out;
}
