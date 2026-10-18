/**
 * The language-server panel, opened by `/lsp`: this chat's servers and the
 * state of each.
 *
 * The list is the core's, whole, from the last `lspUpdated` — the panel keeps
 * none of its own, and needs no refresh: the list arrives with the chat and
 * again on every change. A server starts when a file first needs it, so an
 * idle one is normal, not a fault.
 *
 * One thing can be done here: restart a server. It is a command to the core,
 * which stops the process and starts it again; the panel shows it go down and
 * come back up through the same list. Declaring a server is the settings
 * window's.
 */

import { Circle } from "@phosphor-icons/react";

import type { LspServerSummary, LspStatus } from "../../core/client";
import { t, type Key } from "../../i18n";
import { Panel } from "./Panel";

const STATUS: Record<LspStatus, Key> = {
  idle: "lsp.status_idle",
  starting: "lsp.status_starting",
  ready: "lsp.status_ready",
  failed: "lsp.status_failed",
  disabled: "lsp.status_disabled",
};

export function LspPanel({
  servers,
  onRestart,
}: {
  /** `null` — the list has not arrived yet. */
  servers: LspServerSummary[] | null;
  onRestart: (server: string) => void;
}) {
  return (
    <Panel>
      {servers === null || servers.length === 0 ? (
        <div className="panel-empty">{t("lsp.panel_empty")}</div>
      ) : (
        <div className="panel-body">
          {servers.map((server) => (
            <Server key={server.name} server={server} onRestart={() => onRestart(server.name)} />
          ))}
        </div>
      )}
    </Panel>
  );
}

function Server({ server, onRestart }: { server: LspServerSummary; onRestart: () => void }) {
  const canRestart = server.status !== "disabled" && server.status !== "starting";
  return (
    <section className="mcp-server">
      <div className="mcp-head">
        <span className="mcp-name">{server.name}</span>
        <Status status={server.status} />
        <div className="composer-gap" />
        {canRestart && (
          <button type="button" className="btn btn-small" onClick={onRestart}>
            {t(server.status === "idle" ? "lsp.start" : "lsp.restart")}
          </button>
        )}
      </div>
      <div className="mcp-target">{server.command}</div>
      <div className="mcp-meta">
        {t("lsp.extensions", { list: server.extensions.map((e) => `.${e}`).join(" ") })}
        {server.label !== null && ` · ${server.label}`}
      </div>
      {server.reason !== null && <pre className="mcp-reason">{server.reason}</pre>}
    </section>
  );
}

function Status({ status }: { status: LspStatus }) {
  const tone =
    status === "ready" || status === "starting"
      ? "mcp-status-live"
      : status === "failed"
        ? "mcp-status-fail"
        : "";
  return (
    <span className={`mcp-status ${tone}`}>
      {status === "starting" && (
        <span className="pulse">
          <Circle size={7} weight="fill" />
        </span>
      )}{" "}
      {t(STATUS[status])}
    </span>
  );
}
