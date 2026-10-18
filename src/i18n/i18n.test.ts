/**
 * Dictionaries and plural forms (§14).
 *
 * The main check is coverage: English must know every key Russian has.
 * The compiler catches a missing key, but not a Russian string copied and left
 * in the English dictionary — this test catches that.
 */

import { describe, expect, it } from "vitest";

import { sections as enSections } from "./en";
import { LOCALES } from "./locales";
import { duplicates } from "./merge";
import { ru, sections as ruSections } from "./ru";
import { ruForm, enForm, singleForm, zeroOneForm } from "./plural";
import { translate, translatePlural } from "./index";

/** Every dictionary except Russian: it is the source, and the others are checked against it. */
const translated = Object.entries(LOCALES).filter(([code]) => code !== "ru");

describe("dictionaries", () => {
  it("cover each other key for key", () => {
    for (const [code, info] of translated) {
      expect(Object.keys(info.strings).sort(), code).toEqual(Object.keys(ru).sort());
    }
  });

  it("do not declare a key in two sections", () => {
    // Merging sections would let the last one win and say nothing.
    expect(duplicates(ruSections)).toEqual([]);
    expect(duplicates(enSections)).toEqual([]);
  });

  it("name every language", () => {
    // An empty name is an empty line in the language list (§10).
    for (const [code, info] of Object.entries(LOCALES)) {
      expect(info.name.trim(), code).not.toBe("");
    }
  });

  it("leave no Cyrillic in translations", () => {
    for (const [code, info] of translated) {
      const russian = Object.entries(info.strings)
        .filter(([, value]) => /[А-Яа-яЁё]/.test(value))
        .map(([key]) => key);
      expect(russian, code).toEqual([]);
    }
  });

  it("leave no empty strings", () => {
    // An empty translation passes the type check and silently erases the label.
    for (const [code, info] of translated) {
      const empty = Object.entries(info.strings)
        .filter(([key, value]) => value.trim() === "" && ru[key as keyof typeof ru].trim() !== "")
        .map(([key]) => key);
      expect(empty, code).toEqual([]);
    }
  });

  it("keep the same substitution slots", () => {
    const holes = (text: string) =>
      (text.match(/\{(\w+)\}/g) ?? []).sort().join(",");

    for (const [code, info] of translated) {
      for (const key of Object.keys(ru) as (keyof typeof ru)[]) {
        expect(holes(info.strings[key] ?? ""), `${code}: ${key}`).toBe(holes(ru[key]));
      }
    }
  });

  it("keep plural groups complete", () => {
    // A missing form shows up on screen as «3 файл».
    for (const key of Object.keys(ru)) {
      if (!key.endsWith(".one")) continue;
      const base = key.slice(0, -".one".length);
      expect(Object.keys(ru), base).toEqual(
        expect.arrayContaining([`${base}.few`, `${base}.many`]),
      );
    }
  });
});

describe("plural forms", () => {
  it("counts Russian by the last two digits", () => {
    expect(ruForm(1)).toBe("one");
    expect(ruForm(2)).toBe("few");
    expect(ruForm(5)).toBe("many");
    // Eleven is not «один» (one): every naive implementation breaks on this.
    expect(ruForm(11)).toBe("many");
    expect(ruForm(14)).toBe("many");
    expect(ruForm(21)).toBe("one");
    expect(ruForm(22)).toBe("few");
    expect(ruForm(0)).toBe("many");
  });

  it("English knows two forms", () => {
    expect(enForm(1)).toBe("one");
    expect(enForm(2)).toBe("other");
    expect(enForm(0)).toBe("other");
  });

  it("French and Brazilian count zero as singular", () => {
    expect(zeroOneForm(0)).toBe("one");
    expect(zeroOneForm(1)).toBe("one");
    expect(zeroOneForm(2)).toBe("other");
  });

  it("Chinese, Japanese and Korean do not inflect", () => {
    expect(singleForm()).toBe("other");
  });

  it("finds a form in every language", () => {
    // A returned key means `row.files.other` on screen instead of the file count.
    for (const [code] of Object.entries(LOCALES)) {
      for (const n of [0, 1, 2, 5, 11, 21]) {
        const text = translatePlural(code as keyof typeof LOCALES, "row.files", n);
        expect(text, `${code}: ${n}`).toContain(String(n));
        expect(text, `${code}: ${n}`).not.toContain("row.files");
      }
    }
  });

  it("substitutes the number into the chosen form", () => {
    expect(translatePlural("ru", "row.files", 1)).toBe("1 файл");
    expect(translatePlural("ru", "row.files", 3)).toBe("3 файла");
    expect(translatePlural("ru", "row.files", 11)).toBe("11 файлов");
    // English needs the `other` form, which the dictionary does not have: it falls
    // back to `many`, where the same string lies.
    expect(translatePlural("en", "row.files", 3)).toBe("3 files");
    expect(translatePlural("en", "row.files", 1)).toBe("1 file");
  });
});

describe("substitution", () => {
  it("fills named slots", () => {
    expect(translate("ru", "act.created", { name: "a.rs" })).toBe("Создан a.rs");
    expect(translate("en", "act.created", { name: "a.rs" })).toBe("Created a.rs");
  });

  it("leaves an unfilled slot visible", () => {
    // An empty gap where the file name goes is a label lost silently.
    expect(translate("ru", "act.created")).toBe("Создан {name}");
  });

  it("shows a key that does not exist", () => {
    expect(translate("ru", "no.such.key")).toBe("no.such.key");
  });
});
