import { Check, Clock, WarningCircle } from "@phosphor-icons/react";
import { t, tp } from "../i18n";
import type { ActionNotice } from "./editNotices";

export function FileLspBadge({ files }: { files: ActionNotice[][] }) {
  const notices = files.flat();
  const errors = notices.reduce((sum, notice) => sum + notice.errors, 0);
  const warnings = notices.reduce((sum, notice) => sum + notice.warnings, 0);
  const pending = files.filter((file) => file.some((notice) => notice.kind === "lsp-pending")).length;
  const checked = files.filter((file) => file.some((notice) => notice.kind === "lsp-clean")).length;
  if (errors || warnings) {
    const counts = errors ? tp("row.lsp_error_count", errors) : tp("row.lsp_warning_count_full", warnings);
    return <Badge icon={WarningCircle} tone={errors ? "errors" : "warnings"} label={t("row.lsp_counts_short", { counts })} />;
  }
  if (pending) return <Badge icon={Clock} tone="neutral" label={tp("file_group.pending", pending)} />;
  if (checked) return <Badge icon={Check} tone="neutral" label={t("file_group.checked", { n: checked })} />;
  return null;
}

export function diagnosticCounts(notices: ActionNotice[], full = false): string {
  const errors = notices.reduce((sum, notice) => sum + notice.errors, 0);
  const warnings = notices.reduce((sum, notice) => sum + notice.warnings, 0);
  return [errors > 0 && tp("row.lsp_error_count", errors),
    warnings > 0 && tp(full ? "row.lsp_warning_count_full" : "row.lsp_warning_count", warnings),
  ].filter(Boolean).join(", ");
}

export function LspBadge({ notices, compactClean = false }: { notices: ActionNotice[]; compactClean?: boolean }) {
  if (notices.length === 0) return null;
  const counts = diagnosticCounts(notices);
  const pending = notices.some((notice) => notice.kind === "lsp-pending");
  const errors = notices.some((notice) => notice.errors > 0);
  const Icon = counts ? WarningCircle : pending ? Clock : Check;
  const label = counts ? t("row.lsp_counts_short", { counts }) : t(pending ? "row.lsp_pending_short" : "row.lsp_clean_short");
  if (compactClean && !counts && !pending) {
    return <span className="row-lsp-check" role="img" aria-label={label} title={label}>
      <Check size={13} aria-hidden="true" />
    </span>;
  }
  return <Badge icon={Icon} tone={counts ? errors ? "errors" : "warnings" : "neutral"} label={label}>
    {counts && pending && <Clock size={12} aria-label={t("row.lsp_pending_short")} />}
  </Badge>;
}

function Badge({ icon: Icon, tone, label, children }: {
  icon: typeof Check;
  tone: "errors" | "warnings" | "neutral";
  label: string;
  children?: React.ReactNode;
}) {
  return <span className={`row-lsp-label row-lsp-${tone}`}>
    <Icon size={13} aria-hidden="true" />
    {label}
    {children}
  </span>;
}
