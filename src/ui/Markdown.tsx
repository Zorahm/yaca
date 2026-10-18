/**
 * Markdown в тексте агента (§5.2).
 *
 * Свой разбор, и не из принципа: он строит узлы React, а не HTML, поэтому
 * `dangerouslySetInnerHTML` тут нет вовсе — а с ним не нужен и санитайзер для
 * текста, который пришёл от модели.
 *
 * §12 требует, чтобы незакрытый блок кода рендерился как код: текст приходит
 * дельтами, и открывающая ограда живёт на экране раньше закрывающей. Здесь это
 * не оговорка, а обычный случай — блок кода закрывается концом текста.
 *
 * Ссылки не разбираются намеренно. Открыть их некуда: браузер из окна дёргает
 * оболочка, и пока такой команды нет, подчёркнутый текст, который никуда не
 * ведёт, — это интерфейс, врущий про состояние (§0).
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

    // Ограда: собираем до закрывающей или до конца — незакрытый блок всё
    // равно код, и мигать он не должен (§12).
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

/** Инлайн-код, полужирный, курсив. Остальное — текст как есть. */
function inline(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  // Порядок важен: код первым, чтобы `**` внутри него остались собой.
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
