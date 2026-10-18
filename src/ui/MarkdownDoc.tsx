/**
 * A Markdown file drawn as a document, for the file panel's «Render» view.
 *
 * It walks the tree from `markdownTree.ts` and builds React nodes, so nothing
 * from the file ever reaches `innerHTML`. Links to the web and to mail go to
 * the system through the opener plugin; links to a heading (`#slug`) scroll
 * inside the document; anything else (a relative path) stays plain text with
 * its target in the tooltip. Images are not fetched — a file may point at a
 * tracker or at a path this window cannot read — and show their alt text.
 */

import { Image as ImageIcon } from "@phosphor-icons/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useMemo, useRef, type MouseEvent, type ReactNode } from "react";

import { CodeBlock } from "./CodeBlock";
import { isOpenable, parseMarkdown, type Block, type Inline } from "./markdownTree";

export function MarkdownDoc({ text }: { text: string }) {
  const root = useRef<HTMLDivElement>(null);
  const tree = useMemo(() => parseMarkdown(text), [text]);

  const onClick = (event: MouseEvent) => {
    const link = (event.target as HTMLElement).closest("a");
    if (!link) return;
    event.preventDefault();
    const href = link.getAttribute("data-href") ?? "";
    if (isOpenable(href)) {
      void openUrl(href).catch(() => {});
    } else if (href.startsWith("#")) {
      const id = decodeURIComponent(href.slice(1));
      root.current?.querySelector(`[id="${CSS.escape(id)}"]`)?.scrollIntoView({ block: "start" });
    }
  };

  return (
    <div className="md-doc" ref={root} onClick={onClick}>
      {blocks(tree)}
    </div>
  );
}

function blocks(list: Block[], tight = false): ReactNode[] {
  return list.map((block, key) => {
    switch (block.t) {
      case "heading": {
        const Tag = `h${block.level}` as "h1";
        return (
          <Tag key={key} id={block.id}>
            {inline(block.c)}
          </Tag>
        );
      }
      case "para":
        return tight ? (
          <span key={key} className="md-tight">
            {inline(block.c)}
          </span>
        ) : (
          <p key={key}>{inline(block.c)}</p>
        );
      case "code":
        return <CodeBlock key={key} text={block.text} info={block.info} />;
      case "quote":
        return <blockquote key={key}>{blocks(block.blocks)}</blockquote>;
      case "hr":
        return <hr key={key} />;
      case "list": {
        const items = block.items.map((item, k) => (
          <li key={k} className={item.task === null ? undefined : "md-task"}>
            {item.task !== null && <input type="checkbox" checked={item.task} readOnly disabled />}
            {blocks(item.blocks, !block.loose)}
          </li>
        ));
        return block.ordered ? (
          <ol key={key} start={block.start}>
            {items}
          </ol>
        ) : (
          <ul key={key}>{items}</ul>
        );
      }
      case "table":
        return (
          <div className="md-table-wrap" key={key}>
            <table>
              <thead>
                <tr>
                  {block.header.map((cell, k) => (
                    <th key={k} style={{ textAlign: block.aligns[k] ?? undefined }}>
                      {inline(cell)}
                    </th>
                  ))}
                </tr>
              </thead>
              {block.rows.length > 0 && (
                <tbody>
                  {block.rows.map((row, r) => (
                    <tr key={r}>
                      {row.map((cell, k) => (
                        <td key={k} style={{ textAlign: block.aligns[k] ?? undefined }}>
                          {inline(cell)}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              )}
            </table>
          </div>
        );
    }
  });
}

function inline(list: Inline[]): ReactNode[] {
  return list.map((node, key) => {
    switch (node.t) {
      case "text":
        return node.v;
      case "code":
        return (
          <code className="code-inline" key={key}>
            {node.v}
          </code>
        );
      case "strong":
        return <strong key={key}>{inline(node.c)}</strong>;
      case "em":
        return <em key={key}>{inline(node.c)}</em>;
      case "del":
        return <del key={key}>{inline(node.c)}</del>;
      case "br":
        return <br key={key} />;
      case "image":
        return (
          <span className="md-image" key={key} title={node.src}>
            <ImageIcon size={13} />
            {node.alt}
          </span>
        );
      case "link": {
        const live = isOpenable(node.href) || node.href.startsWith("#");
        return (
          <a
            key={key}
            href={live ? node.href : undefined}
            data-href={node.href}
            title={node.title || node.href}
            className={live ? undefined : "md-dead"}
          >
            {inline(node.c)}
          </a>
        );
      }
    }
  });
}
