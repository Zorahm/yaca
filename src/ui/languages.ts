/**
 * Which grammar to use for a piece of code, and loading it from the bundle.
 *
 * Only the mapping here is synchronous; every parser is fetched with a
 * dynamic `import()` so Vite code-splits each grammar into its own chunk.
 * Nothing is fetched over the network — grammars load from the local bundle
 * that ships with the app, even fully offline.
 *
 * Until a parser arrives the caller renders plain text; when it lands, the
 * component re-renders with highlighting. A blocked render path is never
 * traded for earlier color: an awaited parser would postpone first paint of
 * every diff.
 */

import type { Parser } from "@lezer/common";

/** Language ids are internal keys; only `languageOf` and `fenceOf` mint them. */
export type LanguageId =
  | "javascript"
  | "javascript-jsx"
  | "typescript"
  | "typescript-tsx"
  | "rust"
  | "python"
  | "css"
  | "html"
  | "json"
  | "markdown"
  | "toml"
  | "yaml"
  | "shell";

/**
 * Extension → language, lowercase extension without the dot. Kept deliberately
 * modest: the languages this app's diffs actually show are source files,
 *.toml manifests, and documents. Extending is one line in either map.
 */
const EXTENSIONS: ReadonlyMap<string, LanguageId> = new Map([
  ["js", "javascript"],
  ["mjs", "javascript"],
  ["cjs", "javascript"],
  ["jsx", "javascript-jsx"],
  ["ts", "typescript"],
  ["mts", "typescript"],
  ["cts", "typescript"],
  ["tsx", "typescript-tsx"],
  ["rs", "rust"],
  ["py", "python"],
  ["css", "css"],
  ["html", "html"],
  ["htm", "html"],
  ["json", "json"],
  ["md", "markdown"],
  ["markdown", "markdown"],
  ["toml", "toml"],
  ["yaml", "yaml"],
  ["yml", "yaml"],
  ["sh", "shell"],
  ["bash", "shell"],
  ["zsh", "shell"],
]);

/** Fenced-code info strings → language. Aliases a model actually writes. */
const FENCES: ReadonlyMap<string, LanguageId> = new Map([
  ["js", "javascript"],
  ["jsx", "javascript-jsx"],
  ["javascript", "javascript"],
  ["mjs", "javascript"],
  ["cjs", "javascript"],
  ["ts", "typescript"],
  ["tsx", "typescript-tsx"],
  ["typescript", "typescript"],
  ["rs", "rust"],
  ["rust", "rust"],
  ["py", "python"],
  ["python", "python"],
  ["css", "css"],
  ["html", "html"],
  ["json", "json"],
  ["md", "markdown"],
  ["markdown", "markdown"],
  ["toml", "toml"],
  ["yaml", "yaml"],
  ["yml", "yaml"],
  ["sh", "shell"],
  ["bash", "shell"],
  ["shell", "shell"],
  ["zsh", "shell"],
]);

/** Language for a file path, or `null` when no grammar is worth loading. */
export function languageOf(path: string): LanguageId | null {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  if (dot < 1) return null;
  return EXTENSIONS.get(name.slice(dot + 1).toLowerCase()) ?? null;
}

/** Language for the info string after a ``` fence, or `null`. */
export function fenceOf(info: string): LanguageId | null {
  return FENCES.get(info.toLowerCase()) ?? null;
}

// Each loader resolves a Parser synchronously from the module it imports —
// Lezer parsers and `StreamLanguage.define(...).parser` parse without await.
const LOADERS: Readonly<Record<LanguageId, () => Promise<Parser>>> = {
  javascript: () => import("@lezer/javascript").then((m) => m.parser),
  "javascript-jsx": () =>
    import("@lezer/javascript").then((m) => m.parser.configure({ dialect: "jsx" })),
  typescript: () =>
    import("@lezer/javascript").then((m) => m.parser.configure({ dialect: "ts" })),
  "typescript-tsx": () =>
    import("@lezer/javascript").then((m) => m.parser.configure({ dialect: "ts,jsx" })),
  rust: () => import("@lezer/rust").then((m) => m.parser),
  python: () => import("@lezer/python").then((m) => m.parser),
  css: () => import("@lezer/css").then((m) => m.parser),
  html: () => import("@lezer/html").then((m) => m.parser),
  json: () => import("@lezer/json").then((m) => m.parser),
  markdown: () => import("@lezer/markdown").then((m) => m.parser),
  // Stream-derived languages: the legacy mode is wrapped the same way the
  // editor does it, and the resulting parser produces ordinary Lezer trees.
  toml: () =>
    Promise.all([
      import("@codemirror/language"),
      import("@codemirror/legacy-modes/mode/toml"),
    ]).then(([language, mode]) => language.StreamLanguage.define(mode.toml).parser),
  yaml: () =>
    Promise.all([
      import("@codemirror/language"),
      import("@codemirror/legacy-modes/mode/yaml"),
    ]).then(([language, mode]) => language.StreamLanguage.define(mode.yaml).parser),
  shell: () =>
    Promise.all([
      import("@codemirror/language"),
      import("@codemirror/legacy-modes/mode/shell"),
    ]).then(([language, mode]) => language.StreamLanguage.define(mode.shell).parser),
};

const resolved = new Map<LanguageId, Promise<Parser | null>>();

/**
 * Parser for a language, loaded once and reused. `null` means known to be
 * broken: the caller keeps rendering plain text instead of retrying forever.
 */
export function parserOf(language: LanguageId): Promise<Parser | null> {
  let promise = resolved.get(language);
  if (promise === undefined) {
    promise = LOADERS[language]().catch(() => null);
    resolved.set(language, promise);
  }
  return promise;
}
