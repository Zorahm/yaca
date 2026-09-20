import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { fenceOf, languageOf } from "./languages";

describe("languageOf", () => {
  it("maps the extensions this app actually edits", () => {
    expect(languageOf("src/main.rs")).toBe("rust");
    expect(languageOf("src\\ui\\Diff.tsx")).toBe("typescript-tsx");
    expect(languageOf("app.ts")).toBe("typescript");
    expect(languageOf("pkg.json")).toBe("json");
    expect(languageOf("Cargo.toml")).toBe("toml");
    expect(languageOf("notes.md")).toBe("markdown");
    expect(languageOf("run.sh")).toBe("shell");
  });

  it("answers null for names with no usable extension", () => {
    expect(languageOf("Makefile")).toBeNull();
    expect(languageOf(".gitignore")).toBeNull();
    expect(languageOf("no-extension")).toBeNull();
  });

  it("answers null for extensions outside the grammar map", () => {
    expect(languageOf("notes.txt")).toBeNull();
    expect(languageOf("image.png")).toBeNull();
  });

  it("matches extensions case-insensitively", () => {
    expect(languageOf("App.TSX")).toBe("typescript-tsx");
  });
});

describe("fenceOf", () => {
  it("resolves the aliases models write into fences", () => {
    expect(fenceOf("ts")).toBe("typescript");
    expect(fenceOf("rust")).toBe("rust");
    expect(fenceOf("sh")).toBe("shell");
    expect(fenceOf("yml")).toBe("yaml");
  });

  it("answers null for an empty or unknown fence", () => {
    // An empty fence is the block around command output: it must stay plain.
    expect(fenceOf("")).toBeNull();
    expect(fenceOf("mermaid")).toBeNull();
  });
});

/**
 * The offline guarantee: grammars load from the bundle, never from the
 * network. The module must not grow a `fetch` or an absolute URL — the test
 * reads its own source and holds the line.
 */
describe("languages source", () => {
  it("never reaches outside the bundle", () => {
    const source = readFileSync(fileURLToPath(new URL("./languages.ts", import.meta.url)), "utf8");
    expect(source.includes("fetch(")).toBe(false);
    expect(source).not.toMatch(/https?:\/\//);
  });
});
