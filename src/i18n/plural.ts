/**
 * Склонение числительных (§14).
 *
 * Своё, а не `Intl.PluralRules`: правила совпадают, но `Intl` возвращает
 * категорию по строке локали, а нам нужна ровно та локаль, которую выбрал
 * человек, и ровно те три формы, которые лежат в словаре. Плюс это чистая
 * функция — её видно в тесте целиком.
 */

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

/**
 * French and Brazilian Portuguese: zero is singular — "0 fichier", "0 arquivo".
 *
 * The one place the two-form languages differ from English, and it is on the
 * number an empty list shows.
 */
export function zeroOneForm(n: number): PluralForm {
  const abs = Math.abs(n);
  return abs === 0 || abs === 1 ? "one" : "other";
}

/** Chinese, Japanese, Korean: the noun does not change with the count. */
export function singleForm(): PluralForm {
  return "other";
}
