import { Image as ImageIcon } from "@phosphor-icons/react";
import { Fragment, useMemo, type ReactNode } from "react";

import { CodeBlock } from "./CodeBlock";
import { fileReference, referenceNode, referenceText, type OpenReference } from "./FileReferences";
import { isOpenable, parseMarkdown, plainText, type Block, type Inline } from "./markdownTree";

type RenderOptions = {
  documentView?: boolean;
  onOpenFile?: OpenReference;
};

export function MarkdownContent({ text, ...options }: RenderOptions & { text: string }) {
  const tree = useMemo(() => parseMarkdown(text), [text]);
  return <>{renderBlocks(tree, options)}</>;
}

function renderBlocks(list: Block[], options: RenderOptions, tight = false): ReactNode[] {
  const inline = (nodes: Inline[]) => renderInline(nodes, options);
  return list.map((block, key) => {
    switch (block.t) {
      case "heading": {
        const level = options.documentView ? block.level : Math.min(block.level, 3);
        const Tag = `h${level}` as "h1";
        return (
          <Tag key={key} id={options.documentView ? block.id : undefined}>
            {inline(block.c)}
          </Tag>
        );
      }
      case "para":
        return tight ? (
          <span key={key} className="md-tight">{inline(block.c)}</span>
        ) : (
          <p key={key}>{inline(block.c)}</p>
        );
      case "code":
        return <CodeBlock key={key} text={block.text} info={block.info} />;
      case "quote":
        return <blockquote key={key}>{renderBlocks(block.blocks, options)}</blockquote>;
      case "hr":
        return <hr key={key} />;
      case "list": {
        const Tag = block.ordered ? "ol" : "ul";
        return (
          <Tag key={key} start={block.ordered ? block.start : undefined} className={block.loose ? "md-loose" : undefined}>
            {block.items.map((item, k) => (
              <li key={k} className={item.task === null ? undefined : "md-task"}>
                {item.task !== null && <input type="checkbox" checked={item.task} readOnly disabled />}
                {renderBlocks(item.blocks, options, !block.loose)}
              </li>
            ))}
          </Tag>
        );
      }
      case "table":
        return (
          <div className={options.documentView ? "md-table-wrap" : "agent-table-wrap"} key={key}>
            <table className={options.documentView ? undefined : "agent-table"}>
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

function renderInline(list: Inline[], options: RenderOptions): ReactNode[] {
  const { onOpenFile, documentView } = options;
  return list.map((node, key) => {
    switch (node.t) {
      case "text":
        return <Fragment key={key}>{referenceText(node.v, onOpenFile)}</Fragment>;
      case "code": {
        const code = onOpenFile && fileReference(node.v)
          ? referenceNode(node.v, onOpenFile)
          : <code className="code-inline">{node.v}</code>;
        return (
          <Fragment key={key}>{code}</Fragment>
        );
      }
      case "strong":
      case "em":
      case "del": {
        const Tag = node.t;
        return <Tag key={key}>{renderInline(node.c, options)}</Tag>;
      }
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
        if (!documentView) {
          const target = node.href.replace(/^mailto:/i, "");
          return (
            <Fragment key={key}>
              {renderInline(node.c, options)}
              {target && plainText(node.c) !== target && <span className="md-link-target"> ({target})</span>}
            </Fragment>
          );
        }
        const live = isOpenable(node.href) || node.href.startsWith("#");
        return (
          <a
            key={key}
            href={live ? node.href : undefined}
            data-href={node.href}
            title={node.title || node.href}
            className={live ? undefined : "md-dead"}
          >
            {renderInline(node.c, options)}
          </a>
        );
      }
    }
  });
}
