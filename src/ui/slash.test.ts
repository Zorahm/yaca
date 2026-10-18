/**
 * The slash-command menu §7.3.
 *
 * Checks what decides the field's behaviour: when the menu is there, what it
 * lists, and which of the typed text the interface runs itself and which goes
 * to the model.
 */

import { describe, expect, it } from "vitest";

import type { SkillInfo } from "../core/client";
import { builtinFor, menuFor, mcpPrompts, withSkills, type Builtin, type MenuCommand } from "./slash";

const BUILTINS: Builtin[] = ["model", "compact", "clear"];

const project: MenuCommand[] = [
  { name: "review", description: "Review the changes", body: "...", kind: "command" },
  { name: "compact", description: "A file named like a built-in", body: "...", kind: "command" },
];

const names = (text: string, builtins = BUILTINS) =>
  menuFor(text, project, builtins)?.map((item) => item.name) ?? null;

describe("command menu", () => {
  it("is not shown for an ordinary message", () => {
    expect(names("fix the date parsing")).toBeNull();
    expect(names(" /review")).toBeNull();
  });

  it("shows everything for a bare slash", () => {
    expect(names("/")).toEqual(["clear", "compact", "model", "review"]);
  });

  it("narrows to what is typed", () => {
    expect(names("/c")).toEqual(["clear", "compact"]);
    expect(names("/rev")).toEqual(["review"]);
  });

  it("disappears when there is nothing to complete", () => {
    expect(names("/zzz")).toBeNull();
  });

  it("closes at a space: arguments follow", () => {
    expect(names("/review src/main.rs")).toBeNull();
    expect(names("/review ")).toBeNull();
  });

  it("does not show built-ins that have no action on this screen", () => {
    // The home screen §9.1 has nothing to compact or start over, but the taken
    // name stays taken here too, otherwise `/compact` would mean different
    // things on two screens.
    expect(names("/", ["model"])).toEqual(["model", "review"]);
  });

  it("does not duplicate a name taken by a built-in command", () => {
    // `/compact` doing something other than compacting is worse than its absence.
    const items = menuFor("/compact", project, BUILTINS);
    expect(items?.length).toBe(1);
    expect(items?.[0].builtin).toBe("compact");
  });
});

describe("sending", () => {
  it("recognises a built-in command", () => {
    expect(builtinFor("/compact", BUILTINS)).toBe("compact");
    expect(builtinFor("/compact ", BUILTINS)).toBe("compact");
  });

  it("does not treat a project name as a command: the core expands it", () => {
    expect(builtinFor("/review", BUILTINS)).toBeNull();
  });

  it("does not treat a line with arguments as a command: built-ins take none", () => {
    expect(builtinFor("/compact tail", BUILTINS)).toBeNull();
    expect(builtinFor("an ordinary message", BUILTINS)).toBeNull();
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
