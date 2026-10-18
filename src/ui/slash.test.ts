/**
 * Меню слэш-команд §7.3.
 *
 * Проверяется то, что решает поведение поля: когда меню есть, что в нём стоит
 * и что из набранного интерфейс выполняет сам, а что уезжает модели.
 */

import { describe, expect, it } from "vitest";

import type { SkillInfo } from "../core/client";
import { builtinFor, menuFor, mcpPrompts, withSkills, type Builtin, type MenuCommand } from "./slash";

const BUILTINS: Builtin[] = ["model", "compact", "clear"];

const project: MenuCommand[] = [
  { name: "review", description: "Просмотреть изменения", body: "...", kind: "command" },
  { name: "compact", description: "Файл, назвавшийся встроенной", body: "...", kind: "command" },
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

describe("skills in the menu", () => {
  const skill = (name: string, userInvocable = true): SkillInfo => ({
    name,
    description: `skill ${name}`,
    path: `/skills/${name}/SKILL.md`,
    scope: "project",
    modelInvocable: true,
    userInvocable,
  });

  it("join the commands in name order", () => {
    const merged = withSkills(project, [skill("audit"), skill("zeta")]);
    expect(merged.map((c) => c.name)).toEqual(["audit", "compact", "review", "zeta"]);
  });

  it("give way to a command of the same name, as the core does", () => {
    const merged = withSkills(project, [skill("review")]);
    expect(merged.filter((c) => c.name === "review").map((c) => c.kind)).toEqual(["command"]);
  });

  it("say where each command comes from", () => {
    const plugin = { name: "tools:lint", description: "", body: "" };
    const merged = withSkills([plugin, project[0]], [skill("audit")]);
    expect(merged.map((c) => [c.name, c.kind, c.scope])).toEqual([
      ["audit", "skill", "project"],
      ["review", "command", undefined],
      ["tools:lint", "pluginCommand", undefined],
    ]);
  });

  it("leave out the ones kept from the menu", () => {
    expect(withSkills([], [skill("hidden", false)])).toEqual([]);
  });
});

describe("mcpPrompts", () => {
  it("offers the prompts of ready servers, arguments named in order", () => {
    const server = (status: "ready" | "failed") => ({
      name: "docs",
      source: "global" as const,
      transport: "http" as const,
      target: "https://x",
      status,
      reason: null,
      label: null,
      tools: [],
      resources: false,
      loginUrl: null,
      loggedIn: false,
      prompts: [{ command: "mcp__docs__explain", description: "Explain a page.", arguments: ["page", "audience"] }],
    });
    const offered = mcpPrompts([server("ready")]);
    expect(offered).toHaveLength(1);
    expect(offered[0].name).toBe("mcp__docs__explain");
    expect(offered[0].kind).toBe("mcpPrompt");
    expect(offered[0].description).toBe("<page> <audience> — Explain a page.");
    expect(mcpPrompts([server("failed")])).toEqual([]);
    expect(mcpPrompts(null)).toEqual([]);
  });
});
