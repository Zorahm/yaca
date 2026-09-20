/**
 * Слэш-команды композера §7.3.
 *
 * Две породы в одном меню. Встроенные — то, что делает сам интерфейс: в ядро
 * уезжает `Command`, а не текст. Проектные лежат файлами в
 * `.yaca/commands/*.md`, и разворачивает их ядро в момент отправки — окну от
 * них нужны имя и описание, чтобы нарисовать строку.
 *
 * Встроенных здесь четыре из восьми, перечисленных в §7.3. `/mcp`, `/init`,
 * `/cost` и `/help` ведут в MCP, файл проекта, разбор трат и справку —
 * подсистемы и экраны, которых нет, а команда, которая ничего не открывает,
 * ничем не лучше кнопки, которая ничего не открывает (§0). Приедут вместе со
 * своими подсистемами.
 *
 * Меню — функция от текста поля, а не отдельное состояние: закрывается оно
 * тем, что текст перестал быть командой. Флаг «меню открыто» был бы второй
 * версией той же правды и разошёлся бы с ней на первом же `Backspace`.
 */

import type { SlashCommand } from "../core/client";
import { t, type Key } from "../i18n";

/** Встроенная команда — то, что интерфейс умеет сделать сам. */
export type Builtin = "checkpoints" | "clear" | "compact" | "model";

/**
 * Все имена, занятые встроенными командами.
 *
 * Шире, чем список доступных на экране: на домашнем экране §9.1 сжимать
 * нечего, но `/compact` там не должен вдруг означать чужой файл. Имя команды
 * значит одно и то же везде.
 */
export const RESERVED: Builtin[] = ["checkpoints", "clear", "compact", "model"];

const NOTE: Record<Builtin, Key> = {
  checkpoints: "slash.checkpoints",
  clear: "slash.clear",
  compact: "slash.compact",
  model: "slash.model",
};

/** Строка меню. Имя — без слэша, как и у файла. */
export type SlashItem = {
  name: string;
  description: string;
  /** `null` у проектной команды: её разворачивает ядро. */
  builtin: Builtin | null;
};

/**
 * Что показывает меню для набранного текста, или `null`, когда меню не нужно.
 *
 * Пробел закрывает меню: дальше идут аргументы, и дополнять уже нечего.
 */
export function menuFor(
  text: string,
  project: SlashCommand[],
  builtins: Builtin[],
): SlashItem[] | null {
  if (!text.startsWith("/")) return null;
  const typed = text.slice(1);
  if (/\s/.test(typed)) return null;

  const prefix = typed.toLowerCase();
  const items = all(project, builtins).filter((item) => item.name.startsWith(prefix));
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
function all(project: SlashCommand[], builtins: Builtin[]): SlashItem[] {
  const taken = new Set<string>(RESERVED);
  return [
    ...[...builtins].sort().map((name) => ({
      name,
      description: t(NOTE[name]),
      builtin: name,
    })),
    ...project
      .filter((command) => !taken.has(command.name))
      .map((command) => ({
        name: command.name,
        description: command.description,
        builtin: null,
      })),
  ];
}
