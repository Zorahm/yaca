/**
 * An MCP server as the settings form holds it (§10).
 *
 * The file keeps a server as a program with a list of arguments and two maps;
 * a form keeps text. This is the road between the two, kept apart from the
 * form so it can be tested without rendering one.
 *
 * Arguments are one per line rather than one string split on spaces: an
 * argument may hold a space, and the core starts the program directly, with no
 * shell to split a line by its rules. Environment and headers are one pair per
 * line for the same reason a `.env` file is.
 */

import type { McpServerSettings } from "../../core/client";
import type { Key } from "../../i18n";

export type McpKind = "stdio" | "http";

export type McpDraft = {
  name: string;
  kind: McpKind;
  command: string;
  /** One argument per line. */
  args: string;
  /** `NAME=value`, one per line. */
  env: string;
  cwd: string;
  url: string;
  /** `Name: value`, one per line. */
  headers: string;
  disabled: boolean;
  /**
   * What the form does not edit — the two timeouts — carried through a save
   * as it was read. A form that dropped them would quietly undo a line
   * written by hand.
   */
  kept: Pick<McpServerSettings, "startupTimeoutSecs" | "toolTimeoutSecs">;
};

export function emptyDraft(): McpDraft {
  return {
    name: "",
    kind: "stdio",
    command: "",
    args: "",
    env: "",
    cwd: "",
    url: "",
    headers: "",
    disabled: false,
    kept: { startupTimeoutSecs: null, toolTimeoutSecs: null },
  };
}

function pairsText(map: { [key in string]?: string }, separator: string): string {
  return Object.entries(map)
    .map(([name, value]) => `${name}${separator}${value ?? ""}`)
    .join("\n");
}

export function draftOf(name: string, settings: McpServerSettings): McpDraft {
  return {
    name,
    kind: settings.url !== null && settings.command === null ? "http" : "stdio",
    command: settings.command ?? "",
    args: settings.args.join("\n"),
    env: pairsText(settings.env, "="),
    cwd: settings.cwd ?? "",
    url: settings.url ?? "",
    headers: pairsText(settings.headers, ": "),
    disabled: settings.disabled,
    kept: {
      startupTimeoutSecs: settings.startupTimeoutSecs,
      toolTimeoutSecs: settings.toolTimeoutSecs,
    },
  };
}

const lines = (text: string) =>
  text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");

/** `null` for a line that has no separator or no name before it. */
function pairs(text: string, separator: string): Record<string, string> | null {
  const out: Record<string, string> = {};
  for (const line of lines(text)) {
    const at = line.indexOf(separator);
    if (at <= 0) return null;
    out[line.slice(0, at).trim()] = line.slice(at + separator.length).trim();
  }
  return out;
}

/**
 * What goes to the file. Only the half the chosen kind uses is written: a
 * command left over under an address would be a declaration the core refuses
 * for naming both.
 */
export function settingsOf(draft: McpDraft): McpServerSettings {
  const stdio = draft.kind === "stdio";
  const blank = (text: string) => (text.trim() === "" ? null : text.trim());
  return {
    command: stdio ? blank(draft.command) : null,
    args: stdio ? lines(draft.args) : [],
    env: stdio ? (pairs(draft.env, "=") ?? {}) : {},
    cwd: stdio ? blank(draft.cwd) : null,
    type: stdio ? null : "http",
    url: stdio ? null : blank(draft.url),
    headers: stdio ? {} : (pairs(draft.headers, ":") ?? {}),
    disabled: draft.disabled,
    ...draft.kept,
  };
}

/** Letters a tool name may carry: the server's name becomes part of one. */
const NAME = /^[A-Za-z0-9_-]+$/;

/**
 * Why the draft cannot be saved. The same things the core would refuse, said
 * before the file is written rather than as a failed server after it.
 */
export function problemsOf(draft: McpDraft, taken: string[], isNew: boolean): Key[] {
  const out: Key[] = [];
  const name = draft.name.trim();
  if (name === "") out.push("mcp.problem_name_empty");
  else if (!NAME.test(name)) out.push("mcp.problem_name_chars");
  else if (isNew && taken.includes(name)) out.push("mcp.problem_name_taken");

  if (draft.kind === "stdio") {
    if (draft.command.trim() === "") out.push("mcp.problem_command_empty");
    if (pairs(draft.env, "=") === null) out.push("mcp.problem_env");
  } else {
    if (!/^https?:\/\/\S+$/.test(draft.url.trim())) out.push("mcp.problem_url");
    if (pairs(draft.headers, ":") === null) out.push("mcp.problem_headers");
  }
  return out;
}

/** The line a server is recognised by in a list: its command or its address. */
export function targetOf(settings: McpServerSettings): string {
  if (settings.url !== null && settings.command === null) return settings.url;
  return [settings.command ?? "", ...settings.args].join(" ").trim();
}
