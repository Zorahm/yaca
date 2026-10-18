/**
 * Composer slash commands §7.3.
 *
 * Two kinds in one menu. Built-ins are what the interface does itself: a
 * `Command` goes to the core, not text. Project commands are files in
 * `.yaca/commands/*.md`, and the core expands them at send time; the window
 * needs only their name and description to draw a row.
 *
 * Five of the eight built-ins listed in §7.3 are here. `/init`, `/cost` and
 * `/help` lead to the project file, the cost breakdown and help, which are
 * screens that do not exist yet, and a command that opens nothing is no better
 * than a button that opens nothing (§0). They arrive together with their
 * subsystems.
 *
 * The menu is a function of the input's text, not a separate state: it closes
 * when the text stops being a command. A "menu open" flag would be a second
 * version of the same truth, and it would diverge from it on the first
 * `Backspace`.
 */

import type { McpServerSummary, SkillInfo, SkillScope, SlashCommand } from "../core/client";
import { t, type Key } from "../i18n";

/** A built-in command: something the interface can do by itself. */
export type Builtin = "checkpoints" | "clear" | "compact" | "lsp" | "mcp" | "model";

/**
 * All names taken by built-in commands.
 *
 * Wider than the list available on screen: the home screen §9.1 has nothing to
 * compact, but `/compact` there must not suddenly mean someone else's file. A
 * command's name means the same thing everywhere.
 */
export const RESERVED: Builtin[] = ["checkpoints", "clear", "compact", "lsp", "mcp", "model"];

const NOTE: Record<Builtin, Key> = {
  checkpoints: "slash.checkpoints",
  clear: "slash.clear",
  compact: "slash.compact",
  lsp: "slash.lsp",
  mcp: "slash.mcp",
  model: "slash.model",
};

/** Where a command the core expands comes from, for the pane that explains it. */
export type CommandKind = "command" | "pluginCommand" | "skill" | "mcpPrompt";

/** A command of the menu that is not built in: a file, a plugin's, a skill. */
export type MenuCommand = SlashCommand & { kind: CommandKind; scope?: SkillScope };

/** A menu row. The name has no slash, the same as for a file. */
export type SlashItem = {
  name: string;
  description: string;
  /** `null` for a project command: the core expands it. */
  builtin: Builtin | null;
  kind: CommandKind | "builtin";
  scope?: SkillScope;
};

/**
 * What the menu shows for the typed text, or `null` when no menu is needed.
 *
 * A space closes the menu: arguments follow, and there is nothing left to complete.
 */
export function menuFor(
  text: string,
  project: MenuCommand[],
  builtins: Builtin[],
): SlashItem[] | null {
  if (!text.startsWith("/")) return null;
  const typed = text.slice(1);
  if (/\s/.test(typed)) return null;

  const prefix = typed.toLowerCase();
  const items = all(project, builtins).filter((item) =>
    item.name.toLowerCase().startsWith(prefix),
  );
  return items.length > 0 ? items : null;
}

/**
 * The built-in command the text equals, or `null`.
 *
 * Built-ins take no arguments, so `/compact tail` is not a command but a
 * message, and it goes to the model as it is: the core treats a `/name` that no
 * file answers to in exactly the same way.
 */
export function builtinFor(text: string, builtins: Builtin[]): Builtin | null {
  const name = text.trim().slice(1);
  if (!text.trim().startsWith("/")) return null;
  return builtins.includes(name as Builtin) ? (name as Builtin) : null;
}

/**
 * Built-ins first, then project commands; each group in alphabetical order.
 *
 * A file named like a built-in command drops out of the menu, and on the screen
 * where that built-in is absent too: a `/compact` that means one thing on the
 * home screen and another in a conversation is worse than no `/compact` at all.
 */
function all(project: MenuCommand[], builtins: Builtin[]): SlashItem[] {
  const taken = new Set<string>(RESERVED);
  return [
    ...[...builtins].sort().map((name) => ({
      name,
      description: t(NOTE[name]),
      builtin: name,
      kind: "builtin" as const,
    })),
    ...project
      .filter((command) => !taken.has(command.name))
      .map((command) => ({
        name: command.name,
        description: command.description,
        builtin: null,
        kind: command.kind,
        scope: command.scope,
      })),
  ];
}

/**
 * The project's commands with the skills a human may call by name, as one
 * group in name order. The core looks a typed `/name` up among the commands
 * first, so a skill named like a command is left out here as well: the menu
 * must offer what the core will actually run.
 */
export function withSkills(commands: SlashCommand[], skills: SkillInfo[]): MenuCommand[] {
  const taken = new Set(commands.map((command) => command.name));
  // A plugin's command carries its plugin's name before a colon; a file of
  // `.yaca/commands` cannot.
  const files: MenuCommand[] = commands.map((command) => ({
    ...command,
    kind: command.name.includes(":") ? "pluginCommand" : "command",
  }));
  const callable: MenuCommand[] = skills
    .filter((skill) => skill.userInvocable && !taken.has(skill.name))
    .map((skill) => ({
      name: skill.name,
      description: skill.description,
      body: "",
      kind: "skill",
      scope: skill.scope,
    }));
  return [...files, ...callable].sort((a, b) => a.name.localeCompare(b.name));
}

/**
 * The prompts of this chat's ready MCP servers, as commands: the core fills
 * `/mcp__server__prompt words…` by asking the server. The arguments are named
 * in the description, in the order typed words fill them.
 */
export function mcpPrompts(servers: McpServerSummary[] | null): MenuCommand[] {
  return (servers ?? [])
    .filter((server) => server.status === "ready")
    .flatMap((server) =>
      server.prompts.map((prompt) => {
        const args = prompt.arguments.map((name) => `<${name}>`).join(" ");
        const about = prompt.description ?? "";
        return {
          name: prompt.command,
          description: args === "" ? about : `${args}${about === "" ? "" : ` — ${about}`}`,
          body: "",
          kind: "mcpPrompt" as const,
        };
      }),
    );
}
