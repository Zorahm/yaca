import { Fragment, type ReactNode } from "react";

export type OpenReference = (path: string, line?: number) => void;

export function fileReference(text: string): { path: string; line: number } | null {
  if (text.includes("://") || /[\n<>]/.test(text)) return null;
  const match = /^(.+\.[\w-]+):([1-9]\d*)(?::[1-9]\d*)?$/.exec(text);
  const line = Number(match?.[2]);
  return match && Number.isSafeInteger(line) ? { path: match[1], line } : null;
}

export function referenceNode(text: string, onOpen: OpenReference): ReactNode {
  const location = fileReference(text);
  return location ? <button className="file-reference code-inline" onClick={() => onOpen(location.path, location.line)}>{text}</button> : text;
}

export function referenceText(text: string, onOpen?: OpenReference): ReactNode {
  if (!onOpen) return text;
  const pattern = /(?<![\w:/\\])(?:[A-Za-z]:[\\/])?[\w@./\\-]+\.[\w-]+:[1-9]\d*(?::[1-9]\d*)?/g;
  const parts: ReactNode[] = [];
  let at = 0;
  for (const match of text.matchAll(pattern)) {
    parts.push(text.slice(at, match.index));
    parts.push(<Fragment key={match.index}>{referenceNode(match[0], onOpen)}</Fragment>);
    at = match.index! + match[0].length;
  }
  parts.push(text.slice(at));
  return parts;
}
