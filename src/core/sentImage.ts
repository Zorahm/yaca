import type { SentFile } from "./bindings/SentFile";

/** An image in the shared gallery can refer to a delivery instead of a live path. */
export function sentImagePath(file: Pick<SentFile, "id" | "name">): string {
  return `sent://${file.id}/${encodeURIComponent(file.name)}`;
}

export function sentImage(path: string): { id: string; name: string } | null {
  const match = /^sent:\/\/(delivery_[a-f0-9]{32})\/(.+)$/.exec(path);
  if (!match) return null;
  try { return { id: match[1], name: decodeURIComponent(match[2]) }; }
  catch { return null; }
}
