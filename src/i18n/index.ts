/**
 * Перевод интерфейса (§14).
 *
 * Ни одной строки текста в компонентах — только ключи. Ключи типизированы по
 * русскому словарю, поэтому опечатка и забытый перевод ловятся компилятором,
 * а не человеком, который однажды увидит английское слово посреди ленты. Ровно
 * то же правило, по которому типы ядра генерируются из Rust.
 *
 * Своё вместо `i18next`: нужны ключи, подстановка и склонение (§14), а
 * namespace, backend и suspense — нет. Сто строк против дерева зависимостей.
 */

import { isLocale, LOCALES, type Locale } from "./locales";
import type { PluralForm } from "./plural";
import type { ru } from "./ru";

export { LOCALES, LOCALE_CODES, type Locale } from "./locales";

/** Все ключи. Русский словарь — источник: английский обязан его покрыть. */
export type Key = keyof typeof ru;

/**
 * Основы групп склонения: `bar.summary.one` даёт `bar.summary`.
 *
 * Через параметр типа, а не напрямую от `Key`: условный тип распределяется по
 * объединению только когда слева стоит параметр. Написанное в лоб дало бы
 * `never`, и склонения молча перестали бы проверяться.
 */
type BaseOf<K> = K extends `${infer B}.one` ? B : never;
export type PluralBase = BaseOf<Key>;

export type Params = Record<string, string | number>;

/**
 * Порядок, в котором ищется форма. Английский просит `other`, которого в
 * словаре нет: там та же строка лежит под `many`.
 */
const FALLBACK: Record<PluralForm, PluralForm[]> = {
  one: ["one", "many", "few", "other"],
  few: ["few", "many", "other", "one"],
  many: ["many", "other", "few", "one"],
  other: ["other", "many", "few", "one"],
};

/**
 * Язык окна (§14).
 *
 * Живая привязка, а не константа: окно настроек §10 даёт выбрать язык, и
 * выбранный ставится [`setLocale`] из точки входа — до первого рендера, пока
 * словарь ещё никто не прочитал. Менять его после рендера нельзя: половина
 * надписей уже нарисована, и вторая половина приехала бы на другом языке.
 */
export let locale: Locale = detect();

/**
 * Ставит язык окна. `null` — «как в системе», то есть тот же вопрос к вебвью,
 * который задавался до появления настройки.
 *
 * Зовётся ровно из одного места — `bootPrefs` в `src/ui/prefs.ts`, — и до
 * `createRoot`. Заодно проставляет `lang` на разметке: скринридер должен
 * знать, на каком языке читает (§14).
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

/** Строка по ключу. Отсутствие ключа — ошибка компиляции, не пустой экран. */
export function t(key: Key, params?: Params): string {
  return translate(locale, key, params);
}

/** Строка со склонением: `tp("bar.summary", 3)` → «3 файла». */
export function tp(base: PluralBase, n: number, params?: Params): string {
  return translatePlural(locale, base, n, params);
}

/** То же, но с явной локалью — этим пользуются тесты. */
export function translate(target: Locale, key: string, params?: Params): string {
  const text = LOCALES[target].strings[key] ?? LOCALES.ru.strings[key];
  // Ключ, которого нет ни в одном словаре, виден как ключ. Пустая строка на
  // его месте — это молча пропавшая надпись.
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

/** Подстановка `{name}`. Незаполненное место остаётся видимым по той же причине. */
function fill(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = params[name];
    return value === undefined ? whole : String(value);
  });
}

/** Первая буква прописной, с учётом языка (§5.4: шапка-итог начинается с неё). */
export function capitalize(text: string): string {
  if (!text) return text;
  return text[0].toLocaleUpperCase(locale) + text.slice(1);
}
