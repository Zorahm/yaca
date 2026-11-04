/** Project language servers. Status and switches come from the core. */
import { Circle, DownloadSimple, Power, ArrowClockwise } from "@phosphor-icons/react";
import type { LspServerSummary, LspStatus } from "../../core/client";
import { t, type Key } from "../../i18n";
import { Panel } from "./Panel";
import "./lsp.css";

const STATUS: Record<LspStatus, Key> = {
  idle: "lsp.status_idle", starting: "lsp.status_starting", ready: "lsp.status_ready",
  failed: "lsp.status_failed", disabled: "lsp.status_disabled",
  missing: "lsp.status_missing", installing: "lsp.status_installing",
};

export function LspPanel({ servers, onRestart, onEnable, onInstall }: {
  servers: LspServerSummary[] | null;
  onRestart: (server: string) => void;
  onEnable: (server: string, enabled: boolean) => void;
  onInstall: (server: string) => void;
}) {
  return <Panel>
    <div className="lsp-list">
      <p className="lsp-note">{t("lsp.autostart")}</p>
      {servers === null ? <div className="lsp-note">{t("lsp.loading")}</div>
        : servers.length === 0 ? <div className="lsp-note">{t("lsp.panel_empty")}</div>
        : servers.map((server) => <Server key={server.name} server={server}
          onRestart={() => onRestart(server.name)} onEnable={(enabled) => onEnable(server.name, enabled)}
          onInstall={() => onInstall(server.name)} />)}
    </div>
  </Panel>;
}

function Server({ server, onRestart, onEnable, onInstall }: {
  server: LspServerSummary; onRestart: () => void;
  onEnable: (enabled: boolean) => void; onInstall: () => void;
}) {
  const disabled = server.status === "disabled";
  const busy = server.status === "starting" || server.status === "installing";
  const canInstall = (server.status === "missing" || server.status === "failed") && server.installation;
  const tone = server.status === "ready" ? "lsp-status-ready" : server.status === "failed" ? "lsp-status-failed" : busy ? "lsp-status-busy" : "";
  return <article className="lsp-server">
    <div className="lsp-info">
      <div className="lsp-heading">
        <h3 className="lsp-name">{server.name}</h3>
        <span className={`lsp-status ${tone}`} role="status">
          {busy && <span className="pulse"><Circle size={7} weight="fill" /></span>}
          {t(STATUS[server.status])}
        </span>
      </div>
      <div className="lsp-extensions">{server.extensions.map((ext) => <code key={ext}>.{ext}</code>)}</div>
      {server.label && <div className="lsp-note">{server.label}</div>}
      <code className="lsp-command">{server.command}</code>
      {server.reason && <pre className="lsp-reason">{server.reason}</pre>}
      {canInstall && <div className="lsp-install">
        <p className="lsp-note">{t("lsp.install_note")}</p>
        <code className="lsp-command">{server.installation}</code>
      </div>}
    </div>
    <div className="lsp-actions">
      {canInstall ? <button className="btn btn-small btn-accent" onClick={onInstall}>
        <DownloadSimple size={14} />{t("lsp.install")}
      </button> : !disabled && !busy && <button className="btn btn-small" onClick={onRestart}>
        <ArrowClockwise size={14} />{t(server.status === "idle" ? "lsp.start" : "lsp.restart")}
      </button>}
      <button className="btn btn-small lsp-switch" onClick={() => onEnable(disabled)}>
        <Power size={14} />{t(disabled ? "lsp.enable" : "lsp.disable")}
      </button>
    </div>
  </article>;
}
