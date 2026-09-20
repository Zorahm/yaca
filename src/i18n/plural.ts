/**
 * Склонение числительных (§14).
 *
 * Своё, а не `Intl.PluralRules`: правила совпадают, но `Intl` возвращает
 * категорию по строке локали, а нам нужна ровно та локаль, которую выбрал
 * человек, и ровно те три формы, которые лежат в словаре. Плюс это чистая
 * функция — её видно в тесте целиком.
 */

export type Locale = "ru" | "en";

/** Формы, которые может потребовать словарь. */
export type PluralForm = "one" | "few" | "many" | "other";

/**
 * Русский: 1 файл, 2 файла, 5 файлов, 11 файлов, 21 файл.
 *
 * Одиннадцать — не «один»: вторая цифра важнее последней, и это тот случай,
 * на котором ломается всякая «простая» реализация.
 */
export function ruForm(n: number): PluralForm {
  const abs = Math.abs(Math.trunc(n));
  const tens = abs % 100;
  const ones = abs % 10;

  if (tens >= 11 && tens <= 14) return "many";
  if (ones === 1) return "one";
  if (ones >= 2 && ones <= 4) return "few";
  return "many";
}

export function enForm(n: number): PluralForm {
  return Math.abs(n) === 1 ? "one" : "other";
}

export function formOf(locale: Locale, n: number): PluralForm {
  return locale === "ru" ? ruForm(n) : enForm(n);
}
