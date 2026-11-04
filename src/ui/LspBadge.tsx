import { Check, Clock, WarningCircle } from "@phosphor-icons/react";
import { t, tp } from "../i18n";
import type { ActionNotice } from "./editNotices";

export function diagnosticCounts(notices: ActionNotice[], full = false): string {
  const errors = notices.reduce((sum, notice) => sum + notice.errors, 0);
  const warnings = notices.reduce((sum, notice) => sum + notice.warnings, 0);
  return [errors > 0 && tp("row.lsp_error_count", errors),
    warnings > 0 && tp(full ? "row.lsp_warning_count_full" : "row.lsp_warning_count", warnings),
  ].filter(Boolean).join(", ");
}

export function LspBadge({ notices }: { notices: ActionNotice[] }) {
  if (notices.length === 0) return null;
  const counts = diagnosticCounts(notices);
  const pending = notices.some((notice) => notice.kind === "lsp-pending");
  const errors = notices.some((notice) => notice.errors > 0);
  const Icon = counts ? WarningCircle : pending ? Clock : Check;
  const label = counts ? t("row.lsp_counts_short", { counts }) : t(pending ? "row.lsp_pending_short" : "row.lsp_clean_short");
  return <span className={`row-lsp-label ${counts ? errors ? "row-lsp-errors" : "row-lsp-warnings" : "row-lsp-neutral"}`}>
    <Icon size={13} aria-hidden="true" />
    {label}
    {counts && pending && <Clock size={12} aria-label={t("row.lsp_pending_short")} />}
  </span>;
}
