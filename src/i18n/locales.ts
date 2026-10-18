/**
 * The languages the window speaks (§14).
 *
 * The one place a language is registered. Adding one is a folder of sections
 * next to `ru/` and `en/`, an entry here, and a variant of `Language` in
 * `src-tauri/src/prefs.rs` — the last is what the compiler asks for first:
 * `Locale` is derived from that enum, and `LOCALES` has to cover it.
 */

import type { Language } from "../core/client";
import { de } from "./de";
import { en } from "./en";
import { es } from "./es";
import { fr } from "./fr";
import { ja } from "./ja";
import { ko } from "./ko";
import type { Section } from "./merge";
import { enForm, ruForm, singleForm, zeroOneForm, type PluralForm } from "./plural";
import { pt } from "./pt";
import { ru } from "./ru";
import { zh } from "./zh";

/** A language with a dictionary: every choice of the setting except "as the system". */
export type Locale = Exclude<Language, "system">;

export type LocaleInfo = {
  /**
   * The language named in itself — «Русский», not "Russian". Not a dictionary
   * key: it reads the same in every language, and it is the only way to find
   * one's own language in an interface one cannot read.
   */
  name: string;
  plural: (n: number) => PluralForm;
  strings: Section;
};

export const LOCALES: Record<Locale, LocaleInfo> = {
  ru: { name: "Русский", plural: ruForm, strings: ru },
  en: { name: "English", plural: enForm, strings: en },
  // Simplified. A primary subtag is all `detect` looks at, so `zh-TW` lands
  // here too: Simplified is closer to that reader than Russian is.
  zh: { name: "简体中文", plural: singleForm, strings: zh },
  es: { name: "Español", plural: enForm, strings: es },
  ja: { name: "日本語", plural: singleForm, strings: ja },
  ko: { name: "한국어", plural: singleForm, strings: ko },
  de: { name: "Deutsch", plural: enForm, strings: de },
  fr: { name: "Français", plural: zeroOneForm, strings: fr },
  // Brazilian; `pt-PT` lands here for the same reason `zh-TW` lands on `zh`.
  pt: { name: "Português (Brasil)", plural: zeroOneForm, strings: pt },
};

/** In the order the settings window lists them. */
export const LOCALE_CODES = Object.keys(LOCALES) as Locale[];

/** Whether a language tag's primary subtag is one of ours. */
export function isLocale(code: string): code is Locale {
  return (LOCALE_CODES as string[]).includes(code);
}
