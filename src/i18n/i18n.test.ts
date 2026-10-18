/**
 * Словари и склонение (§14).
 *
 * Главная проверка — покрытие: английский обязан знать каждый ключ русского.
 * Компилятор ловит отсутствующий ключ, но не ловит скопированную русскую
 * строку, оставленную в английском словаре, — это ловится здесь.
 */

import { describe, expect, it } from "vitest";

import { sections as enSections } from "./en";
import { LOCALES } from "./locales";
import { duplicates } from "./merge";
import { ru, sections as ruSections } from "./ru";
import { ruForm, enForm, singleForm, zeroOneForm } from "./plural";
import { translate, translatePlural } from "./index";

/** Каждый словарь, кроме русского: он источник, с ним и сверяют. */
const translated = Object.entries(LOCALES).filter(([code]) => code !== "ru");

describe("словари", () => {
  it("покрывают друг друга ключ в ключ", () => {
    for (const [code, info] of translated) {
      expect(Object.keys(info.strings).sort(), code).toEqual(Object.keys(ru).sort());
    }
  });

  it("не объявляют ключ в двух разделах", () => {
    // Склейка разделов отдала бы победу последнему и промолчала.
    expect(duplicates(ruSections)).toEqual([]);
    expect(duplicates(enSections)).toEqual([]);
  });

  it("называют каждый язык", () => {
    // Пустое имя — пустая строка в списке языков §10.
    for (const [code, info] of Object.entries(LOCALES)) {
      expect(info.name.trim(), code).not.toBe("");
    }
  });

  it("не оставляют кириллицу в переводах", () => {
    for (const [code, info] of translated) {
      const russian = Object.entries(info.strings)
        .filter(([, value]) => /[А-Яа-яЁё]/.test(value))
        .map(([key]) => key);
      expect(russian, code).toEqual([]);
    }
  });

  it("не оставляют пустых строк", () => {
    // Пустой перевод проходит проверку типов и молча стирает надпись.
    for (const [code, info] of translated) {
      const empty = Object.entries(info.strings)
        .filter(([key, value]) => value.trim() === "" && ru[key as keyof typeof ru].trim() !== "")
        .map(([key]) => key);
      expect(empty, code).toEqual([]);
    }
  });

  it("держат одни и те же места подстановки", () => {
    const holes = (text: string) =>
      (text.match(/\{(\w+)\}/g) ?? []).sort().join(",");

    for (const [code, info] of translated) {
      for (const key of Object.keys(ru) as (keyof typeof ru)[]) {
        expect(holes(info.strings[key] ?? ""), `${code}: ${key}`).toBe(holes(ru[key]));
      }
    }
  });

  it("держат группы склонения целиком", () => {
    // Форма, которой нет, — это «3 файл» на экране.
    for (const key of Object.keys(ru)) {
      if (!key.endsWith(".one")) continue;
      const base = key.slice(0, -".one".length);
      expect(Object.keys(ru), base).toEqual(
        expect.arrayContaining([`${base}.few`, `${base}.many`]),
      );
    }
  });
});

describe("склонение", () => {
  it("русское считает по двум последним цифрам", () => {
    expect(ruForm(1)).toBe("one");
    expect(ruForm(2)).toBe("few");
    expect(ruForm(5)).toBe("many");
    // Одиннадцать — не «один»: на этом ломается всякая простая реализация.
    expect(ruForm(11)).toBe("many");
    expect(ruForm(14)).toBe("many");
    expect(ruForm(21)).toBe("one");
    expect(ruForm(22)).toBe("few");
    expect(ruForm(0)).toBe("many");
  });

  it("английское знает две формы", () => {
    expect(enForm(1)).toBe("one");
    expect(enForm(2)).toBe("other");
    expect(enForm(0)).toBe("other");
  });

  it("французское и бразильское считают ноль единственным числом", () => {
    expect(zeroOneForm(0)).toBe("one");
    expect(zeroOneForm(1)).toBe("one");
    expect(zeroOneForm(2)).toBe("other");
  });

  it("китайское, японское и корейское не склоняют", () => {
    expect(singleForm()).toBe("other");
  });

  it("находит форму в каждом языке", () => {
    // Вернувшийся ключ — это `row.files.other` на экране вместо числа файлов.
    for (const [code] of Object.entries(LOCALES)) {
      for (const n of [0, 1, 2, 5, 11, 21]) {
        const text = translatePlural(code as keyof typeof LOCALES, "row.files", n);
        expect(text, `${code}: ${n}`).toContain(String(n));
        expect(text, `${code}: ${n}`).not.toContain("row.files");
      }
    }
  });

  it("подставляет число в выбранную форму", () => {
    expect(translatePlural("ru", "row.files", 1)).toBe("1 файл");
    expect(translatePlural("ru", "row.files", 3)).toBe("3 файла");
    expect(translatePlural("ru", "row.files", 11)).toBe("11 файлов");
    // Английскому нужна форма `other`, которой в словаре нет: он падает
    // в `many`, где лежит та же строка.
    expect(translatePlural("en", "row.files", 3)).toBe("3 files");
    expect(translatePlural("en", "row.files", 1)).toBe("1 file");
  });
});

describe("подстановка", () => {
  it("заполняет именованные места", () => {
    expect(translate("ru", "act.created", { name: "a.rs" })).toBe("Создан a.rs");
    expect(translate("en", "act.created", { name: "a.rs" })).toBe("Created a.rs");
  });

  it("оставляет незаполненное место видимым", () => {
    // Пустота на месте имени файла — это надпись, потерявшаяся молча.
    expect(translate("ru", "act.created")).toBe("Создан {name}");
  });

  it("показывает ключ, которого нет", () => {
    expect(translate("ru", "нет.такого")).toBe("нет.такого");
  });
});
