/**
 * Notice bars §6.1.1.
 *
 * There are no change bars here any more: edits go to the working tree at once,
 * and confirmation is asked once, before the write (§8, §16.37). A bar with an
 * "Apply" button was a second confirmation of the same thing.
 *
 * Two of the five notice bars listed in §6.1.1 are here — "context is full" and
 * "LSP is not responding", the ones the interface can learn about — and one beyond
 * the list: about MCP servers. A server that waits for approval or failed to
 * connect would otherwise be visible nowhere until the person types `/mcp`
 * themselves, while the agent works without its tools. "File changed externally"
 * without a watcher on the tree, and "version available" without an updater, are
 * labels that would never appear, and a place where nobody would later remember
 * that they do not work.
 *
 * There is no "no API key" bar. A key is not required up front (§10): a
 * provider that wants one refuses the request, and the error names the key.
 * A warning ahead of that was wrong for every local server, which never asks.
 */

import { Warning, X } from "@phosphor-icons/react";
import { useState } from "react";

import type { Feed } from "../core/feed";
import { t } from "../i18n";
import { FULL_RATIO } from "./contextParts";
import { tokens } from "./format";

export function Notices({
  feed,
  onCompact,
  onOpenMcp,
  onOpenLsp,
}: {
  feed: Feed;
  onCompact: () => void;
  onOpenMcp: () => void;
  onOpenLsp: () => void;
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
  const loginNeeded = named("needsLogin");
  // The names are part of the key: closing the bar about one server must not
  // hide the next one that fails.
  const mcpBars = [
    { id: `mcp-approval:${unapproved.join(",")}`, names: unapproved, note: "notice.mcp_approval" },
    { id: `mcp-failed:${failed.join(",")}`, names: failed, note: "notice.mcp_failed" },
    { id: `mcp-login:${loginNeeded.join(",")}`, names: loginNeeded, note: "notice.mcp_login" },
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
          <button className="btn btn-small" onClick={onOpenLsp}>
            {t("notice.open")}
          </button>
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
