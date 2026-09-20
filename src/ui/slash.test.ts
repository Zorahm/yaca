/**
 * Меню слэш-команд §7.3.
 *
 * Проверяется то, что решает поведение поля: когда меню есть, что в нём стоит
 * и что из набранного интерфейс выполняет сам, а что уезжает модели.
 */

import { describe, expect, it } from "vitest";

import type { SlashCommand } from "../core/client";
import { builtinFor, menuFor, type Builtin } from "./slash";

const BUILTINS: Builtin[] = ["model", "compact", "clear"];

const project: SlashCommand[] = [
  { name: "review", description: "Просмотреть изменения", body: "..." },
  { name: "compact", description: "Файл, назвавшийся встроенной", body: "..." },
];

const names = (text: string, builtins = BUILTINS) =>
  menuFor(text, project, builtins)?.map((item) => item.name) ?? null;

describe("меню команд", () => {
  it("не показывается для обычного сообщения", () => {
    expect(names("почини разбор дат")).toBeNull();
    expect(names(" /review")).toBeNull();
  });

  it("показывает всё по одному слэшу", () => {
    expect(names("/")).toEqual(["clear", "compact", "model", "review"]);
  });

  it("сужается по набранному", () => {
    expect(names("/c")).toEqual(["clear", "compact"]);
    expect(names("/rev")).toEqual(["review"]);
  });

  it("исчезает, когда дополнять нечего", () => {
    expect(names("/zzz")).toBeNull();
  });

  it("закрывается пробелом: дальше идут аргументы", () => {
    expect(names("/review src/main.rs")).toBeNull();
    expect(names("/review ")).toBeNull();
  });

  it("не показывает встроенных, за которыми на этом экране нет действия", () => {
    // Домашний экран §9.1: сжимать и начинать заново нечего. Занятое имя
    // остаётся занятым и здесь — иначе `/compact` значил бы на двух экранах
    // разное.
    expect(names("/", ["model"])).toEqual(["model", "review"]);
  });

  it("не сдваивает имя, занятое встроенной командой", () => {
    // `/compact`, делающий что-то кроме сжатия, хуже, чем его отсутствие.
    const items = menuFor("/compact", project, BUILTINS);
    expect(items?.length).toBe(1);
    expect(items?.[0].builtin).toBe("compact");
  });
});

describe("отправка", () => {
  it("узнаёт встроенную команду", () => {
    expect(builtinFor("/compact", BUILTINS)).toBe("compact");
    expect(builtinFor("/compact ", BUILTINS)).toBe("compact");
  });

  it("не считает командой проектное имя: его разворачивает ядро", () => {
    expect(builtinFor("/review", BUILTINS)).toBeNull();
  });

  it("не считает командой строку с аргументами: встроенные их не берут", () => {
    expect(builtinFor("/compact хвост", BUILTINS)).toBeNull();
    expect(builtinFor("обычное сообщение", BUILTINS)).toBeNull();
  });
});
