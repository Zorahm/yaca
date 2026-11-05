import { Plus } from "@phosphor-icons/react";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { addMarketplace, refreshMarketplace, removeMarketplace, type MarketplaceSummary } from "../../core/client";
import { locale, t, type Key } from "../../i18n";
import { PluginChip, PluginConfirm, PluginMenu, PluginOriginal } from "./PluginParts";
import { compactPluginPath, marketplaceSourceTarget, showMarketplaceSource } from "./pluginPresentation";
import { PluginCatalog } from "./PluginCatalog";

const ORIGIN: Record<MarketplaceSummary["origin"], Key> = {
  builtin: "mkt.origin_builtin", claudeCode: "mkt.origin_claude", own: "mkt.origin_own",
};

function updatedAt(time: number, now: number): string {
  const minutes = Math.max(0, Math.round((now / 1000 - time) / 60));
  const relative = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  return t("mkt.updated_at", { time: minutes < 60 ? relative.format(-minutes, "minute") : minutes < 1440
    ? relative.format(-Math.round(minutes / 60), "hour") : relative.format(-Math.round(minutes / 1440), "day") });
}

export function Marketplaces({ list, onChanged, onInstalled }: {
  list: MarketplaceSummary[]; onChanged: () => Promise<void>; onInstalled: () => void;
}) {
  const [adding, setAdding] = useState(false);
  const [source, setSource] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [now, setNow] = useState(Date.now());
  const [catalogRevision, setCatalogRevision] = useState(0);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 60000);
    return () => window.clearInterval(timer);
  }, []);

  const run = async (name: string, action: () => Promise<unknown>) => {
    if (busy !== null) return;
    setBusy(name); setErrors((prev) => { const next = { ...prev }; delete next[name]; return next; });
    try { await action(); await onChanged(); setNow(Date.now()); }
    catch (e) { setErrors((prev) => ({ ...prev, [name]: String(e) })); }
    finally { setBusy(null); }
  };
  const refresh = (name: string) => run(name, async () => {
    await refreshMarketplace(name);
    if (open === name) setCatalogRevision((n) => n + 1);
  });
  const add = () => void run("add", async () => {
    const added = await addMarketplace(source.trim());
    setSource(""); setAdding(false); setOpen(added.name);
  });

  return <section className="plugin-marketplaces">
    <div className="plugin-section-heading"><h2>{t("mkt.title")}</h2>
      <button type="button" className="btn btn-small" aria-expanded={adding} onClick={() => setAdding(!adding)}><Plus size={14} />{t("mkt.add_marketplace")}</button>
    </div>
    {adding && <form className="plugin-add" onSubmit={(e) => { e.preventDefault(); add(); }}>
      <label htmlFor="marketplace-source">{t("mkt.add_placeholder")}</label>
      <div className="plugin-actions"><input id="marketplace-source" className="prov-input prov-input-mono" autoFocus spellCheck={false} value={source} onChange={(e) => setSource(e.target.value)} />
        <button type="submit" className="btn btn-small" disabled={!source.trim() || busy !== null}>{busy === "add" ? t("mkt.loading") : t("mkt.add")}</button></div>
      {errors.add && <div className="set-error" role="alert">{errors.add}</div>}
    </form>}
    <div className="plugin-list">{list.map((m) => {
      const error = errors[m.name] ?? m.error;
      const target = marketplaceSourceTarget(m.source);
      const sourceLabel = target?.kind === "path" ? compactPluginPath(m.source) : m.source;
      return <article className="plugin-card marketplace-card" key={m.name} aria-label={m.name} aria-busy={busy === m.name}>
        <div className="marketplace-row">
          <div className="marketplace-info">
            <div className="plugin-identity"><h3>{m.name}</h3><PluginChip>{t(ORIGIN[m.origin])}</PluginChip></div>
            {showMarketplaceSource(m.name, m.source) && <code className="marketplace-source" title={m.source}>{sourceLabel}</code>}
          </div>
          <div className="marketplace-status">
            <PluginChip tone={error ? "error" : undefined}>{error ? t("mkt.failed") : m.fetched ? t("mkt.count", { count: m.plugins ?? 0 }) : t("mkt.not_fetched")}</PluginChip>
            {m.updatedAt !== null && !error && <small>{updatedAt(m.updatedAt, now)}</small>}
          </div>
          <div className="plugin-actions marketplace-actions">
            <button type="button" className="btn btn-small" disabled={busy !== null} aria-expanded={open === m.name}
              onClick={() => error || !m.fetched ? void refresh(m.name) : setOpen(open === m.name ? null : m.name)}>
              {busy === m.name ? t("mkt.loading") : error ? t("mkt.retry") : m.fetched ? t("mkt.open") : t("mkt.fetch")}
            </button>
            <PluginMenu label={t("plug.actions", { name: m.name })} disabled={busy !== null} items={[
              { label: t("plug.update_source"), onSelect: () => void refresh(m.name) },
              { label: t("mkt.open_source"), disabled: target === null, onSelect: () => void run(m.name, () => target?.kind === "url" ? openUrl(target.value) : openPath(target!.value)) },
              { label: t(m.origin === "builtin" ? "mkt.hide" : "mkt.remove"), onSelect: () => setConfirming(m.name) },
            ]} />
          </div>
        </div>
        {error && <div className="plugin-market-error"><PluginOriginal text={error} /></div>}
        {confirming === m.name && <PluginConfirm busy={busy !== null} label={t(m.origin === "builtin" ? "mkt.hide" : "mkt.remove")}
          text={t(m.origin === "claudeCode" ? "mkt.remove_claude_question" : "mkt.remove_question", { name: m.name })}
          onCancel={() => setConfirming(null)} onConfirm={() => void run(m.name, async () => {
            await removeMarketplace(m.name); setConfirming(null); if (open === m.name) setOpen(null);
          })} />}
      </article>;
    })}</div>
    {open !== null && <PluginCatalog key={`${open}:${catalogRevision}`} name={open} onInstalled={onInstalled} onClose={() => setOpen(null)} />}
  </section>;
}
