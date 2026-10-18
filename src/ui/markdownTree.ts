/**
 * A Markdown document parser for the file panel: CommonMark plus the GFM
 * parts people actually write (tables, task lists, strikethrough, autolinks).
 *
 * Unlike `Markdown.tsx` — which renders a streaming agent reply and keeps to a
 * small subset — this one reads a whole file, so it handles nested lists,
 * block quotes holding other blocks, setext headings, rules, reference links
 * and hard breaks. It produces a plain tree, not HTML: the renderer builds
 * React nodes from it, so raw HTML in the file is never injected.
 */

import { type Align, parseTable } from "./markdownTable";

export type Inline =
  | { t: "text"; v: string }
  | { t: "code"; v: string }
  | { t: "strong" | "em" | "del"; c: Inline[] }
  | { t: "link"; href: string; title: string; c: Inline[] }
  | { t: "image"; src: string; alt: string }
  | { t: "br" };

export type ListItem = {
  /** `null` — an ordinary item; `true`/`false` — a task box, checked or not. */
  task: boolean | null;
  blocks: Block[];
};

export type Block =
  | { t: "heading"; level: number; id: string; c: Inline[] }
  | { t: "para"; c: Inline[] }
  | { t: "code"; info: string; text: string }
  | { t: "quote"; blocks: Block[] }
  | { t: "list"; ordered: boolean; start: number; loose: boolean; items: ListItem[] }
  | { t: "table"; aligns: Align[]; header: Inline[][]; rows: Inline[][][] }
  | { t: "hr" };

type Refs = Map<string, { href: string; title: string }>;

/** Parse a whole document. */
export function parseMarkdown(source: string): Block[] {
  const lines = source.replace(/\r\n?/g, "\n").replace(/^\uFEFF/, "").split("\n");
  const refs: Refs = new Map();
  const body = lines.map((line) => line.replace(/^(\s*)\t/g, "$1    "));
  const slugs = new Map<string, number>();
  return parseBlocks(stripDefinitions(body, refs), refs, slugs);
}

// --- blocks ---------------------------------------------------------------

