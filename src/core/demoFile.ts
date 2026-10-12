/**
 * The file the demo stand shows in the side panel (§6.4).
 *
 * In the real window the full view of a file comes from the core, which reads
 * it from disk when asked (`readFileView`). The stand has no core, and a panel
 * that answered nothing sat on "reading the file…" forever — an interface lying
 * about its state. This module is the stand's stand-in for that one exchange: a
 * small file with two changed regions, shaped like the core's reply.
 *
 * The panel only shows. There is nothing to decide about a hunk: the edit is
 * already in the working tree, and the one way back is a checkpoint (§5.8).
 */

import type { DiffLine } from "./bindings/DiffLine";
import type { FileFact } from "./bindings/FileFact";
import type { FileView } from "./bindings/FileView";
import type { ViewSegment } from "./bindings/ViewSegment";

export const DEMO_FILE_PATH = "src/segments.ts";

/** A region the agent changed: the lines before and after. */
interface Region {
  old: string[];
  new: string[];
}

type Chunk = string[] | Region;

function isRegion(chunk: Chunk): chunk is Region {
  return !Array.isArray(chunk);
}

const CHUNKS: Chunk[] = [
  [
    'import { offset } from "./zone";',
    "",
    "export interface Segment {",
    "  start: Date;",
    "  end: Date;",
    "}",
    "",
  ],
  {
    old: ["// Разбор одного сегмента."],
    new: ["// Разбор одного сегмента: время приходит в UTC, зона — отдельным полем."],
  },
  [
    "export function parseSegment(raw: string): Date {",
    "  if (raw.length === 0) {",
    '    throw new Error("пустая строка");',
    "  }",
  ],
  {
    old: ["  return new Date(raw);"],
    new: ["  const at = Date.parse(raw);", "  return new Date(at + offset(raw));"],
  },
  ["}", "", "export function parseAll(rows: string[]): Date[] {", "  return rows.map(parseSegment);", "}"],
];

function line(kind: DiffLine["kind"], oldLine: number | null, newLine: number | null, text: string): DiffLine {
  return { kind, oldLine, newLine, text, spans: [] };
}

/** The panel's answer to `readFileView`; `null` for a file the demo never changed. */
export function demoFileView(path: string): FileView | null {
  if (path !== DEMO_FILE_PATH) return null;

  const segments: ViewSegment[] = [];
  let added = 0;
  let removed = 0;
  let oldAt = 1;
  let newAt = 1;
  let plain: DiffLine[] = [];

  const flush = () => {
    if (plain.length > 0) segments.push({ kind: "unchanged", lines: plain });
    plain = [];
  };

  for (const chunk of CHUNKS) {
    if (!isRegion(chunk)) {
      for (const text of chunk) {
        plain.push(line("context", oldAt, newAt, text));
        oldAt += 1;
        newAt += 1;
      }
      continue;
    }

    flush();
    const oldStart = oldAt;
    const newStart = newAt;
    const lines: DiffLine[] = [];
    for (const text of chunk.old) {
      lines.push(line("del", oldAt, null, text));
      oldAt += 1;
    }
    for (const text of chunk.new) {
      lines.push(line("add", null, newAt, text));
      newAt += 1;
    }
    removed += chunk.old.length;
    added += chunk.new.length;
    segments.push({
      kind: "hunk",
      hunk: {
        oldStart,
        oldLines: chunk.old.length,
        newStart,
        newLines: chunk.new.length,
        lines,
      },
    });
  }
  flush();

  const fact: FileFact = { path, kind: "modified", movedFrom: null, added, removed };
  return { fact, segments, truncated: false };
}
