import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { marketplaceCatalog, previewPlugin, installReviewedPlugin, discardPluginPreview, type CatalogEntry } from "../../core/client";
import type { PluginPreview } from "../../core/bindings/PluginPreview";
import { t } from "../../i18n";
import { PluginChip, PluginContentsView, PluginCounts, PluginDescription, PluginOriginal } from "./PluginParts";

export function PluginCatalog({ name, onInstalled, onClose }: { name: string; onInstalled: () => void; onClose: () => void }) {
  const [entries, setEntries] = useState<CatalogEntry[] | null>(null);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [review, setReview] = useState<{ id: string; preview: PluginPreview } | null>(null);
  const mounted = useRef(true);
  const token = useRef<string | null>(null);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; if (token.current) void discardPluginPreview(token.current).catch(() => undefined); };
  }, []);

  const load = useCallback(async () => {
    const list = await marketplaceCatalog(name);
    if (mounted.current) setEntries(list);
  }, [name]);
  useEffect(() => { void load().catch((e: unknown) => setError(String(e))); }, [load]);
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return entries?.filter((e) => `${e.name} ${e.description ?? ""}`.toLowerCase().includes(q));
  }, [entries, query]);

  const inspect = async (id: string) => {
    if (busy) return;
    setBusy(true); setError(null);
    try {
      if (token.current) await discardPluginPreview(token.current);
      token.current = null; setReview(null);
      const preview = await previewPlugin(id);
      if (!mounted.current) { await discardPluginPreview(preview.token); return; }
      token.current = preview.token; setReview({ id, preview });
    } catch (e) { if (mounted.current) setError(String(e)); }
    finally { if (mounted.current) setBusy(false); }
  };
  const cancel = async () => {
    if (busy || !token.current) return;
    setBusy(true);
    try { await discardPluginPreview(token.current); token.current = null; setReview(null); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  const install = async () => {
    if (busy || !token.current) return;
    setBusy(true); setError(null);
    // Once submitted, the installer owns cleanup. Leaving the tab must not
    // delete its staging directory while the core is moving the files.
    const installingToken = token.current;
    token.current = null;
    try {
      await installReviewedPlugin(installingToken);
      if (mounted.current) { setReview(null); await load(); }
      onInstalled();
    } catch (e) {
      // A consumed or failed review cannot be retried without inspecting again.
      if (mounted.current) { setReview(null); setError(String(e)); }
    } finally { if (mounted.current) setBusy(false); }
  };

  return <section className="plugin-catalog" aria-busy={busy}>
    <div className="plugin-section-heading"><h2>{t("mkt.catalog", { name })}</h2>
      <button type="button" className="btn btn-small" disabled={busy} onClick={onClose}>{t("common.close")}</button></div>
    <input className="prov-input" value={query} spellCheck={false} aria-label={t("mkt.search")} placeholder={t("mkt.search")} onChange={(e) => setQuery(e.target.value)} />
    {error && <div className="set-error" role="alert">{error}</div>}
    {!entries && !error && <p className="set-empty">{t("mkt.loading")}</p>}
    {shown?.length === 0 && <p className="set-empty">{t("mkt.empty")}</p>}
    <div className="plugin-list">{shown?.map((entry) => {
      const preview = review?.id === entry.id ? review.preview : null;
      return <article className="plugin-card" key={entry.id}>
        <div className="plugin-card-header"><div className="plugin-identity"><h3>{entry.name}</h3>
          {entry.version && <span className="plugin-version">{entry.version}</span>}
          {entry.installed !== null && <PluginChip tone="accent">{t("mkt.installed", { version: entry.installed })}</PluginChip>}
        </div>
          {entry.installed === null && <button type="button" className="btn btn-small" disabled={busy || entry.unavailable !== null} onClick={() => void inspect(entry.id)}>
            {t("mkt.install")}
          </button>}
        </div>
        <PluginDescription text={entry.description} />
        {entry.unavailable && <PluginOriginal text={entry.unavailable} />}
        <div className="plugin-catalog-footer">
          {preview && <PluginCounts contents={preview.contents} />}
          <button type="button" className="link" aria-expanded={preview !== null} disabled={busy || entry.unavailable !== null}
            onClick={() => void (preview ? cancel() : inspect(entry.id))}><span aria-hidden="true">{preview ? "▾" : "▸"} </span>{t("plug.contents")}</button>
        </div>
        {preview && <div className="plugin-review">
          {(preview.contents.hooks.length > 0 || preview.contents.mcpServers.length > 0) && <div className="plugin-warning"><strong>{t("plug.launch_warning")}</strong></div>}
          <PluginContentsView contents={preview.contents} />
          {preview.notes.length > 0 && <div className="plugin-warning"><p>{t("plug.component_notes")}</p><PluginOriginal text={preview.notes.join("\n")} /></div>}
          <div className="plugin-actions">
            {entry.installed === null && <button type="button" className="btn btn-small btn-accent" disabled={busy} onClick={() => void install()}>{t("plug.confirm_install")}</button>}
            <button type="button" className="btn btn-small" disabled={busy} onClick={() => void cancel()}>{t("plug.cancel")}</button>
          </div>
        </div>}
      </article>;
    })}</div>
    {busy && <p className="plugin-notice" role="status">{t("mkt.loading")}</p>}
  </section>;
}
