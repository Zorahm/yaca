import type { ActionRow } from "../core/feed";
import type { FileDiff } from "../core/client";

export type Diagnostic = {
  path: string;
  line: number;
  column: number;
  severity: "error" | "warning";
  code: string;
  source?: string;
  message: string;
  location: string;
};

export type ActionNotice = {
  kind: "lsp-errors" | "lsp-clean" | "lsp-pending";
  server: string;
  path?: string;
  detail: string;
  errors: number;
  warnings: number;
};

/** Both the core's severity-first format and compiler output in older journals. */
export function parseDiagnostic(line: string): Diagnostic | null {
  const native = /^(error|warning) (.+?):(\d+):(\d+) ([\s\S]*)$/.exec(line);
  const compiler = /^(.+?):(\d+):(\d+): (error|warning)(?: ([\w-]+))?: ([\s\S]*)$/.exec(line);
  if (!native && !compiler) return null;
  const [path, row, column, severity, compilerCode, originalMessage] = native
    ? [native[2], native[3], native[4], native[1], "", native[5]]
    : [compiler![1], compiler![2], compiler![3], compiler![4], compiler![5] ?? "", compiler![6]];
  const tag = native ? /\s+\[([^\]\n]+)\]$/.exec(originalMessage) : null;
  const parts = tag?.[1].split(/\s+/) ?? [];
  const hasCode = parts.length > 1 || (parts.length === 1 && /\d/.test(parts[0]));
  const code = compilerCode || (hasCode ? parts.pop()! : "");
  const source = parts.join(" ");
  return {
    path, line: Number(row), column: Number(column),
    severity: severity as Diagnostic["severity"], code,
    ...(source ? { source } : {}),
    message: tag ? originalMessage.slice(0, tag.index) : originalMessage,
    location: `${path}:${row}:${column}`,
  };
}

/** Context lines are visible in a diff but were not changed by this action. */
export function diagnosticInChanges(diagnostic: Diagnostic, diff: FileDiff, path: string): boolean {
  return sameDiagnosticPath(diagnostic.path, path) && diff.hunks.some((hunk) =>
    hunk.lines.some((line) => line.kind === "add" && line.newLine === diagnostic.line));
}

function sameDiagnosticPath(left: string, right: string): boolean {
  const normalize = (value: string) => value.replace(/\\/g, "/").replace(/^\.\//, "");
  return normalize(left) === normalize(right);
}

/** Read the core's post-edit notes, including journals saved before this UI existed. */
export function actionNotices(row: ActionRow): ActionNotice[] {
  // A read or command can quote diagnostics without having changed a file.
  if (row.state !== "done" || (!row.fact && !row.children.some((child) => child.fact))) {
    return [];
  }
  return row.output.split(/\r?\n\r?\n(?=Diagnostics from )/).flatMap<ActionNotice>((block) => {
    const report = /^Diagnostics from (.+?): (\d+) errors(?:, (\d+) warnings)?(?: in ([^\r\n]+))?(?:\r?\n([\s\S]*))?$/.exec(block);
    if (report) {
      const errors = Number(report[2]);
      const warnings = Number(report[3] ?? 0);
      return [{
        kind: errors + warnings > 0 ? "lsp-errors" : "lsp-clean",
        server: report[1], ...(report[4] ? { path: report[4] } : {}),
        errors, warnings, detail: (report[5] ?? "").trim(),
      }];
    }
    const pending = /^Diagnostics from (.+?) did not arrive within \d+ s:/.exec(block);
    return pending
      ? [{ kind: "lsp-pending", server: pending[1], detail: "", errors: 0, warnings: 0 }]
      : [];
  });
}

export function diagnosticsForPath(notices: ActionNotice[], path: string): Diagnostic[] {
  return notices.flatMap((notice) => notice.detail.split(/\r?\n/).flatMap((line) => {
    const diagnostic = parseDiagnostic(line);
    return diagnostic && sameDiagnosticPath(diagnostic.path, path) ? [diagnostic] : [];
  }));
}
