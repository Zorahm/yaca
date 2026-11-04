import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ActionRow } from "../core/feed";
import { t, tp } from "../i18n";
import { ActionNotices } from "./ActionNotice";
import { ActionRowView } from "./ActionRow";
import { actionNotices, diagnosticInChanges, diagnosticsForPath, parseDiagnostic } from "./editNotices";
import { diagnosticCounts, LspBadge } from "./LspBadge";
import { Diff } from "./Diff";
import { Work } from "./Work";
import type { FeedActions } from "./Feed";

const diagnostic = "src/a.ts:3:12: error TS2322: Type 'string' is not assignable to type 'number'.";
const note = `Diagnostics from typescript-language-server: 1 errors\n${diagnostic}\n  const a: number = 'x';`;
const edit = (patch: Partial<ActionRow> = {}): ActionRow => ({
  callId: "edit", tool: "Edit", description: null, target: "src/a.ts", input: {},
  state: "done", message: null, meta: null, output: `Edited src/a.ts\n\n${note}`,
  fact: { path: "src/a.ts", kind: "modified", added: 1, removed: 1, movedFrom: null },
  diff: null, children: [], tokens: null, ...patch,
});

describe("post-edit notices", () => {
  it("extracts the server and complete diagnostics from an existing tool result", () => {
    expect(actionNotices(edit())).toEqual([{
      kind: "lsp-errors", server: "typescript-language-server",
      errors: 1, warnings: 0,
      detail: `${diagnostic}\n  const a: number = 'x';`,
    }]);
  });

  it("keeps separate notes for every server of a multi-file rename", () => {
    const pending = "Diagnostics from rust-analyzer did not arrive within 2 s: server has not replied yet. Request them via Lsp (action: diagnostics).";
    const notices = actionNotices(edit({ tool: "Rename", fact: null, children: [edit()], output: `${note}\n\n${pending}` }));
    expect(notices.map((notice) => notice.kind)).toEqual(["lsp-errors", "lsp-pending"]);
    expect(notices[1]).toMatchObject({ server: "rust-analyzer" });
  });

  it("does not label quoted output, an unfinished edit or a clean edit as an LSP error", () => {
    expect(actionNotices(edit({ tool: "Read", fact: null }))).toEqual([]);
    expect(actionNotices(edit({ tool: "Bash", fact: null }))).toEqual([]);
    expect(actionNotices(edit({ state: "running" }))).toEqual([]);
    expect(actionNotices(edit({ output: "Edited src/a.ts" }))).toEqual([]);
  });

  it("does not label a rejected edit as an LSP error", () => {
    const row = edit({ state: "rejected", fact: null, output: "", message: "src/a.ts has not been read in this session" });
    const html = renderToStaticMarkup(createElement(ActionRowView, { row, onOpenFile: () => {} }));
    expect(actionNotices(row)).toEqual([]);
    expect(html).toContain(row.message);
    expect(html).not.toContain("LSP");
  });

  it("marks the collapsed tool without displaying diagnostics outside it", () => {
    const html = renderToStaticMarkup(createElement(ActionRowView, { row: edit(), onOpenFile: () => {} }));
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain("row-lsp-errors");
    expect(html).toContain(t("row.lsp_counts_short", { counts: tp("row.lsp_error_count", 1) }));
    expect(html).not.toContain("TS2322");
    expect(html).not.toContain("row-diagnostics");
  });

  it("keeps only a compact LSP label when the entire work group is collapsed", () => {
    const html = renderToStaticMarkup(createElement(Work, {
      steps: [{ kind: "action", key: "edit", row: edit() }], live: false,
      preview: null, actions: {} as FeedActions,
    }));
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain(t("row.lsp_counts_short", { counts: tp("row.lsp_error_count", 1) }));
    expect(html).not.toContain("TS2322");
    expect(html).not.toContain("row-diagnostics");
    expect(html).not.toContain('class="group-state"');
  });

  it("shows the complete diagnostics inside the expanded tool", () => {
    const html = renderToStaticMarkup(createElement(ActionNotices, { notices: actionNotices(edit()), onOpenFile: () => {} }));
    expect(html).toContain(t("row.lsp_errors", { counts: tp("row.lsp_error_count", 1) }));
    expect(html).not.toContain(t("row.lsp_applied"));
    expect(html).toContain("typescript-language-server");
    expect(html).toContain("src/a.ts:3:12");
    expect(html).toContain("TS2322");
    expect(html).toContain('class="row-diagnostics-output"');
    expect(html).toContain('class="diagnostic-location"');
    expect(html).toContain(t("feed.copy"));
  });

  it("keeps complete counts when only some diagnostics are listed", () => {
    const notices = actionNotices(edit({ output: `Diagnostics from ts: 14 errors, 2 warnings in src/a.ts\nerror src/a.ts:3:12 bad type\n  continued => message\n[and 15 more]` }));
    expect(notices[0]).toMatchObject({ errors: 14, warnings: 2, path: "src/a.ts" });
    const html = renderToStaticMarkup(createElement(LspBadge, { notices }));
    expect(html).toContain(tp("row.lsp_error_count", 14));
    expect(html).toContain(tp("row.lsp_warning_count", 2));
  });

  it("distinguishes checked clean, warnings, pending and unchecked edits", () => {
    const clean = actionNotices(edit({ output: "Diagnostics from ts: 0 errors, 0 warnings in src/a.ts" }));
    expect(clean[0].kind).toBe("lsp-clean");
    expect(renderToStaticMarkup(createElement(LspBadge, { notices: clean }))).toContain(t("row.lsp_clean_short"));
    const warnings = actionNotices(edit({ output: "Diagnostics from ts: 0 errors, 2 warnings in src/a.ts\nwarning src/a.ts:1:1 unused" }));
    expect(renderToStaticMarkup(createElement(LspBadge, { notices: warnings }))).toContain("row-lsp-warnings");
    const pending = actionNotices(edit({ output: "Diagnostics from ts did not arrive within 2 s: server has not replied yet." }));
    expect(renderToStaticMarkup(createElement(LspBadge, { notices: [...clean, ...pending] }))).toContain(t("row.lsp_pending_short"));
    expect(renderToStaticMarkup(createElement(LspBadge, { notices: [] }))).toBe("");
  });

  it("parses both diagnostic formats including Windows paths and compiler codes", () => {
    expect(parseDiagnostic("error C:\\work dir\\src\\a.ts:14:22 Operator '=>' failed [typescript TS2365]"))
      .toMatchObject({ path: "C:\\work dir\\src\\a.ts", line: 14, column: 22, severity: "error", code: "TS2365", source: "typescript", message: "Operator '=>' failed" });
    expect(parseDiagnostic(diagnostic)).toMatchObject({ path: "src/a.ts", line: 3, column: 12, code: "TS2322" });
    expect(parseDiagnostic("  at src/a.ts:3:12")).toBeNull();
  });

  it("marks only matching new-side diff lines, leaving removed and other files alone", () => {
    const notices = actionNotices(edit());
    expect(diagnosticsForPath(notices, "other/a.ts")).toEqual([]);
    const diagnostics = diagnosticsForPath(notices, ".\\src\\a.ts");
    const html = renderToStaticMarkup(createElement(Diff, {
      diff: { added: 2, removed: 1, hunks: [{ oldStart: 3, oldLines: 1, newStart: 3, newLines: 2, lines: [
        { kind: "del", oldLine: 3, newLine: null, text: "old", spans: [] },
        { kind: "add", oldLine: null, newLine: 3, text: "bad", spans: [] },
        { kind: "add", oldLine: null, newLine: 4, text: "good", spans: [] },
      ] }], truncated: false }, diagnostics,
    }));
    expect(html.match(/class="diff-diagnostic-mark /g)).toHaveLength(1);
    expect(html).toContain("diff-add diff-diagnostic diff-diagnostic-error");
  });

  it("moves the native compiler code before its location and wraps the whole message", () => {
    const notices = actionNotices(edit({ output: "Diagnostics from ts: 1 errors, 2 warnings in src/a.ts\nerror src/a.ts:3:2 Operator '=>' failed [typescript TS2365]" }));
    const html = renderToStaticMarkup(createElement(ActionNotices, { notices, onOpenFile: () => {} }));
    expect(html.indexOf("TS2365")).toBeLessThan(html.indexOf("src/a.ts:3:2"));
    expect(html).toContain("diagnostic-entry");
    expect(html).toContain(diagnosticCounts(notices, true));
    expect(diagnosticCounts(notices, true)).toContain(tp("row.lsp_warning_count_full", 2));
    expect(diagnosticCounts(notices)).toContain(tp("row.lsp_warning_count", 2));
    expect(html).toContain("[typescript]");
  });

  it("labels context and absent lines as outside the changed lines", () => {
    const diff = { added: 1, removed: 0, truncated: false, hunks: [{ oldStart: 2, oldLines: 1, newStart: 2, newLines: 2, lines: [
      { kind: "context" as const, oldLine: 2, newLine: 2, text: "context", spans: [] },
      { kind: "add" as const, oldLine: null, newLine: 3, text: "changed", spans: [] },
    ] }] };
    const diagnostic = parseDiagnostic("error src/a.ts:3:1 bad type")!;
    expect(diagnosticInChanges(diagnostic, diff, "src/a.ts")).toBe(true);
    expect(diagnosticInChanges({ ...diagnostic, line: 2 }, diff, "src/a.ts")).toBe(false);
    expect(diagnosticInChanges({ ...diagnostic, line: 280 }, diff, "src/a.ts")).toBe(false);
    expect(diagnosticInChanges(diagnostic, diff, "other/a.ts")).toBe(false);
    const notices = actionNotices(edit({ output: "Diagnostics from ts: 1 errors, 1 warnings in src/a.ts\nerror src/a.ts:3:1 bad type\nwarning src/a.ts:280:7 unused" }));
    const html = renderToStaticMarkup(createElement(ActionNotices, { notices, diff, path: "src/a.ts", onOpenFile: () => {} }));
    expect(html.match(/class="diagnostic-scope"/g)).toHaveLength(1);
    expect(html).toContain(t("row.lsp_outside_changes"));
    expect(renderToStaticMarkup(createElement(ActionNotices, { notices, onOpenFile: () => {} }))).not.toContain("diagnostic-scope");
  });

  it("separates gutter dots and underlines the reported character across Unicode and word spans", () => {
    const diagnostics = [parseDiagnostic("error src/a.ts:3:3 bad type")!, parseDiagnostic("warning src/a.ts:4:2 unused")!];
    const html = renderToStaticMarkup(createElement(Diff, {
      soft: true,
      diff: { added: 2, removed: 0, truncated: false, hunks: [{ oldStart: 3, oldLines: 0, newStart: 3, newLines: 2, lines: [
        { kind: "add", oldLine: null, newLine: 3, text: "😀xy", spans: [{ start: 1, end: 3 }] },
        { kind: "add", oldLine: null, newLine: 4, text: "ab", spans: [] },
      ] }] }, diagnostics,
    }));
    expect(html).toContain('class="diff-diagnostic-cell"');
    expect(html).toContain('class="diff-diagnostic-mark diff-diagnostic-mark-error"');
    expect(html).toContain('class="diff-diagnostic-mark diff-diagnostic-mark-warning"');
    expect(html).toContain('class="diff-gutter">3</span>');
    expect(html).toContain('class="word diagnostic-squiggle diagnostic-squiggle-error">y</span>');
    expect(html).toContain('class="diagnostic-squiggle diagnostic-squiggle-warning">b</span>');
  });
});
