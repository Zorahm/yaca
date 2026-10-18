/**
 * Code block and command output block (§5.2, §5.5.1).
 *
 * The same markup: §5.2 says outright that code blocks in the agent's text are
 * laid out as an output block, with a copy button. They differ only by
 * highlighting: a code block has a language from the ``` fence — its grammar is
 * loaded lazily, as for a diff (§6.3); output has no language and stays plain
 * text — a command's output was not source code.
 *
 * The button lives in the corner of the block itself and shows on hover — the
 * same trick by which §6.3 shows the buttons on a hunk's header. As a separate
 * button under the block it read as an action of the feed, not as an action on
 * this code, and it took a line in the conversation, where every line is a reply.
 *
 * The whole output, not its tail: a disclosure that needs a second click is not
 * a disclosure (§16.34). A long output does not turn the feed into a wall, because
 * the block is limited in height and scrolls by itself (§5.5.1).
 *
 * Windowing, as in the diff (§12), is not needed here: this is one `pre`, not ten
 * thousand elements.
 */

import { Check, Copy } from "@phosphor-icons/react";
import { Fragment, useMemo } from "react";
import type { ReactNode } from "react";

import { t } from "../i18n";
import { runsOf } from "./codeText";
import { plainLine, type Piece } from "./highlight";
import { fenceOf } from "./languages";
import { useHighlight } from "./useHighlight";
import { useCopy } from "./useCopy";

export function CodeBlock({
  text,
  info,
  error = false,
  plain = false,
}: {
  text: string;
  /** The language from the fence — "ts", "rust"… An output block has no fence. */
  info?: string;
  error?: boolean;
  /** No well and no frame: the text sits on the page (a command's output). */
  plain?: boolean;
}) {
  const { copied, copy } = useCopy();
  const label = t(copied ? "feed.copied" : "feed.copy");
  const highlight = useHighlight(fenceOf(info ?? ""));

  // The lines are cut once, on the text and the highlighter: the stream feeds the
  // block with deltas, and rebuilding on each piece is cheaper than keeping a node
  // tree. The memo holds on the stable highlighter from useHighlight: a text delta
  // rebuilds, a random render does not. `shown` takes the already cut array; a
  // second split would only be one more place for the two to diverge.
  const split = useMemo(() => text.split("\n"), [text]);
  const lines = useMemo(
    () => (highlight === null ? split.map(plainLine) : split.map((line) => highlight(line))),
    [split, highlight],
  );

  return (
    <div className="code-wrap">
      <pre
        className={["output", error && "output-error", plain && "output-plain"]
          .filter(Boolean)
          .join(" ")}
      >
        {shown(split, lines)}
      </pre>
      {info && <span className="code-lang">{info}</span>}
      {/* An icon without a caption, so it needs a name (§14); `title` gives the
          same to the mouse. */}
      <button
        className="block-act"
        aria-label={label}
        title={label}
        onClick={() => copy(text)}
      >
        {copied ? <Check size={13} weight="bold" /> : <Copy size={13} />}
      </button>
    </div>
  );
}

/**
 * The lines, glued by real line breaks: `white-space` on `pre` lays them out
 * itself, and a token cannot cut them in half. The lines are already cut when
 * `lines` is built — here is only the second half of the pair.
 */
function shown(lines: readonly string[], pieces: readonly Piece[][]): ReactNode {
  const out: ReactNode[] = [];
  for (const [index, line] of lines.entries()) {
    if (index > 0) out.push("\n");
    // One fragment per line: the runs of a line are keyed from zero, and
    // spread into one list the keys of two lines would collide.
    out.push(<Fragment key={index}>{runsOf(line, pieces[index] ?? [])}</Fragment>);
  }
  return out;
}