const FENCE = /^( {0,3})(`{3,}|~{3,})\s*([^`\s]*)[^`]*$/;
const ATX = /^ {0,3}(#{1,6})(?:\s+(.*?))?(?:\s+#+)?\s*$/;
const HR = /^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$/;
const QUOTE = /^ {0,3}>/;
const MARKER = /^( *)([-*+]|\d{1,9}[.)])( +|$)(.*)$/;
const SETEXT = /^ {0,3}(=+|-+)[ \t]*$/;
const DEFINITION = /^ {0,3}\[([^\]]+)\]:\s*<?([^\s>]+)>?(?:\s+(?:"([^"]*)"|'([^']*)'|\(([^)]*)\)))?\s*$/;

/** Pull `[label]: url "title"` lines out of the text and into `refs`. */
function stripDefinitions(lines: string[], refs: Refs): string[] {
  const out: string[] = [];
  let fence: string | null = null;
  for (const line of lines) {
    const open = FENCE.exec(line);
    if (open) {
      if (fence === null) fence = open[2][0];
      else if (open[2][0] === fence) fence = null;
    }
    const def = fence === null ? DEFINITION.exec(line) : null;
    if (def) {
      const key = def[1].trim().toLowerCase();
      if (!refs.has(key)) refs.set(key, { href: def[2], title: def[3] ?? def[4] ?? def[5] ?? "" });
      continue;
    }
    out.push(line);
  }
  return out;
}

function indentOf(line: string): number {
  return line.length - line.trimStart().length;
}

/** A line that begins a block of its own, so a paragraph cannot run into it. */
function interrupts(lines: string[], at: number): boolean {
  const line = lines[at];
  if (FENCE.test(line) || HR.test(line) || QUOTE.test(line)) return true;
  const heading = ATX.exec(line);
  if (heading) return true;
  const marker = MARKER.exec(line);
  // Only a non-empty item that starts a list (`1.` or a bullet) may cut a paragraph.
  if (marker && marker[4] !== "" && (!/^\d/.test(marker[2]) || /^1[.)]/.test(marker[2]))) {
    return true;
  }
  return line.includes("|") && parseTable(lines, at) !== null;
}

function parseBlocks(lines: string[], refs: Refs, slugs: Map<string, number>): Block[] {
  const out: Block[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];
    if (line.trim() === "") {
      i += 1;
      continue;
    }

    // A fenced block runs to its closing fence, or to the end of the text.
    const fence = FENCE.exec(line);
    if (fence) {
      const ticks = fence[2];
      const close = new RegExp(`^ {0,3}${ticks[0] === "`" ? "`" : "~"}{${ticks.length},}\\s*$`);
      const body: string[] = [];
      i += 1;
      while (i < lines.length && !close.test(lines[i])) {
        body.push(lines[i].slice(Math.min(fence[1].length, indentOf(lines[i]))));
        i += 1;
      }
      i += 1;
      out.push({ t: "code", info: fence[3], text: body.join("\n") });
      continue;
    }

    if (/^ {4,}\S/.test(line)) {
      const body: string[] = [];
      while (i < lines.length && (lines[i].trim() === "" || /^ {4,}/.test(lines[i]))) {
        body.push(lines[i].slice(4));
        i += 1;
      }
      while (body.length > 0 && body[body.length - 1].trim() === "") body.pop();
      out.push({ t: "code", info: "", text: body.join("\n") });
      continue;
    }

    if (/^\s*<!--/.test(line)) {
      while (i < lines.length && !lines[i].includes("-->")) i += 1;
      i += 1;
      continue;
    }

    const heading = ATX.exec(line);
    if (heading) {
      out.push(headingOf(heading[1].length, heading[2] ?? "", refs, slugs));
      i += 1;
      continue;
    }

    if (HR.test(line)) {
      out.push({ t: "hr" });
      i += 1;
      continue;
    }

    if (QUOTE.test(line)) {
      const inner: string[] = [];
      while (i < lines.length && lines[i].trim() !== "") {
        // Lazy continuation: a plain line right under a quote line stays in it.
        if (!QUOTE.test(lines[i]) && interrupts(lines, i)) break;
        inner.push(lines[i].replace(/^ {0,3}> ?/, ""));
        i += 1;
      }
      out.push({ t: "quote", blocks: parseBlocks(inner, refs, slugs) });
      continue;
    }

    if (line.includes("|")) {
      const table = parseTable(lines, i);
      if (table) {
        out.push({
          t: "table",
          aligns: table.aligns,
          header: table.header.map((cell) => parseInline(cell, refs)),
          rows: table.rows.map((row) => row.map((cell) => parseInline(cell, refs))),
        });
        i = table.end;
        continue;
      }
    }

    if (MARKER.test(line)) {
      const list = parseList(lines, i, refs, slugs);
      out.push(list.block);
      i = list.end;
      continue;
    }

    // A paragraph; two or more spaces or a backslash at the end of a line are
    // a hard break, and `===` / `---` under it make it a heading.
    const para: string[] = [line];
    i += 1;
    let setext = 0;
    while (i < lines.length && lines[i].trim() !== "") {
      const under = SETEXT.exec(lines[i]);
      if (under) {
        setext = under[1][0] === "=" ? 1 : 2;
        i += 1;
        break;
      }
      if (interrupts(lines, i)) break;
      para.push(lines[i]);
      i += 1;
    }
    const text = para.map((row) => row.trim()).join("\n");
    if (setext > 0) {
      out.push(headingOf(setext, text, refs, slugs));
    } else {
      const hardBreaks = para.map((row, k) =>
        k < para.length - 1 && / {2,}$/.test(row) ? `${row.trim()}\\` : row.trim(),
      );
      out.push({ t: "para", c: parseInline(hardBreaks.join("\n"), refs) });
    }
  }

  return out;
}

function headingOf(level: number, text: string, refs: Refs, slugs: Map<string, number>): Block {
  const base = slugOf(text);
  const seen = slugs.get(base) ?? 0;
  slugs.set(base, seen + 1);
  return {
    t: "heading",
    level,
    id: seen === 0 ? base : `${base}-${seen}`,
    c: parseInline(text.trim(), refs),
  };
}

/** The anchor GitHub gives a heading: lower case, words joined by dashes. */
export function slugOf(text: string): string {
  return text
    .toLowerCase()
    .replace(/[`*_~[\]()]/g, "")
    .replace(/[^\p{L}\p{N}\s-]/gu, "")
    .trim()
    .replace(/\s/g, "-");
}

