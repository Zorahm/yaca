/**
 * What `Read` returned, taken apart for the feed row.
 *
 * The tool numbers its lines for the model: a right-aligned number, a tab, the
 * text. Shown as is, that is a column of padding before every line. The row
 * draws the same lines the way a diff draws them — number in the gutter, text
 * highlighted by the file's language — so the numbers are parsed back out.
 *
 * Whatever follows the numbered lines is the tool's own remark (a cut-off
 * range, an empty file) and stays text.
 */

import type { DiffLine } from "../core/client";

export type ReadOutput = {
  lines: DiffLine[];
  /** The tool's remark after the lines, or an empty string. */
  note: string;
};

const NUMBERED = /^ *(\d+)\t(.*)$/;

/** `null` when the output holds no numbered line: it is shown as plain text. */
export function parseRead(output: string): ReadOutput | null {
  const rows = output.split("\n");
  const lines: DiffLine[] = [];
  let at = 0;
  for (; at < rows.length; at += 1) {
    const match = NUMBERED.exec(rows[at]);
    if (!match) break;
    const number = Number(match[1]);
    lines.push({
      kind: "context",
      oldLine: number,
      newLine: number,
      text: match[2],
      spans: [],
    });
  }
  if (lines.length === 0) return null;
  return { lines, note: rows.slice(at).join("\n").trim() };
}
