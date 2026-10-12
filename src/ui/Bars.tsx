/**
 * Полосы уведомлений §6.1.1.
 *
 * Полосы изменений здесь больше нет: правки уезжают в рабочее дерево сразу, а
 * подтверждение спрашивается один раз, до записи (§8, §16.37). Полоса с
 * кнопкой «Применить» была вторым подтверждением того же самого.
 *
 * Полос уведомлений здесь две из пяти, перечисленных в §6.1.1, — «контекст
 * заполнен» и «LSP не отвечает», те, про которые интерфейсу есть откуда узнать,
 * — и одна сверх списка: про серверы MCP. Сервер, который ждёт одобрения или
 * не подключился, иначе не был бы виден нигде, пока человек сам не наберёт
 * `/mcp`, а агент тем временем работает без его инструментов. «Файл изменён
 * извне» без наблюдателя за деревом и «доступна версия» без обновлятора — это
 * надписи, которые никогда не появятся, и место, где потом никто не вспомнит,
 * что они не работают.
 *
 * There is no "no API key" bar. A key is not required up front (§10): a
 * provider that wants one refuses the request, and the error names the key.
 * A warning ahead of that was wrong for every local server, which never asks.
 */

import { Warning, X } from "@phosphor-icons/react";
import { useState } from "react";

import type { Feed } from "../core/feed";
import { t } from "../i18n";
import { tokens } from "./format";

/** §6.1.1: полоса появляется, когда контекст занят на 80%. */
const FULL_RATIO = 0.8;

export function Notices({
  feed,
  onCompact,
  onOpenMcp,
}: {
  feed: Feed;
  onCompact: () => void;
  onOpenMcp: () => void;
}) {
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());
  const hide = (id: string) => setDismissed(new Set(dismissed).add(id));

  const usage = feed.usage;
  const full =
    usage !== null &&
    usage.contextMax > 0 &&
    usage.contextUsed / usage.contextMax >= FULL_RATIO;

  const servers = feed.mcp ?? [];
  const named = (status: string) =>
    servers.filter((server) => server.status === status).map((server) => server.name);
  const unapproved = named("needsApproval");
  const failed = named("failed");
  // The names are part of the key: closing the bar about one server must not
  // hide the next one that fails.
  const mcpBars = [
    { id: `mcp-approval:${unapproved.join(",")}`, names: unapproved, note: "notice.mcp_approval" },
    { id: `mcp-failed:${failed.join(",")}`, names: failed, note: "notice.mcp_failed" },
  ] as const;

  // A server that was never asked for is not a problem, and one that is
  // switched off is the human's own doing: only a server that went wrong
  // gets a bar. The agent works without it, as it does without an MCP one.
  const lspFailed = (feed.lsp ?? [])
    .filter((server) => server.status === "failed")
    .map((server) => server.name);
  const lspBar = {
    id: `lsp-failed:${lspFailed.join(",")}`,
    names: lspFailed,
    note: "notice.lsp_failed",
  } as const;

  return (
    <>
      {lspBar.names.length > 0 && !dismissed.has(lspBar.id) && (
        <div className="bar" key={lspBar.id}>
          <span className="bar-icon"><Warning size={15} /></span>
          <span className="bar-note">{t(lspBar.note, { names: lspBar.names.join(", ") })}</span>
          <button
            className="bar-close"
            onClick={() => hide(lspBar.id)}
            aria-label={t("notice.dismiss")}
          >
            <X size={14} />
          </button>
        </div>
      )}
      {mcpBars.map(
        (bar) =>
          bar.names.length > 0 &&
          !dismissed.has(bar.id) && (
            <div className="bar" key={bar.id}>
              <span className="bar-icon"><Warning size={15} /></span>
              <span className="bar-note">{t(bar.note, { names: bar.names.join(", ") })}</span>
              <button className="btn btn-small" onClick={onOpenMcp}>
                {t("notice.open")}
              </button>
              <button
                className="bar-close"
                onClick={() => hide(bar.id)}
                aria-label={t("notice.dismiss")}
              >
                <X size={14} />
              </button>
            </div>
          ),
      )}
      {full && !dismissed.has("context") && (
        <div className="bar">
          <span className="bar-icon"><Warning size={15} /></span>
          <span className="bar-note">
            {t("notice.context_full")} · {tokens(usage.contextUsed)}/
            {tokens(usage.contextMax)}
          </span>
          <button className="btn btn-small" onClick={onCompact}>
            {t("notice.compact")}
          </button>
          <button
            className="bar-close"
            onClick={() => hide("context")}
            aria-label={t("notice.dismiss")}
          >
            <X size={14} />
          </button>
        </div>
      )}
    </>
  );
}