function parseList(
  lines: string[],
  start: number,
  refs: Refs,
  slugs: Map<string, number>,
): { block: Block; end: number } {
  const first = MARKER.exec(lines[start])!;
  const ordered = /^\d/.test(first[2]);
  const bulletChar = ordered ? first[2].slice(-1) : first[2];
  const base = first[1].length;
  const items: ListItem[] = [];
  let loose = false;
  let i = start;

  while (i < lines.length) {
    const m = MARKER.exec(lines[i]);
    if (!m || m[1].length > base + 3 || m[1].length < base) break;
    // Another kind of marker starts another list.
    const same = /^\d/.test(m[2]) === ordered && (ordered ? m[2].slice(-1) : m[2]) === bulletChar;
    if (!same) break;

    // Content sits past the marker; a marker followed by 5+ spaces is code.
    const pad = m[3].length > 4 ? 1 : m[3].length;
    const contentIndent = m[1].length + m[2].length + (m[4] === "" ? 1 : pad);
    const body: string[] = [m[4] === "" ? "" : " ".repeat(Math.max(0, m[3].length - pad)) + m[4]];
    i += 1;

    let blankBefore = false;
    while (i < lines.length) {
      const next = lines[i];
      if (next.trim() === "") {
        blankBefore = true;
        body.push("");
        i += 1;
        continue;
      }
      if (indentOf(next) >= contentIndent) {
        body.push(next.slice(contentIndent));
        blankBefore = false;
        i += 1;
        continue;
      }
      // A plain line right under text continues the item (lazy continuation).
      if (!blankBefore && !MARKER.test(next) && !interrupts(lines, i)) {
        body.push(next.trim());
        i += 1;
        continue;
      }
      break;
    }

    // Trailing blank lines belong to the gap between items, not to the item.
    let trailing = 0;
    while (body.length > 1 && body[body.length - 1] === "") {
      body.pop();
      trailing += 1;
    }
    const nextMarker = i < lines.length ? MARKER.exec(lines[i]) : null;
    if (trailing > 0 && nextMarker && nextMarker[1].length >= base) loose = true;
    if (body.some((row, k) => row === "" && k > 0 && k < body.length - 1)) {
      // A blank line between two blocks of one item makes the list loose too.
      if (parseBlocks(body, refs, slugs).length > 1) loose = true;
    }

    let task: boolean | null = null;
    const box = /^\[([ xX])\]\s+/.exec(body[0]);
    if (box) {
      task = box[1] !== " ";
      body[0] = body[0].slice(box[0].length);
    }
    items.push({ task, blocks: parseBlocks(body, refs, slugs) });

    // Blank lines between items are skipped; anything else ends the list.
    if (trailing > 0 && !(nextMarker && nextMarker[1].length >= base)) break;
  }

  return {
    block: {
      t: "list",
      ordered,
      start: ordered ? parseInt(first[2], 10) : 1,
      loose,
      items,
    },
    end: i,
  };
}

// --- inline ---------------------------------------------------------------

const ESCAPABLE = "\\`*_{}[]()#+-.!|~<>\"'&:/=^$%@,;?";
const ENTITIES: Record<string, string> = {
  amp: "&",
  lt: "<",
  gt: ">",
  quot: '"',
  apos: "'",
  nbsp: "\u00a0",
  copy: "©",
  hellip: "…",
  mdash: "—",
  ndash: "–",
};

function entity(name: string): string | null {
  if (name[0] === "#") {
    const code = name[1] === "x" || name[1] === "X" ? parseInt(name.slice(2), 16) : parseInt(name.slice(1), 10);
    if (!Number.isFinite(code) || code <= 0 || code > 0x10ffff) return null;
    return String.fromCodePoint(code);
  }
  return ENTITIES[name] ?? null;
}

const isSpace = (ch: string | undefined) => ch === undefined || /\s/.test(ch);
const isWord = (ch: string | undefined) => ch !== undefined && /[\p{L}\p{N}]/u.test(ch);

