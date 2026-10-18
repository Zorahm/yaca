/**
 * GFM pipe tables for the Markdown renderer.
 *
 * A table is a header row, a delimiter row (`| --- | :-: | --: |`) and body
 * rows. Without the delimiter row the lines are plain text: while a reply
 * streams in, `| a | b |` is a paragraph until the next line shows up, and
 * the table appears the moment the delimiter does.
 */

export type Align = "left" | "center" | "right" | null;

export interface Table {
  header: string[];
  /** One entry per column; `null` when the delimiter row sets no alignment. */
  aligns: Align[];
  /** Every row has exactly `header.length` cells. */
  rows: string[][];
  /** Index of the first line after the table. */
  end: number;
}

/** Split a row into trimmed cells. Pipes in code spans or after `\` stay in the cell. */
export function splitRow(line: string): string[] {
  let text = line.trim();
  if (text.startsWith("|")) text = text.slice(1);
  if (text.endsWith("|") && !text.endsWith("\\|")) text = text.slice(0, -1);

  const cells: string[] = [];
  let cell = "";
  let inCode = false;
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (ch === "\\" && text[i + 1] === "|") {
      cell += "|";
      i += 1;
    } else if (ch === "`") {
      inCode = !inCode;
      cell += ch;
    } else if (ch === "|" && !inCode) {
      cells.push(cell.trim());
      cell = "";
    } else {
      cell += ch;
    }
  }
  cells.push(cell.trim());
  return cells;
}

function parseAligns(line: string): Align[] | null {
  // A pipe is required: a bare `---` is a rule, not a one-column delimiter.
  if (!line.includes("-") || !line.includes("|")) return null;
  const cells = splitRow(line);
  const aligns: Align[] = [];
  for (const cell of cells) {
    if (!/^:?-+:?$/.test(cell)) return null;
    const left = cell.startsWith(":");
    const right = cell.endsWith(":");
    aligns.push(left && right ? "center" : right ? "right" : left ? "left" : null);
  }
  return aligns;
}

/** A line that starts some other block, so a table can't run through it. */
const BLOCK_START = /^\s*(#{1,6}\s|```|>)/;

/**
 * Parse a table whose header row is `lines[start]`, or return `null` when the
 * lines there are not a table.
 */
export function parseTable(lines: string[], start: number): Table | null {
  const headLine = lines[start];
  const delimLine = lines[start + 1];
  if (headLine === undefined || delimLine === undefined) return null;
  if (!headLine.includes("|")) return null;

  const aligns = parseAligns(delimLine);
  if (!aligns) return null;
  const header = splitRow(headLine);
  if (header.length !== aligns.length) return null;

  const rows: string[][] = [];
  let i = start + 2;
  while (
    i < lines.length &&
    lines[i].trim() !== "" &&
    lines[i].includes("|") &&
    !BLOCK_START.test(lines[i])
  ) {
    const cells = splitRow(lines[i]);
    // Short rows are padded, long ones cut: the grid stays rectangular.
    rows.push(Array.from({ length: header.length }, (_, k) => cells[k] ?? ""));
    i += 1;
  }

  return { header, aligns, rows, end: i };
}
