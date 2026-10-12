/**
 * The `@` file mention (§7.2), as pure functions of the field.
 *
 * The menu is a function of the field's text and caret, the same as the slash
 * menu in `slash.ts`: nothing here remembers that a menu is open, so a menu
 * cannot disagree with the text it is drawn over. A mention is the `@` that
 * starts a word, and the text between it and the caret. Picking a file takes
 * that text out of the field and makes the file a chip above it.
 */

/** A mention in the field: the `@` at `start`, and the query up to the caret. */
export type Mention = { start: number; query: string };

/** The answer to a search, as the feed keeps it: the query it answers too. */
export type FoundFiles = { query: string; paths: string[] };

const WHITESPACE = /\s/;

/**
 * The mention the caret is in, or `null` when there is none.
 *
 * The `@` must start a word: the beginning of the field, or follow whitespace.
 * An address like `me@host` is not a mention. A space after the `@` ends the
 * mention, as it ends a slash command: what follows is no longer a name.
 */
export function mentionAt(text: string, caret: number): Mention | null {
  const before = text.slice(0, caret);
  const start = before.lastIndexOf("@");
  if (start < 0) return null;
  if (start > 0 && !WHITESPACE.test(before[start - 1])) return null;
  const query = before.slice(start + 1);
  if (WHITESPACE.test(query)) return null;
  return { start, query };
}

/**
 * The field with the mention taken out, and where the caret goes then.
 *
 * Only the `@query` text leaves: what is typed after the caret stays put.
 */
export function withoutMention(
  text: string,
  mention: Mention,
  caret: number,
): { text: string; caret: number } {
  return {
    text: text.slice(0, mention.start) + text.slice(caret),
    caret: mention.start,
  };
}

/** The answer that belongs to `query`, or `null` while it is still on its way. */
export function answerFor(found: FoundFiles | null, query: string): string[] | null {
  return found !== null && found.query === query ? found.paths : null;
}

/** Moves the highlight by `by` rows, wrapping at both ends. `count` is above zero. */
export function step(at: number, by: number, count: number): number {
  return (((at + by) % count) + count) % count;
}

/**
 * Identifies one open mention, so that closing it with `Esc` stays closed until
 * the text after the `@` changes. Position and query together: the same query
 * typed again after a space is a new mention.
 */
export function mentionKey(mention: Mention): string {
  return `${mention.start}:${mention.query}`;
}