export function parseInline(text: string, refs: Refs = new Map()): Inline[] {
  const out: Inline[] = [];
  let buf = "";
  const flush = () => {
    if (buf !== "") out.push({ t: "text", v: buf });
    buf = "";
  };
  let i = 0;

  while (i < text.length) {
    const ch = text[i];

    if (ch === "\\") {
      const next = text[i + 1];
      if (next === "\n") {
        flush();
        out.push({ t: "br" });
        i += 2;
      } else if (next !== undefined && ESCAPABLE.includes(next)) {
        buf += next;
        i += 2;
      } else {
        buf += ch;
        i += 1;
      }
      continue;
    }

    if (ch === "\n") {
      buf = buf.replace(/ +$/, "");
      buf += "\n";
      i += 1;
      continue;
    }

    if (ch === "&") {
      const m = /^&(#[xX]?[0-9a-fA-F]+|[a-zA-Z]+);/.exec(text.slice(i, i + 12));
      const decoded = m ? entity(m[1]) : null;
      if (m && decoded !== null) {
        buf += decoded;
        i += m[0].length;
        continue;
      }
    }

    if (ch === "`") {
      let n = 1;
      while (text[i + n] === "`") n += 1;
      const run = "`".repeat(n);
      let close = text.indexOf(run, i + n);
      while (close !== -1 && text[close + n] === "`") {
        let len = n;
        while (text[close + len] === "`") len += 1;
        close = text.indexOf(run, close + len);
      }
      if (close !== -1) {
        flush();
        let code = text.slice(i + n, close).replace(/\n/g, " ");
        if (code.length > 2 && code.startsWith(" ") && code.endsWith(" ") && code.trim() !== "") {
          code = code.slice(1, -1);
        }
        out.push({ t: "code", v: code });
        i = close + n;
        continue;
      }
      buf += run;
      i += n;
      continue;
    }

    if (ch === "!" && text[i + 1] === "[") {
      const link = linkAt(text, i + 1, refs);
      if (link) {
        flush();
        out.push({ t: "image", src: link.href, alt: plainText(parseInline(link.label, refs)) });
        i = link.end;
        continue;
      }
    }

    if (ch === "[") {
      const link = linkAt(text, i, refs);
      if (link) {
        flush();
        out.push({
          t: "link",
          href: link.href,
          title: link.title,
          c: parseInline(link.label, refs),
        });
        i = link.end;
        continue;
      }
    }

    if (ch === "<") {
      const auto = /^<((?:https?:\/\/|mailto:)[^\s<>]+)>/.exec(text.slice(i));
      if (auto) {
        flush();
        out.push({ t: "link", href: auto[1], title: "", c: [{ t: "text", v: auto[1] }] });
        i += auto[0].length;
        continue;
      }
      const mail = /^<([^\s<>@]+@[^\s<>@]+\.[^\s<>@]+)>/.exec(text.slice(i));
      if (mail) {
        flush();
        out.push({ t: "link", href: `mailto:${mail[1]}`, title: "", c: [{ t: "text", v: mail[1] }] });
        i += mail[0].length;
        continue;
      }
      if (/^<br\s*\/?>/i.test(text.slice(i, i + 6))) {
        flush();
        out.push({ t: "br" });
        i += /^<br\s*\/?>/i.exec(text.slice(i))![0].length;
        continue;
      }
    }

    if ((ch === "h" || ch === "w") && !isWord(text[i - 1])) {
      const bare = /^(?:https?:\/\/|www\.)[^\s<]+/.exec(text.slice(i));
      if (bare) {
        // Closing punctuation after a bare address belongs to the sentence.
        const url = bare[0].replace(/[.,:;!?*_~'"]+$/, "").replace(/\)+$/, (run) => {
          const opens = (bare[0].match(/\(/g) ?? []).length;
          const closes = (bare[0].match(/\)/g) ?? []).length;
          return closes > opens ? run.slice(0, run.length - (closes - opens)) : run;
        });
        if (url.length > (url.startsWith("www.") ? 6 : 8)) {
          flush();
          out.push({
            t: "link",
            href: url.startsWith("www.") ? `https://${url}` : url,
            title: "",
            c: [{ t: "text", v: url }],
          });
          i += url.length;
          continue;
        }
      }
    }

    if (ch === "*" || ch === "_" || ch === "~") {
      let n = 1;
      while (text[i + n] === ch) n += 1;
      const before = text[i - 1];
      const after = text[i + n];
      const opens =
        !isSpace(after) && (ch !== "_" || !isWord(before)) && (ch !== "~" || n <= 2);
      if (opens) {
        const take = ch === "~" ? n : Math.min(n, 3);
        const close = findClosing(text, i + n, ch, take);
        if (close !== -1) {
          flush();
          const inner = parseInline(text.slice(i + n, close), refs);
          const kind = ch === "~" ? "del" : take === 1 ? "em" : "strong";
          out.push(
            take === 3
              ? { t: "em", c: [{ t: "strong", c: inner }] }
              : { t: kind, c: inner },
          );
          i = close + take;
          // Surplus delimiters in front of the span stay literal text.
          continue;
        }
      }
      buf += ch.repeat(n);
      i += n;
      continue;
    }

    buf += ch;
    i += 1;
  }

  flush();
  return out;
}

/** Index of a run of exactly `take` `ch` that can close a span, or -1. */
function findClosing(text: string, from: number, ch: string, take: number): number {
  let i = from;
  while (i < text.length) {
    if (text[i] === "\\") {
      i += 2;
      continue;
    }
    if (text[i] === "`") {
      let n = 1;
      while (text[i + n] === "`") n += 1;
      const close = text.indexOf("`".repeat(n), i + n);
      i = close === -1 ? i + n : close + n;
      continue;
    }
    if (text[i] === ch) {
      let n = 1;
      while (text[i + n] === ch) n += 1;
      const before = text[i - 1];
      const after = text[i + n];
      if (n >= take && !isSpace(before) && (ch !== "_" || !isWord(after)) && i > from) {
        return i + n - take;
      }
      i += n;
      continue;
    }
    i += 1;
  }
  return -1;
}

/** `[label](href "title")`, `[label][ref]` or `[ref]` starting at `text[at] === "["`. */
function linkAt(
  text: string,
  at: number,
  refs: Refs,
): { label: string; href: string; title: string; end: number } | null {
  let depth = 0;
  let close = -1;
  for (let i = at; i < text.length; i += 1) {
    const ch = text[i];
    if (ch === "\\") {
      i += 1;
    } else if (ch === "`") {
      const end = text.indexOf("`", i + 1);
      if (end !== -1) i = end;
    } else if (ch === "[") {
      depth += 1;
    } else if (ch === "]") {
      depth -= 1;
      if (depth === 0) {
        close = i;
        break;
      }
    }
  }
  if (close === -1) return null;
  const label = text.slice(at + 1, close);

  if (text[close + 1] === "(") {
    const dest = destination(text, close + 2);
    if (dest) return { label, href: dest.href, title: dest.title, end: dest.end };
    return null;
  }

  const full = /^\[([^\]]*)\]/.exec(text.slice(close + 1));
  const key = (full && full[1] !== "" ? full[1] : label).trim().toLowerCase().replace(/\s+/g, " ");
  const ref = refs.get(key) ?? refs.get(key.replace(/ /g, " "));
  if (!ref) return null;
  return {
    label,
    href: ref.href,
    title: ref.title,
    end: close + 1 + (full ? full[0].length : 0),
  };
}

