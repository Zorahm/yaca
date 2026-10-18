/**
 * Plugins in the settings window: what a new chat loads, the switches, and
 * the marketplaces new ones come from.
 *
 * Read from the core on every opening of the section — Yaca's own plugin
 * folder, what Yaca installed, what Claude Code installed — so a plugin
 * installed a minute ago is here. A plugin's files are not edited here: they
 * are someone else's, and the place to change them is not this window. What
 * Yaca installed it can update and remove; the rest only switch.
 *
 * A switch, an install or a removal reaches the chats opened after it. An open
 * chat keeps the plugins it started with, as it keeps its MCP servers.
 */

import { ArrowsClockwise, X } from "@phosphor-icons/react";
import { useCallback, useEffect, useState } from "react";

import {
  listPlugins,
  setPluginEnabled,
  uninstallPlugin,
  updatePlugin,
  type PluginReport,
  type PluginSummary,
} from "../../core/client";
import { t, type Key } from "../../i18n";
import { Marketplaces } from "./Marketplaces";
import { Switch } from "./Row";

const EMPTY: PluginReport = { plugins: [], diagnostics: [] };

const ORIGIN: Record<PluginSummary["origin"], Key> = {
  yaca: "plug.origin_yaca",
  marketplace: "plug.origin_yaca",
  claudeCode: "plug.origin_claude",
};

export function PluginsSection() {
  const [report, setReport] = useState<PluginReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  const reload = useCallback(() => {
    void listPlugins().then(setReport, (e: unknown) => {
      setReport(EMPTY);
      setError(String(e));
    });
  }, []);
  useEffect(reload, [reload]);

  const toggle = (id: string, on: boolean) => {
    void setPluginEnabled(id, on).then(
      (next) => {
        setError(null);
        setReport(next);
      },
      (e: unknown) => setError(String(e)),
    );
  };

  const update = (id: string) => {
    setBusy(id);
    setError(null);
    void updatePlugin(id)
      .then((o) => {
        setNotice(o.changed ? t("plug.updated", { id, from: o.from, to: o.to }) : t("plug.up_to_date", { id }));
        reload();
      }, (e: unknown) => setError(String(e)))
      .finally(() => setBusy(null));
  };

  const remove = (id: string) => {
    setBusy(id);
    setError(null);
    void uninstallPlugin(id)
      .then(reload, (e: unknown) => setError(String(e)))
      .finally(() => setBusy(null));
  };

  return (
    <>
      <div className="set-head">
        <div>
          <h1 className="set-title">{t("plug.title")}</h1>
          <p className="set-lede">{t("plug.lede")}</p>
        </div>
      </div>

      {error !== null && <div className="set-error">{error}</div>}
      {notice !== null && <div className="set-empty">{notice}</div>}

      {report !== null && report.plugins.length === 0 && (
        <div className="set-empty">{t("plug.none")}</div>
      )}

      {report !== null && report.plugins.length > 0 && (
        <div className="prov-list">
          {report.plugins.map((p) => (
            <div className="prov plug" key={p.id} title={p.path}>
              <div className="prov-main">
                <span className="prov-name">
                  {p.name}
                  {p.version !== null && <span className="plug-version"> {p.version}</span>}
                </span>
                {p.description !== null && <span className="plug-desc">{p.description}</span>}
                <span className="plug-note">
                  {t("plug.counts", {
                    skills: p.skills,
                    commands: p.commands,
                    mcp: p.mcpServers,
                    hooks: p.hooks,
                  })}
                </span>
                {p.notes.map((note) => (
                  <span key={note} className="plug-note">
                    {note}
                  </span>
                ))}
              </div>

              <span className="prov-proto">{t(ORIGIN[p.origin])}</span>

              <div className="prov-actions">
                <Switch on={p.enabled} label={t("plug.enabled", { name: p.id })} onChange={(on) => toggle(p.id, on)} />
                {p.origin === "marketplace" && (
                  <>
                    <button
                      type="button"
                      className="btn btn-row"
                      aria-label={t("plug.update")}
                      title={t("plug.update")}
                      disabled={busy !== null}
                      onClick={() => update(p.id)}
                    >
                      <ArrowsClockwise size={14} className={busy === p.id ? "spin" : undefined} />
                    </button>
                    {confirming === p.id ? (
                      <button
                        type="button"
                        className="btn btn-row btn-danger"
                        onClick={() => {
                          setConfirming(null);
                          remove(p.id);
                        }}
                      >
                        {t("plug.remove_confirm")}
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="btn btn-row btn-danger"
                        aria-label={t("plug.remove")}
                        title={t("plug.remove")}
                        disabled={busy !== null}
                        onClick={() => setConfirming(p.id)}
                        onBlur={() => setConfirming(null)}
                      >
                        <X size={14} />
                      </button>
                    )}
                  </>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      {report !== null && report.diagnostics.length > 0 && (
        <>
          <div className="set-subhead set-subhead-sep">
            <div>
              <span className="set-subname">{t("plug.problems")}</span>
            </div>
          </div>
          <div className="set-list">
            {report.diagnostics.map((d, at) => (
              <div key={`${d.path}-${at}`} className="diag">
                <span className="diag-message">{d.message}</span>
                <span className="diag-path">{d.path}</span>
              </div>
            ))}
          </div>
        </>
      )}

      <Marketplaces onInstalled={reload} />
    </>
  );
}
