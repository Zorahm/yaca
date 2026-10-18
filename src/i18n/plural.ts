/**
 * Plural forms of numerals (§14).
 *
 * Our own, not `Intl.PluralRules`: the rules match, but `Intl` returns the category
 * for a locale string, and we need exactly the locale the user picked and exactly
 * the three forms the dictionary has. Plus this is a pure function — it can be
 * seen whole in a test.
 */

/** Forms a dictionary may require. */
export type PluralForm = "one" | "few" | "many" | "other";

/**
 * Russian: 1 файл, 2 файла, 5 файлов, 11 файлов, 21 файл.
 *
 * Eleven is not «один» (one): the second digit matters more than the last one, and
 * this is the case that breaks every "simple" implementation.
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