/** The `(url "title")` part of an inline link, from just after `(`. */
function destination(
  text: string,
  from: number,
): { href: string; title: string; end: number } | null {
  let i = from;
  while (text[i] === " " || text[i] === "\n") i += 1;
  let href = "";
  if (text[i] === "<") {
    const end = text.indexOf(">", i);
    if (end === -1) return null;
    href = text.slice(i + 1, end);
    i = end + 1;
  } else {
    let parens = 0;
    const start = i;
    while (i < text.length && !/\s/.test(text[i])) {
      if (text[i] === "\\") i += 1;
      else if (text[i] === "(") parens += 1;
      else if (text[i] === ")") {
        if (parens === 0) break;
        parens -= 1;
      }
      i += 1;
    }
    href = text.slice(start, i).replace(/\\(.)/g, "$1");
  }
  while (text[i] === " " || text[i] === "\n") i += 1;
  let title = "";
  const quote = text[i];
  if (quote === '"' || quote === "'" || quote === "(") {
    const closer = quote === "(" ? ")" : quote;
    const end = text.indexOf(closer, i + 1);
    if (end === -1) return null;
    title = text.slice(i + 1, end);
    i = end + 1;
    while (text[i] === " " || text[i] === "\n") i += 1;
  }
  if (text[i] !== ")") return null;
  return { href, title, end: i + 1 };
}

/** The text of an inline run with every mark dropped — an image's alt. */
export function plainText(inline: Inline[]): string {
  return inline
    .map((node) => {
      switch (node.t) {
        case "text":
        case "code":
          return node.v;
        case "image":
          return node.alt;
        case "br":
          return " ";
        default:
          return plainText(node.c);
      }
    })
    .join("");
}

/** Links the panel will hand to the system: web and mail only. */
export function isOpenable(href: string): boolean {
  return /^(https?:\/\/|mailto:)/i.test(href);
}
