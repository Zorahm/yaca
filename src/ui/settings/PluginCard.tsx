import { useState } from "react";
import type { PluginSummary } from "../../core/client";
import { t, type Key } from "../../i18n";
import { Switch } from "./Row";
import { PluginChip, PluginConfirm, PluginContentsView, PluginCounts, PluginDescription, PluginMenu, PluginOriginal } from "./PluginParts";
import { compactPluginPath } from "./pluginPresentation";

const ORIGIN: Record<PluginSummary["origin"], Key> = {
  yaca: "plug.origin_yaca", marketplace: "plug.origin_yaca", claudeCode: "plug.origin_claude",
};

export function PluginCard({ plugin: p, busy, nextVersion, onToggle, onUpdate, onRemove, onPrefer, onIgnore }: {
  plugin: PluginSummary; busy: boolean; nextVersion: string | null;
  onToggle: (on: boolean) => void; onUpdate: () => void; onRemove: () => void;
  onPrefer: (path: string) => void; onIgnore: (ignored: boolean) => void;
}) {
  const [confirming, setConfirming] = useState(false);
  const duplicate = p.duplicates.some((d) => !d.ignored);
  const managed = p.origin === "marketplace";
  return <article className="plugin-card" aria-label={p.name}>
    <div className="plugin-card-header">
      <div className="plugin-identity">
        <h2>{p.name}</h2>
        {p.version && <span className="plugin-version">{nextVersion ? `${p.version} → ${nextVersion}` : p.version}</span>}
        <PluginChip>{t(ORIGIN[p.origin])}</PluginChip>
        {duplicate && <PluginChip tone="warn">{t("plug.duplicate")}</PluginChip>}
      </div>
      <div className="plugin-controls">
        <fieldset disabled={busy} className="plugin-switch"><Switch on={p.enabled} label={t("plug.enabled", { name: p.name })} onChange={onToggle} /></fieldset>
        <PluginMenu label={t("plug.actions", { name: p.name })} disabled={busy} items={[
          ...(managed ? [{ label: t("plug.update_source"), onSelect: onUpdate }] : []),
          { label: t("plug.remove"), onSelect: () => setConfirming(true), disabled: !managed,
            hint: !managed ? t(p.origin === "claudeCode" ? "plug.managed_claude" : "plug.managed_folder") : undefined },
        ]} />
      </div>
    </div>
    <PluginDescription text={p.description} />
    {nextVersion && <button type="button" className="btn btn-small" title={t("plug.update_source")} disabled={busy} onClick={onUpdate}>{t("plug.update")}</button>}
    <details className="plugin-expand">
      <summary><PluginCounts contents={p.contents} /><span className="plugin-expand-label">{t("plug.contents")}</span></summary>
      <PluginContentsView contents={p.contents} />
      <div className="plugin-path"><code title={p.path}>{compactPluginPath(p.path)}</code></div>
      {p.duplicates.map((d) => <div key={d.path} className="plugin-warning">
        <p>{t("plug.duplicate_found", { origin: t(ORIGIN[d.origin]), name: d.id.split("@")[1] ?? d.id, active: t(ORIGIN[p.origin]) })}</p>
        <code title={d.path}>{compactPluginPath(d.path)}</code>
        <div className="plugin-actions">
          <button type="button" className="btn btn-small" disabled={busy} onClick={() => onPrefer(d.path)}>{t("plug.use_copy", { origin: t(ORIGIN[d.origin]) })}</button>
          <button type="button" className="btn btn-small" disabled={busy} onClick={() => onIgnore(!d.ignored)}>{t(d.ignored ? "plug.unignore" : "plug.ignore")}</button>
        </div>
        <PluginOriginal text={`${d.message}\n${d.path}\n${p.path}`} />
      </div>)}
      {p.notes.length > 0 && <div className="plugin-warning"><p>{t("plug.component_notes")}</p><PluginOriginal text={p.notes.join("\n")} /></div>}
    </details>
    {confirming && <PluginConfirm text={t("plug.remove_question", { name: p.name })} busy={busy} onCancel={() => setConfirming(false)} onConfirm={() => { setConfirming(false); onRemove(); }} />}
  </article>;
}
