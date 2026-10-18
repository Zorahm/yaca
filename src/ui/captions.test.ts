/**
 * A deleted directory in the feed (§5.5): `Deleted directory <name>`, counted
 * in files rather than lines, and in files in the group's summary too.
 */

import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ActionRow } from "../core/feed";
import type { PermissionRequest } from "../core/client";
import { capitalize, t, tp } from "../i18n";
import { caption, filePath, isFileRow, rowCounts, rowFiles, summary } from "./captions";
import { Permission } from "./Permission";

function row(patch: Partial<ActionRow>): ActionRow {
  return {
    callId: "c1",
    tool: "DeleteDir",
    description: null,
    target: "build/old",
    input: { path: "build/old" },
    state: "done",
    message: null,
    meta: { lines: 1, items: 3, truncated: false, durationMs: 5 },
    output: "",
    fact: null,
    diff: null,
    children: [],
    tokens: null,
    ...patch,
  };
}

/** The caption as text, without the markup around the name. */
function text(node: ReturnType<typeof caption>): string {
  return renderToStaticMarkup(createElement("span", null, node)).replace(/<[^>]+>/g, "");
}

describe("a deleted directory", () => {
  it("is captioned as a directory while it goes and once it is gone", () => {
    expect(text(caption(row({ state: "running", meta: null })))).toBe(
      t("act.deleting_dir", { name: "old" }),
    );
    expect(text(caption(row({})))).toBe(t("act.deleted_dir", { name: "old" }));
  });

  it("counts files, not lines", () => {
    expect(rowFiles(row({}))).toBe(3);
    expect(rowCounts(row({}))).toBeNull();
    // Nothing to count until it is done.
    expect(rowFiles(row({ state: "running", meta: null }))).toBeNull();
  });

  it("adds its files to the deleted ones in the summary", () => {
    const { parts } = summary([row({})]);
    expect(parts.map((p) => p.text)).toEqual([capitalize(tp("group.deleted.with", 3))]);
  });
});

describe("the question about deleting a directory", () => {
  const request = (files: PermissionRequest["files"]): PermissionRequest => ({
    id: "r1",
    callId: "c1",
    kind: "deleteDirectory",
    target: "old",
    detail: "build/old",
    note: null,
    sizeBytes: null,
    files,
    unsaved: null,
    invariant: null, server: null,
    scopes: [],
    position: 1,
    total: 1,
  });
  const markup = (files: PermissionRequest["files"]) =>
    renderToStaticMarkup(createElement(Permission, { request: request(files), onAnswer: () => {} }));

  it("says how many files go with it", () => {
    expect(markup({ files: 3, more: false })).toContain(tp("ask.files", 3));
  });

  it("says when the count stopped short", () => {
    expect(markup({ files: 10000, more: true })).toContain(tp("ask.files_more", 10000));
  });
});

describe("group header with a subagent", () => {
  it("counts what the subagent did to the project", () => {
    const created = { path: "a.txt", kind: "created" as const, movedFrom: null, added: 3, removed: 0 };
    const task = row({
      tool: "Task",
      target: null,
      meta: null,
      children: [
        row({ callId: "w1", tool: "Write", target: "a.txt", fact: created }),
        row({ callId: "b1", tool: "Bash", target: "npm test", meta: null }),
      ],
    });

    const head = summary([task]);
    // The file was created and the command run — by whom does not matter to the
    // header: it is about the project.
    expect(head.parts).toHaveLength(3);
    expect(head.added).toBe(3);
  });
});

describe("a read file", () => {
  const read = (patch: Partial<ActionRow>) =>
    row({ tool: "Read", target: "crates/yaca-cli/Cargo.toml", meta: null, ...patch });

  it("is captioned by the interface, with the file's name", () => {
    // The model's own description gives way: the name has to be in the row.
    const done = read({ description: "Read the manifest" });
    expect(text(caption(done))).toBe(t("act.read", { name: "Cargo.toml" }));
    expect(text(caption(read({ state: "running" })))).toBe(
      t("act.reading", { name: "Cargo.toml" }),
    );
  });

  it("opens in the file panel once it is read, by the path it was read by", () => {
    expect(filePath(read({}))).toBe("crates/yaca-cli/Cargo.toml");
    expect(filePath(read({ state: "running" }))).toBeNull();
    expect(isFileRow(read({}))).toBe(true);
  });

  it("carries the path as the name's tooltip", () => {
    const html = renderToStaticMarkup(
      createElement("span", null, caption(read({}), () => {})),
    );
    expect(html).toContain('title="crates/yaca-cli/Cargo.toml"');
  });
});
