import { ArrowsClockwise, FolderOpen, Plus } from "@phosphor-icons/react";
import { useCallback, useEffect, useState } from "react";
import {
  listPlugins, listMarketplaces, marketplaceCatalog, openPluginsFolder,
  setPluginEnabled, uninstallPlugin, updatePlugin, preferPluginCopy, ignorePluginDuplicates,
  type PluginReport, type MarketplaceSummary,
} from "../../core/client";
import { t } from "../../i18n";
import { Marketplaces } from "./Marketplaces";
import { PluginCard } from "./PluginCard";
import { PluginOriginal } from "./PluginParts";
import { compactPluginPath, newerPluginVersion } from "./pluginPresentation";
import { Segments } from "./Row";
import "./plugins.css";

export function PluginsSection({ onNewChat }: { onNewChat?: () => void }) {
  const [report, setReport] = useState<PluginReport | null>(null);
  const [markets, setMarkets] = useState<MarketplaceSummary[]>([]);
  const [versions, setVersions] = useState<Record<string, string>>({});
  const [tab, setTab] = useState<"installed" | "markets">("installed");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [pendingChat, setPendingChat] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  const reloadMarkets = useCallback(async () => {
    const next = await listMarketplaces();
    setMarkets(next);
    // Read cached catalogs only: opening settings must not clone repositories.
    const catalogs = await Promise.allSettled(next.filter((m) => m.fetched && !m.error).map((m) => marketplaceCatalog(m.name)));
    const available: Record<string, string> = {};
    for (const result of catalogs) if (result.status === "fulfilled") {
      for (const entry of result.value) if (entry.version) available[entry.id] = entry.version;
    }
    setVersions(available);
  }, []);

  const reload = useCallback(async () => {
    const [plugins, marketplaces] = await Promise.allSettled([listPlugins(), reloadMarkets()]);
    if (plugins.status === "fulfilled") setReport(plugins.value);
    else setError(String(plugins.reason));
    if (marketplaces.status === "rejected") setError(String(marketplaces.reason));
  }, [reloadMarkets]);
  useEffect(() => { void reload(); }, [reload]);
  useEffect(() => {
    if (!pendingChat) return;
    const timer = window.setTimeout(() => setPendingChat(false), 12000);
    return () => window.clearTimeout(timer);
  }, [pendingChat, report]);

  const run = async (id: string, action: () => Promise<unknown>, changed = false) => {
    if (busy !== null) return;
    setBusy(id); setError(null); setNotice(null);
    try {
      await action();
      if (changed) setPendingChat(true);
    } catch (e) { setError(String(e)); }
    finally { await reload(); setBusy(null); }
  };

  const update = async (id: string) => {
    const outcome = await updatePlugin(id);
    setNotice(outcome.changed ? t("plug.updated", { id, from: outcome.from, to: outcome.to }) : t("plug.up_to_date", { id }));
  };
  const managed = report?.plugins.filter((p) => p.origin === "marketplace") ?? [];

  return <div className="plugins-page">
    <div className="plugin-page-heading">
      <h1 className="set-title">{t("plug.title")}</h1>
      <div className="plugin-actions">
        <button type="button" className="btn btn-small btn-accent" onClick={() => setTab("markets")}><Plus size={14} />{t("mkt.install")}</button>
        <button type="button" className="btn btn-small" onClick={() => void run("folder", openPluginsFolder)} disabled={busy !== null}><FolderOpen size={14} />{t("plug.folder")}</button>
        <button type="button" className="btn btn-small" disabled={busy !== null || managed.length === 0}
          title={t("plug.update_source")} onClick={() => void run("all", async () => {
            const results: string[] = [];
            for (const p of managed) {
              try { await update(p.id); } catch (e) { results.push(`${p.name}: ${String(e)}`); }
            }
            setNotice(t("plug.update_all_done"));
            if (results.length) throw new Error(results.join("\n"));
          }, true)}><ArrowsClockwise size={14} className={busy === "all" ? "spin" : undefined} />{t("plug.update_all")}</button>
      </div>
    </div>
    <p className="plugin-lede">{t("plug.lede")}</p>
    <Segments label={t("plug.title")} value={tab} onChange={setTab} options={[
      { value: "installed", label: `${t("plug.installed_tab")} · ${report?.plugins.length ?? "…"}` },
      { value: "markets", label: `${t("mkt.title")} · ${markets.length}` },
    ]} />
    {error && <div className="set-error" role="alert">{error}</div>}
    {notice && <div className="plugin-notice" role="status">{notice}</div>}
    {pendingChat && <div className="plugin-toast" role="status">
      <span>{t("plug.next_chat")}</span>
      {onNewChat && <button type="button" className="btn btn-small" onClick={onNewChat}>{t("plug.new_chat")}</button>}
      <button type="button" className="link" onClick={() => setPendingChat(false)}>{t("common.close")}</button>
    </div>}
    {tab === "installed" && <>
      {report === null && !error && <div className="set-empty">{t("mkt.loading")}</div>}
      {report?.plugins.length === 0 && <div className="set-empty">{t("plug.none")}</div>}
      {!!report?.diagnostics.length && <div className="plugin-errors" role="alert">
        <strong>{t("plug.attention", { count: report.diagnostics.length })}</strong>
        {report.diagnostics.map((d, i) => <div key={`${d.path}-${i}`}>
          <p>{t("plug.load_failed")}</p><code title={d.path}>{compactPluginPath(d.path)}</code>
          <PluginOriginal text={`${d.message}\n${d.path}`} />
        </div>)}
      </div>}
      <div className="plugin-list">
        {report?.plugins.map((p) => <PluginCard key={p.id} plugin={p} busy={busy !== null}
          nextVersion={newerPluginVersion(p.version, versions[p.id] ?? null) ? versions[p.id] : null}
          onToggle={(on) => void run(p.id, async () => { setReport(await setPluginEnabled(p.id, on)); }, true)}
          onUpdate={() => void run(p.id, () => update(p.id), true)}
          onRemove={() => void run(p.id, () => uninstallPlugin(p.id), true)}
          onPrefer={(path) => void run(p.id, async () => { setReport(await preferPluginCopy(p.name, path)); }, true)}
          onIgnore={(ignored) => void run(p.id, async () => { setReport(await ignorePluginDuplicates(p.name, ignored)); })} />)}
      </div>
    </>}
    {tab === "markets" && <Marketplaces list={markets} onChanged={reloadMarkets} onInstalled={() => { setPendingChat(true); void reload(); }} />}
  </div>;
}
