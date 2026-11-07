import { type ReactNode } from "react";

import { type OpenReference } from "./FileReferences";
import { MarkdownContent } from "./MarkdownContent";

export function Markdown({ text, onOpenFile }: { text: string; onOpenFile?: OpenReference }): ReactNode {
  return (
    <div className="agent">
      <MarkdownContent text={text} onOpenFile={onOpenFile} />
    </div>
  );
}
