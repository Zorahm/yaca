import { Check, Copy } from "@phosphor-icons/react";
import type { FileDiff } from "../core/client";
import { t } from "../i18n";
import { diagnosticInChanges, parseDiagnostic, type ActionNotice } from "./editNotices";
import { pathKey } from "./fileActivity";
import { diagnosticCounts } from "./LspBadge";
import { useCopy } from "./useCopy";

type NoticeProps = {
  onOpenFile: (path: string, line?: number) => void;
  diff?: FileDiff | null;
  path?: string;
  directoryPrefix?: string;
};

export function ActionNotices({ notices, ...props }: NoticeProps & {
  notices: ActionNotice[];
}) {
  return notices.filter((notice) => notice.kind !== "lsp-clean").map((notice, index) => (
    <Notice key={index} notice={notice} {...props} />
  ));
}

function Notice({ notice, onOpenFile, diff, path, directoryPrefix }: NoticeProps & {
  notice: ActionNotice;
}) {
  const { copied, copy } = useCopy();
  const pending = notice.kind === "lsp-pending";
  const label = t(copied ? "feed.copied" : "feed.copy");
  return (
    <div className={`row-diagnostics${notice.errors > 0 ? " row-diagnostics-error" : ""}`}>
      <div className="row-diagnostics-head">
        <div className="row-diagnostics-title">
          {t(pending ? "row.lsp_pending" : "row.lsp_errors", { counts: diagnosticCounts([notice], true) })}
        </div>
        {notice.detail && (
          <button className="row-diagnostics-copy" aria-label={label} title={label} onClick={() => copy(notice.detail)}>
            {copied ? <Check size={13} /> : <Copy size={13} />}
            {label}
          </button>
        )}
      </div>
      <div className="row-diagnostics-server">{notice.server}</div>
      {pending && <div className="row-dim">{t("row.lsp_waiting")}</div>}
      {notice.detail && <div className="row-diagnostics-output">{notice.detail.split(/\r?\n/).map((line, index) => {
        const diagnostic = parseDiagnostic(line);
        const outside = diagnostic && diff && path !== undefined && !diagnosticInChanges(diagnostic, diff, path);
        const diagnosticPath = diagnostic && pathKey(diagnostic.path);
        const location = diagnostic && directoryPrefix && diagnosticPath && diagnosticPath.startsWith(directoryPrefix)
          ? `${diagnosticPath.slice(directoryPrefix.length)}:${diagnostic.line}:${diagnostic.column}`
          : diagnostic?.location;
        return <div className="diagnostic-entry" key={index}>
          {diagnostic ? <>
            <span className={`diagnostic-${diagnostic.severity}`}>{diagnostic.severity}</span>{" "}
            {diagnostic.code && <><span className="diagnostic-code">{diagnostic.code}</span>{" "}</>}
            <button className="diagnostic-location" title={location !== diagnostic.location ? diagnostic.location : undefined}
              onClick={() => onOpenFile(diagnostic.path, diagnostic.line)}>{location}</button>{" "}
            {diagnostic.message}
            {diagnostic.source && diagnostic.source.toLowerCase() !== notice.server.toLowerCase() &&
              <> <span className="diagnostic-code">[{diagnostic.source}]</span></>}
            {outside && <span className="diagnostic-scope">{t("row.lsp_outside_changes")}</span>}
          </> : line || "\u00a0"}
        </div>;
      })}</div>}
    </div>
  );
}
