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

import { openUrl } from "@tauri-apps/plugin-opener";
import { useRef, type MouseEvent } from "react";

import { MarkdownContent } from "./MarkdownContent";
import { isOpenable } from "./markdownTree";

export function MarkdownDoc({ text }: { text: string }) {
  const root = useRef<HTMLDivElement>(null);

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
      <MarkdownContent text={text} documentView />
    </div>
  );
}
