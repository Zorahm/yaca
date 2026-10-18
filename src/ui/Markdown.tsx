/**
 * Markdown in the agent's text (§5.2).
 *
 * Its own parser, and not out of principle: it builds React nodes, not HTML, so
 * there is no `dangerouslySetInnerHTML` here at all — and so no sanitiser is
 * needed for text that came from the model.
 *
 * §12 requires an unclosed code block to render as code: the text arrives in
 * deltas, and the opening fence lives on the screen before the closing one. Here
 * this is not a caveat but an ordinary case — a code block is closed by the end of
 * the text.
 *
 * Links are deliberately not parsed. There is nowhere to open them: the browser in
 * the window is driven by the shell, and until such a command exists, underlined
 * text that leads nowhere is an interface lying about the state (§0).
 */

import type { CSSProperties, ReactNode } from "react";

import { CodeBlock } from "./CodeBlock";
import { type Align, parseTable } from "./markdownTable";

export function Markdown({ text }: { text: string }): ReactNode {
  return <div className="agent">{blocks(text)}</div>;
}

function blocks(text: string): ReactNode[] {
  const lines = text.split("\n");
  const out: ReactNode[] = [];
  let paragraph: string[] = [];
  let list: { ordered: boolean; items: string[] } | null = null;

  const flushParagraph = () => {
    if (paragraph.length === 0) return;
    out.push(<p key={out.length}>{inline(paragraph.join("\n"))}</p>);
    paragraph = [];
  };
  const flushList = () => {
    if (!list) return;
    const items = list.items.map((item, i) => <li key={i}>{inline(item)}</li>);
    out.push(
      list.ordered ? (
        <ol key={out.length}>{items}</ol>
      ) : (
        <ul key={out.length}>{items}</ul>
      ),
    );
    list = null;
  };
  const flush = () => {
    flushParagraph();
    flushList();
  };

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];

    // A fence: collect up to the closing one or to the end — an unclosed block is
    // still code, and it must not flicker (§12).
    const fence = /^\s*```(\S*)\s*$/.exec(line);
    if (fence) {
      flush();
      const body: string[] = [];
      i += 1;
      while (i < lines.length && !/^\s*```\s*$/.test(lines[i])) {
        body.push(lines[i]);
        i += 1;
      }
      out.push(<CodeBlock key={out.length} text={body.join("\n")} info={fence[1]} />);
      continue;
    }

    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    if (heading) {
      flush();
      const level = Math.min(heading[1].length, 3);
      const Tag = (["h1", "h2", "h3"] as const)[level - 1];
      out.push(<Tag key={out.length}>{inline(heading[2])}</Tag>);
      continue;
    }

    // A table needs its delimiter row; until that line arrives the header
    // is an ordinary paragraph.
    const table = parseTable(lines, i);
    if (table) {
      flush();
      out.push(
        <div className="agent-table-wrap" key={out.length}>
          <table className="agent-table">
            <thead>
              <tr>
                {table.header.map((cell, k) => (
                  <th key={k} style={alignStyle(table.aligns[k])}>
                    {inline(cell)}
                  </th>
                ))}
              </tr>
            </thead>
            {table.rows.length > 0 && (
              <tbody>
                {table.rows.map((row, j) => (
                  <tr key={j}>
                    {row.map((cell, k) => (
                      <td key={k} style={alignStyle(table.aligns[k])}>
                        {inline(cell)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            )}
          </table>
        </div>,
      );
      i = table.end - 1;
      continue;
    }

    const bullet = /^\s*[-*+]\s+(.*)$/.exec(line);
    const numbered = /^\s*\d+[.)]\s+(.*)$/.exec(line);
    if (bullet || numbered) {
      flushParagraph();
      const ordered = numbered !== null;
      if (!list || list.ordered !== ordered) {
        flushList();
        list = { ordered, items: [] };
      }
      list.items.push((bullet ?? numbered)![1]);
      continue;
    }

    const quote = /^\s*>\s?(.*)$/.exec(line);
    if (quote) {
      flush();
      out.push(<blockquote key={out.length}>{inline(quote[1])}</blockquote>);
      continue;
    }

    if (line.trim() === "") {
      flush();
      continue;
    }

    flushList();
    paragraph.push(line);
  }

  flush();
  return out;
}

function alignStyle(align: Align): CSSProperties | undefined {
  return align ? { textAlign: align } : undefined;
}

/** Inline code, bold, italics. Everything else is text as it is. */
function inline(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  // The order matters: code first, so that `**` inside it stays as itself.
  const pattern = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*(?!\s)[^*\n]*[^*\s]\*)|((?<!\w)_(?!\s)[^_\n]*[^_\s]_(?!\w))/g;

  let at = 0;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text)) !== null) {
    if (match.index > at) out.push(text.slice(at, match.index));
    const token = match[0];

    if (token.startsWith("`")) {
      out.push(
        <code className="code-inline" key={out.length}>
          {token.slice(1, -1)}
        </code>,
      );
    } else if (token.startsWith("**")) {
      out.push(<strong key={out.length}>{token.slice(2, -2)}</strong>);
    } else {
      out.push(<em key={out.length}>{token.slice(1, -1)}</em>);
    }
    at = match.index + token.length;
  }

  if (at < text.length) out.push(text.slice(at));
  return out;
}
