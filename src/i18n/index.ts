/**
 * UI translation (§14).
 *
 * Not a single line of text in the components — keys only. Keys are typed from
 * the Russian dictionary, so a typo or a forgotten translation is caught by the
 * compiler, not by a person who one day sees an English word in the middle of
 * the feed. It is the same rule by which the core's types are generated from Rust.
 *
 * Our own instead of `i18next`: we need keys, substitution and plural forms
 * (§14), and no namespaces, backend or suspense. A hundred lines against a
 * dependency tree.
 */

import { isLocale, LOCALES, type Locale } from "./locales";
import type { PluralForm } from "./plural";
import type { ru } from "./ru";

export { LOCALES, LOCALE_CODES, type Locale } from "./locales";

/** All keys. The Russian dictionary is the source: English must cover it. */
export type Key = keyof typeof ru;

/**
 * Bases of plural groups: `bar.summary.one` gives `bar.summary`.
 *
 * Through a type parameter, not directly from `Key`: a conditional type distributes
 * over a union only when the parameter sits on the left. Written head-on it would
 * give `never`, and plural forms would silently stop being checked.
 */
type BaseOf<K> = K extends `${infer B}.one` ? B : never;
export type PluralBase = BaseOf<Key>;

export type Params = Record<string, string | number>;

/**
 * The order in which a form is looked up. English asks for `other`, which is not in
 * the dictionary: the same string lies under `many` there.
 */
const FALLBACK: Record<PluralForm, PluralForm[]> = {
  one: ["one", "many", "few", "other"],
  few: ["few", "many", "other", "one"],
  many: ["many", "other", "few", "one"],
  other: ["other", "many", "few", "one"],
};

/**
 * Window language (§14).
 *
 * A live binding, not a constant: the settings window §10 lets the user pick a
 * language, and the chosen one is set by [`setLocale`] from the entry point —
 * before the first render, while no one has read the dictionary yet. It cannot
 * be changed after rendering: half the labels are already drawn, and the other
 * half would come out in another language.
 */
export let locale: Locale = detect();

/**
 * Sets the window language. `null` means «as the system», i.e. the same question
 * to the webview that was asked before the setting existed.
 *
 * Called from exactly one place — `bootPrefs` in `src/ui/prefs.ts` — and before
 * `createRoot`. It also sets `lang` on the markup: a screen reader must know which
 * language it is reading (§14).
 */
export function setLocale(next: Locale | null): void {
  locale = next ?? detect();
  // Only when it changes. The preferences event arrives for every setting —
  // the theme, the accent — and writing the same `lang` again is still a
  // change to the document the webview hears: with a field focused it re-reads
  // the input language, and the next key typed may be lost.
  if (typeof document !== "undefined" && document.documentElement.lang !== locale) {
    document.documentElement.lang = locale;
  }
}

function detect(): Locale {
  const tags =
    typeof navigator === "undefined" ? [] : (navigator.languages ?? [navigator.language]);
  for (const tag of tags) {
    if (typeof tag !== "string") continue;
    const base = tag.toLowerCase().split("-")[0];
    if (isLocale(base)) return base;
  }
  return "ru";
}

/** String by key. A missing key is a compile error, not an empty screen. */
export function t(key: Key, params?: Params): string {
  return translate(locale, key, params);
}

/** Plural string: `tp("bar.summary", 3)` → «3 файла». */
export function tp(base: PluralBase, n: number, params?: Params): string {
  return translatePlural(locale, base, n, params);
}

/** Same, but with an explicit locale — the tests use this. */
export function translate(target: Locale, key: string, params?: Params): string {
  const text = LOCALES[target].strings[key] ?? LOCALES.ru.strings[key];
  // A key that is in no dictionary shows as the key. An empty string in its place
  // would be a label that vanished silently.
  return text === undefined ? key : fill(text, params);
}

export function translatePlural(
  target: Locale,
  base: string,
  n: number,
  params?: Params,
): string {
  const wanted = LOCALES[target].plural(n);
  for (const form of FALLBACK[wanted]) {
    const text = LOCALES[target].strings[`${base}.${form}`];
    if (text !== undefined) return fill(text, { n, ...params });
  }
  return `${base}.${wanted}`;
}

/** Substitutes `{name}`. An unfilled slot stays visible for the same reason. */
function fill(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = params[name];
    return value === undefined ? whole : String(value);
  });
}

/** Uppercases the first letter by the language's rules (§5.4: the summary header starts with it). */
export function capitalize(text: string): string {
  if (!text) return text;
  return text[0].toLocaleUpperCase(locale) + text.slice(1);
}
