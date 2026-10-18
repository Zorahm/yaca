/**
 * Marketplaces and their catalogs, under the installed plugins.
 *
 * Nothing here is fetched until it is asked for: the list is names and
 * addresses, and opening a marketplace's catalog, or "Update", is what clones
 * it. A clone takes as long as the network does, so every action shows that
 * it is under way and says what went wrong in the core's own words.
 */

import { ArrowsClockwise, Plus, X } from "@phosphor-icons/react";
import { useCallback, useEffect, useMemo, useState } from "react";

import {
  addMarketplace,
  installPlugin,
  listMarketplaces,
  marketplaceCatalog,
  refreshMarketplace,
  removeMarketplace,
  type CatalogEntry,
  type MarketplaceSummary,
} from "../../core/client";
import { t, type Key } from "../../i18n";

const ORIGIN: Record<MarketplaceSummary["origin"], Key> = {
  builtin: "mkt.origin_builtin",
  claudeCode: "mkt.origin_claude",
  own: "mkt.origin_own",
};

export function Marketplaces({ onInstalled }: { onInstalled: () => void }) {
  const [list, setList] = useState<MarketplaceSummary[]>([]);
  const [adding, setAdding] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    void listMarketplaces().then(setList, (e: unknown) => setError(String(e)));
  }, []);
  useEffect(reload, [reload]);

  /** One action at a time, named by what it is about, with its error shown. */
  const run = (what: string, action: () => Promise<unknown>) => {
    setBusy(what);
    setError(null);
    void action()
      .then(reload, (e: unknown) => setError(String(e)))
      .finally(() => setBusy(null));
  };

  const add = () => {
    const source = adding.trim();
    if (source === "") return;
    run("add", () => addMarketplace(source).then(() => setAdding("")));
  };

  return (
    <>
      <div className="set-subhead set-subhead-sep">
        <div>
          <span className="set-subname">{t("mkt.title")}</span>
          <span className="set-subhint">{t("mkt.hint")}</span>
        </div>
      </div>

      {error !== null && <div className="set-error">{error}</div>}

      <div className="prov-list">
        {list.map((m) => (
          <div className="prov plug" key={m.name}>
            <div className="prov-main">
              <span className="prov-name">{m.name}</span>
              <span className="prov-meta">{m.source}</span>
              <span className="plug-note">
                {m.error ?? (m.plugins !== null ? t("mkt.count", { count: m.plugins }) : t("mkt.not_fetched"))}
              </span>
            </div>
            <span className="prov-proto">{t(ORIGIN[m.origin])}</span>
            <div className="prov-actions">
              <button
                type="button"
                className="btn btn-small"
                aria-pressed={open === m.name}
                onClick={() => setOpen(open === m.name ? null : m.name)}
              >
                {t("mkt.open")}
              </button>
              <button
                type="button"
                className="btn btn-row"
                aria-label={t("mkt.refresh")}
                title={t("mkt.refresh")}
                disabled={busy !== null}
                onClick={() => run(`refresh:${m.name}`, () => refreshMarketplace(m.name))}
              >
                <ArrowsClockwise size={14} className={busy === `refresh:${m.name}` ? "spin" : undefined} />
              </button>
              {confirming === m.name ? (
                <button
                  type="button"
                  className="btn btn-row btn-danger"
                  onClick={() => {
                    setConfirming(null);
                    if (open === m.name) setOpen(null);
                    run(`remove:${m.name}`, () => removeMarketplace(m.name));
                  }}
                >
                  {t("plug.remove_confirm")}
                </button>
              ) : (
                <button
                  type="button"
                  className="btn btn-row btn-danger"
                  aria-label={t("mkt.remove")}
                  title={t("mkt.remove")}
                  onClick={() => setConfirming(m.name)}
                  onBlur={() => setConfirming(null)}
                >
                  <X size={14} />
                </button>
              )}
            </div>
          </div>
        ))}
      </div>

      <div className="rule-add">
        <input
          className="prov-input prov-input-mono"
          value={adding}
          spellCheck={false}
          aria-label={t("mkt.add")}
          placeholder={t("mkt.add_placeholder")}
          onChange={(e) => setAdding(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
        <button type="button" className="btn btn-small" disabled={adding.trim() === "" || busy !== null} onClick={add}>
          <Plus size={13} /> {t("mkt.add")}
        </button>
      </div>

      {open !== null && (
        <Catalog
          key={open}
          name={open}
          onInstalled={() => {
            reload();
            onInstalled();
          }}
        />
      )}
    </>
  );
}

function Catalog({ name, onInstalled }: { name: string; onInstalled: () => void }) {
  const [entries, setEntries] = useState<CatalogEntry[] | null>(null);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    void marketplaceCatalog(name).then(setEntries, (e: unknown) => {
      setEntries([]);
      setError(String(e));
    });
  }, [name]);
  useEffect(load, [load]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (entries === null || q === "") return entries;
    return entries.filter(
      (e) => e.name.toLowerCase().includes(q) || (e.description ?? "").toLowerCase().includes(q),
    );
  }, [entries, query]);

  const install = (id: string) => {
    setBusy(id);
    setError(null);
    void installPlugin(id)
      .then(() => {
        load();
        onInstalled();
      }, (e: unknown) => setError(String(e)))
      .finally(() => setBusy(null));
  };

  return (
    <>
      <div className="set-subhead set-subhead-sep">
        <div>
          <span className="set-subname">{t("mkt.catalog", { name })}</span>
        </div>
      </div>
      {error !== null && <div className="set-error">{error}</div>}
      {entries === null ? (
        <div className="set-empty">{t("mkt.loading")}</div>
      ) : (
        <>
          <input
            className="prov-input"
            value={query}
            spellCheck={false}
            aria-label={t("mkt.search")}
            placeholder={t("mkt.search")}
            onChange={(e) => setQuery(e.target.value)}
          />
          {shown !== null && shown.length === 0 && <div className="set-empty">{t("mkt.empty")}</div>}
          <div className="prov-list">
            {shown?.map((e) => (
              <div className="prov plug" key={e.id} title={e.source}>
                <div className="prov-main">
                  <span className="prov-name">
                    {e.name}
                    {e.version !== null && <span className="plug-version"> {e.version}</span>}
                  </span>
                  {e.description !== null && <span className="plug-desc">{e.description}</span>}
                  {e.unavailable !== null && <span className="plug-note">{e.unavailable}</span>}
                </div>
                {e.category !== null && <span className="prov-proto">{e.category}</span>}
                <div className="prov-actions">
                  {e.installed !== null ? (
                    <span className="plug-note">{t("mkt.installed", { version: e.installed })}</span>
                  ) : (
                    <button
                      type="button"
                      className="btn btn-small"
                      disabled={e.unavailable !== null || busy !== null}
                      onClick={() => install(e.id)}
                    >
                      {busy === e.id ? t("mkt.loading") : t("mkt.install")}
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </>
  );
}
