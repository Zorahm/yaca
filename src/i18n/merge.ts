/**
 * Joins the sections of a dictionary into the dictionary.
 *
 * A language is written as several files — one per area of the window — and
 * read as one flat map, so `t("side.new")` does not care which file the key
 * lives in. Keys are flat and prefixed (`side.`, `mcp.`); the prefix is what
 * assigns a key to a section.
 */

export type Section = Readonly<Record<string, string>>;

/**
 * `A | B` into `A & B`: the dictionary has every key of every section.
 *
 * Through a function parameter because that is the one position where
 * TypeScript infers an intersection from a union.
 */
type Intersect<U> = (U extends unknown ? (section: U) => void : never) extends (
  section: infer I,
) => void
  ? I
  : never;

export type Merged<T extends readonly Section[]> = Intersect<T[number]>;

export function merge<const T extends readonly Section[]>(sections: T): Merged<T> {
  // `Object.assign` is typed for four sources at most; the result is exactly
  // the intersection, which is what `Merged` says.
  return Object.assign({}, ...sections) as Merged<T>;
}

/**
 * Keys declared in more than one section.
 *
 * The type cannot see them: an intersection of two objects with the same key
 * is still an object with that key, and `merge` would let the later section
 * win silently. The test asks this instead.
 */
export function duplicates(sections: readonly Section[]): string[] {
  const seen = new Set<string>();
  const twice = new Set<string>();
  for (const section of sections) {
    for (const key of Object.keys(section)) {
      (seen.has(key) ? twice : seen).add(key);
    }
  }
  return [...twice];
}
