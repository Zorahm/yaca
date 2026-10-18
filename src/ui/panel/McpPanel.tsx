/**
 * The MCP panel, opened by `/mcp`: this chat's servers and the state of
 * each.
 *
 * The list is the core's, whole, from the last `mcpUpdated` — the panel keeps
 * none of its own. It also asks once when it opens: servers start connecting
 * with the chat, and a change that happened while nobody was listening would
 * otherwise never be drawn.
 *
 * What can be done here are commands to the core: connect again, approve a
 * server that came with the project's `.mcp.json`, and log in to (or out of)
 * a remote server that asks for a login. A login's page opens in the browser
 * by itself; the link stays in the panel for when it did not.
 * Declaring a server is not one of them — that is the settings window, and it
 * reaches the next chat, not this one.
 */

import { Circle } from "@phosphor-icons/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef } from "react";

import type { McpServerSummary, McpSource, McpStatus } from "../../core/client";
import { t, tp, type Key } from "../../i18n";
import { Panel } from "./Panel";

const STATUS: Record<McpStatus, Key> = {
  connecting: "mcp.status_connecting",
  ready: "mcp.status_ready",
  failed: "mcp.status_failed",
  disabled: "mcp.status_disabled",
  needsApproval: "mcp.status_needs_approval",
  needsLogin: "mcp.status_needs_login",
  loggingIn: "mcp.status_logging_in",
};

const SOURCE: Record<McpSource, Key> = {
  global: "mcp.source_global",
  projectFile: "mcp.source_project_file",
  plugin: "mcp.source_plugin",
  app: "mcp.source_app",
  project: "mcp.source_project",
};

export function McpPanel({
  servers,
  onRefresh,
  onReconnect,
  onApprove,
  onLogin,
  onLogout,
}: {
  /** `null` — the list has not arrived yet. */
  servers: McpServerSummary[] | null;
  onRefresh: () => void;
  onReconnect: (server: string) => void;
  onApprove: (server: string) => void;
  onLogin: (server: string) => void;
  onLogout: (server: string) => void;
}) {
  // Once per opening. `onRefresh` is not a dependency on purpose: it is a new
  // function on every render of the window, and the panel would ask in a loop.
  useEffect(() => {
    onRefresh();
  }, []);

  return (
    <Panel>
      {servers === null || servers.length === 0 ? (
        <div className="panel-empty">{t("mcp.panel_empty")}</div>
      ) : (
        <div className="panel-body">
          {servers.map((server) => (
            <Server
              key={server.name}
              server={server}
              onReconnect={() => onReconnect(server.name)}
              onApprove={() => onApprove(server.name)}
              onLogin={() => onLogin(server.name)}
              onLogout={() => onLogout(server.name)}
            />
          ))}
        </div>
      )}
    </Panel>
  );
}

function Server({
  server,
  onReconnect,
  onApprove,
  onLogin,
  onLogout,
}: {
  server: McpServerSummary;
  onReconnect: () => void;
  onApprove: () => void;
  onLogin: () => void;
  onLogout: () => void;
}) {
  const waiting = server.status === "needsApproval";
  // The login page opens once per login: the list arrives again on every
  // change, and the browser must not get a new tab each time.
  const opened = useRef<string | null>(null);
  useEffect(() => {
    const page = server.loginUrl;
    if (page !== null && opened.current !== page) {
      opened.current = page;
      void openUrl(page).catch(() => {});
    }
  }, [server.loginUrl]);
  return (
    <section className="mcp-server">
      <div className="mcp-head">
        <span className="mcp-name">{server.name}</span>
        <Status status={server.status} />
        <div className="composer-gap" />
        {server.status === "needsLogin" && (
          <button type="button" className="btn btn-small btn-accent" onClick={onLogin}>
            {t("mcp.login")}
          </button>
        )}
        {server.loggedIn && server.status === "ready" && (
          <button type="button" className="btn btn-small" onClick={onLogout}>
            {t("mcp.logout")}
          </button>
        )}
        {waiting ? (
          <button type="button" className="btn btn-small btn-accent" onClick={onApprove}>
            {t("mcp.approve")}
          </button>
        ) : server.status === "needsLogin" || server.status === "loggingIn" ? null : (
          server.status !== "disabled" &&
          server.status !== "connecting" && (
            <button type="button" className="btn btn-small" onClick={onReconnect}>
              {t("mcp.reconnect")}
            </button>
          )
        )}
      </div>

      {/* Never cut: for a server waiting to be approved this line is what is
          being approved — the program that will run. */}
      <div className="mcp-target">{server.target}</div>
      <div className="mcp-meta">
        {t(SOURCE[server.source])}
        {server.label !== null && ` · ${server.label}`}
      </div>

      {waiting && <div className="mcp-note">{t("mcp.approval_note")}</div>}
      {server.status === "loggingIn" && (
        <div className="mcp-note">
          {t("mcp.login_note")}{" "}
          {server.loginUrl !== null && (
            <button type="button" className="link" onClick={() => void openUrl(server.loginUrl!).catch(() => {})}>
              {t("mcp.login_open")}
            </button>
          )}
        </div>
      )}
      {server.reason !== null && <pre className="mcp-reason">{server.reason}</pre>}

      {server.status === "ready" && (
        <details className="mcp-tools">
          <summary>
            {tp("mcp.tools", server.tools.length)}
            {server.resources && ` · ${t("mcp.resources")}`}
          </summary>
          <ul>
            {server.tools.map((tool) => (
              <li key={tool.name}>
                <span className="mcp-tool-name">{tool.name}</span>
                <span className="mcp-tool-kind">
                  {t(tool.readOnly ? "mcp.tool_reads" : "mcp.tool_writes")}
                </span>
                {tool.description !== null && (
                  <span className="mcp-tool-about">{firstLine(tool.description)}</span>
                )}
              </li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}

function Status({ status }: { status: McpStatus }) {
  const tone =
    status === "ready" || status === "connecting" || status === "loggingIn"
      ? "mcp-status-live"
      : status === "failed"
        ? "mcp-status-fail"
        : "";
  return (
    <span className={`mcp-status ${tone}`}>
      {(status === "connecting" || status === "loggingIn") && (
        <span className="pulse">
          <Circle size={7} weight="fill" />
        </span>
      )}{" "}
      {t(STATUS[status])}
    </span>
  );
}

function firstLine(text: string): string {
  return text.split("\n")[0] ?? "";
}
