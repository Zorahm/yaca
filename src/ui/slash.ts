/**
 * Слэш-команды композера §7.3.
 *
 * Две породы в одном меню. Встроенные — то, что делает сам интерфейс: в ядро
 * уезжает `Command`, а не текст. Проектные лежат файлами в
 * `.yaca/commands/*.md`, и разворачивает их ядро в момент отправки — окну от
 * них нужны имя и описание, чтобы нарисовать строку.
 *
 * Встроенных здесь пять из восьми, перечисленных в §7.3. `/init`, `/cost` и
 * `/help` ведут в файл проекта, разбор трат и справку — экраны, которых нет,
 * а команда, которая ничего не открывает, ничем не лучше кнопки, которая
 * ничего не открывает (§0). Приедут вместе со своими подсистемами.
 *
 * Меню — функция от текста поля, а не отдельное состояние: закрывается оно
 * тем, что текст перестал быть командой. Флаг «меню открыто» был бы второй
 * версией той же правды и разошёлся бы с ней на первом же `Backspace`.
 */

import type { McpServerSummary, SkillInfo, SkillScope, SlashCommand } from "../core/client";
import { t, type Key } from "../i18n";

/** Встроенная команда — то, что интерфейс умеет сделать сам. */
export type Builtin = "checkpoints" | "clear" | "compact" | "lsp" | "mcp" | "model";

/**
 * Все имена, занятые встроенными командами.
 *
 * Шире, чем список доступных на экране: на домашнем экране §9.1 сжимать
 * нечего, но `/compact` там не должен вдруг означать чужой файл. Имя команды
 * значит одно и то же везде.
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

/** Строка меню. Имя — без слэша, как и у файла. */
export type SlashItem = {
  name: string;
  description: string;
  /** `null` у проектной команды: её разворачивает ядро. */
  builtin: Builtin | null;
  kind: CommandKind | "builtin";
  scope?: SkillScope;
};

/**
 * Что показывает меню для набранного текста, или `null`, когда меню не нужно.
 *
 * Пробел закрывает меню: дальше идут аргументы, и дополнять уже нечего.
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
 * Встроенная команда, которой равна строка, или `null`.
 *
 * Аргументов встроенные не берут, поэтому `/compact хвост` — не команда, а
 * сообщение, и уезжает модели как есть: ровно так же ядро поступает с
 * `/name`, на которое не отзывается ни один файл.
 */
export function builtinFor(text: string, builtins: Builtin[]): Builtin | null {
  const name = text.trim().slice(1);
  if (!text.trim().startsWith("/")) return null;
  return builtins.includes(name as Builtin) ? (name as Builtin) : null;
}

/**
 * Встроенные, затем проектные — обе группы по алфавиту.
 *
 * Файл, назвавшийся именем встроенной команды, из меню выпадает — и на том
 * экране, где встроенной нет, тоже: `/compact`, означающий на домашнем экране
 * одно, а в разговоре другое, хуже, чем `/compact`, которого нет.
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
