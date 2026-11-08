import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ActionRow } from "../core/feed";
import { fold, type WorkStep } from "../core/work";
import { t, tp } from "../i18n";
import { actionNotices } from "./editNotices";
import { fileDirectories, groupFileSteps, uniqueFileNames } from "./fileActivity";
import { FileActivity } from "./FileActivityGroup";
import { FileLspBadge } from "./LspBadge";

function call(key: string, tool: string, path: string, patch: Partial<ActionRow> = {}) {
  return {
    kind: "action", key, row: {
      callId: key, tool, description: null, target: path, input: {}, state: "done",
      message: null, meta: null, output: "", fact: null, diff: null, children: [], tokens: null, ...patch,
    },
  } satisfies WorkStep;
}

const read = (language: string) => call(`read-${language}`, "Read", `src/i18n/${language}/settings.ts`);
const edit = (language: string, patch?: Partial<ActionRow>) => {
  const path = `src/i18n/${language}/settings.ts`;
  return call(`edit-${language}`, "Edit", path, {
    fact: { kind: "modified", path, added: 1, removed: 1, movedFrom: null }, ...patch,
  });
};

describe("file activity groups", () => {
  it("uses two reads or three writes, with stable anchors and order for alternating tools", () => {
    const [ja, de, fr] = ["ja", "de", "fr"].map((language) => [read(language), edit(language)]);
    expect(groupFileSteps([...ja, ...de]).get(ja[1].key)).toBeUndefined();
    expect(groupFileSteps([...ja, ...de]).get(ja[0].key)?.steps).toEqual([ja[0], de[0]]);
    const groups = groupFileSteps([...ja, ...de, ...fr]);
    expect(groups.get(ja[1].key)?.steps).toEqual([ja[1], de[1], fr[1]]);
    expect(groups.get(de[1].key)).toBeNull();
  });

  it("keeps a model message as a boundary", () => {
    const one = read("ja");
    const two = read("de");
    const items = fold([
      { kind: "actions", key: "a", rows: [one.row] },
      { kind: "agent", key: "b", blockId: "b", text: "Next file" },
      { kind: "actions", key: "c", rows: [two.row] },
    ]);
    expect(items.map((item) => item.kind)).toEqual(["work", "agent", "work"]);
    expect(items.flatMap((item) => item.kind === "work" ? [...groupFileSteps(item.steps)] : [])).toEqual([]);
  });

  it("resolves path collisions and describes shallow or unrelated directories", () => {
    expect([...uniqueFileNames([
      "src/a/ja/settings.ts", ".\\src\\b\\ja\\settings.ts", "src/de/settings.ts", "src/other.ts",
    ]).values()]).toEqual(["a/ja/settings.ts", "b/ja/settings.ts", "de/settings.ts", "other.ts"]);
    expect(fileDirectories(["ja", "de", "fr", "es", "ru"].map((lang) => `src/i18n/${lang}/settings.ts`)))
      .toEqual({ prefix: "src/i18n/", compact: "src/i18n/{ja, de, fr, es, …+1}/", count: 5 });
    expect(fileDirectories(["src/one/a.ts", "src/two/b.ts"])).toEqual({ prefix: "", compact: "", count: 2 });
    expect(fileDirectories(["/a.ts", "b.ts"])).toEqual({ prefix: "", compact: "", count: 2 });
  });

  it("opens a group with failed edits and prioritizes the expanded diagnostic error", () => {
    const warning = "Diagnostics from ts: 0 errors, 2 warnings in src/i18n/ja/settings.ts";
    const error = "Diagnostics from ts: 1 errors, 0 warnings in src/i18n/de/settings.ts\nerror src/i18n/de/settings.ts:1:1 bad type [ts TS2322]";
    const steps = [
      edit("ja", { output: warning }),
      edit("fr", { state: "rejected", fact: null }),
      edit("de", { output: error }),
    ];
    const html = renderToStaticMarkup(createElement(FileActivity, {
      group: groupFileSteps(steps).get(steps[0].key)!, onOpenFile: () => {},
    }));
    expect(html).toContain('aria-expanded="true"');
    expect(html).toContain(tp("group.failed", 1));
    expect(html).toContain("+2");
    expect(html).toContain("−2");
    expect(html).toContain(t("row.lsp_counts_short", { counts: tp("row.lsp_error_count", 1) }));
    expect(html.indexOf('class="file-detail-directory">de/')).toBeLessThan(html.indexOf('class="file-detail-directory">ja/'));
    expect(html).toContain("bad type");
    expect(html).toContain('title="src/i18n/de/settings.ts:1:1">de/settings.ts:1:1</button>');
    expect(html).toContain("TS2322");
    expect(html).not.toContain("[ts]");
  });

  it("replaces a pending check with its late report and counts checked files", () => {
    const path = "src/i18n/ja/settings.ts";
    const pending = "Diagnostics from ts did not arrive within 2 s: no answer.";
    const clean = `Diagnostics from ts: 0 errors, 0 warnings in ${path}`;
    const waiting = actionNotices(edit("ja", { output: pending }).row);
    const checked = actionNotices(edit("ja", { output: `${pending}\nFile: ${path}\n\n${clean}` }).row);
    expect(checked.map((notice) => notice.kind)).toEqual(["lsp-clean"]);
    expect(renderToStaticMarkup(createElement(FileLspBadge, { files: [waiting, checked] })))
      .toContain(tp("file_group.pending", 1));
    expect(renderToStaticMarkup(createElement(FileLspBadge, { files: [checked] })))
      .toContain(t("file_group.checked", { n: 1 }));
  });
});
