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
import { caption, rowCounts, rowFiles, summary } from "./captions";
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

describe("шапка группы с подагентом", () => {
  it("считает то, что подагент сделал с проектом", () => {
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
    // Файл создан, команда выполнена — кем, шапке всё равно: она про проект.
    expect(head.parts).toHaveLength(3);
    expect(head.added).toBe(3);
  });
});
