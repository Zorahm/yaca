/** Providers are grouped by setup, with readiness and the next action on each row. */
import { ArrowClockwise, CaretLeft, Plus, MagnifyingGlass } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";

import type { ModelInfo, ProviderSpec } from "../../core/client";
import { t } from "../../i18n";
import { ProviderForm } from "./ProviderForm";
import { ProviderRow } from "./ProviderRow";
import { draftFromSpec, emptyDraft, type ProviderDraft } from "./provider";
import { providerStatus, STATUS_ORDER } from "./providerStatus";
import { RoutingSection } from "./RoutingSection";
import { useProviders } from "./useProviders";
import "./providers.css";

type Editing = { draft: ProviderDraft; isNew: boolean; focusKey?: boolean };

export function ProvidersSection({ models, onChanged, currentProvider }: {
  models: ModelInfo[];
  onChanged: () => void;
  currentProvider?: string;
}) {
  const { providers, error, keySources, pollErrors, reload, save, remove, ask } = useProviders();
  const [editing, setEditing] = useState<Editing | null>(null);
  const [search, setSearch] = useState("");
  const [asking, setAsking] = useState<ReadonlySet<string>>(new Set());
  const inFlight = useRef(new Set<string>());
  const [refreshing, setRefreshing] = useState(false);
  const batchRunning = useRef(false);
  const [now, setNow] = useState(Date.now);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  const askRow = async (spec: ProviderSpec) => {
    if (inFlight.current.has(spec.id)) return;
    inFlight.current.add(spec.id);
    setAsking(new Set(inFlight.current));
    try {
      await ask(spec);
    } finally {
      inFlight.current.delete(spec.id);
      setAsking(new Set(inFlight.current));
      setNow(Date.now());
      onChanged();
    }
  };

  const closeForm = () => {
    setEditing(null);
    void reload();
    onChanged();
  };

  if (editing !== null) {
    return <>
      <button type="button" className="prov-back" onClick={closeForm}>
        <CaretLeft size={13} /> {t("prov.title")}
      </button>
      <ProviderForm
        key={editing.isNew ? "new" : editing.draft.id}
        draft={editing.draft} isNew={editing.isNew} focusKey={editing.focusKey}
        taken={providers?.list.map((spec) => spec.id) ?? []}
        onSave={save} onSaved={closeForm} onCancel={closeForm}
        checkedAt={providers?.polls[editing.draft.id]?.checkedAt}
      />
    </>;
  }

  const sourceOf = (spec: ProviderSpec) => keySources[spec.id] ?? { kind: "loading" as const };
  const statusOf = (spec: ProviderSpec) => providerStatus(spec, sourceOf(spec), providers?.polls[spec.id], asking.has(spec.id), pollErrors[spec.id]);
  const canPoll = (spec: ProviderSpec) => ["keychain", "environment", "not_needed"].includes(sourceOf(spec).kind);
  const pollable = providers?.list.filter(canPoll) ?? [];
  const refreshAll = async () => {
    if (batchRunning.current || inFlight.current.size > 0) return;
    batchRunning.current = true;
    setRefreshing(true);
    try {
      // Commands return the whole registry; serial polling avoids competing file writes.
      for (const spec of pollable) await askRow(spec);
    } finally {
      batchRunning.current = false;
      setRefreshing(false);
    }
  };
  const visible = (providers?.list ?? []).filter((spec) =>
    `${spec.displayName} ${spec.baseUrl}`.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()),
  ).sort((a, b) => STATUS_ORDER[statusOf(a)] - STATUS_ORDER[statusOf(b)] || a.displayName.localeCompare(b.displayName));
  const groups = [
    { name: t("prov.group_connected"), list: visible.filter(canPoll), muted: false },
    { name: t("prov.group_unconfigured"), list: visible.filter((spec) => !canPoll(spec)), muted: true },
  ];

  return <>
    <div className="set-head providers-head">
      <div>
        <h1 className="set-title">{t("prov.title")}</h1>
        <p className="set-lede">{t("prov.lede")}</p>
      </div>
      <div className="providers-toolbar">
        <button type="button" className="btn btn-small" disabled={!pollable.length || asking.size > 0 || refreshing}
          onClick={() => void refreshAll()}>
          <ArrowClockwise size={14} className={refreshing ? "prov-spin" : undefined} />
          {t("prov.refresh_all")}
        </button>
        <button type="button" className="btn btn-small" disabled={refreshing}
          onClick={() => setEditing({ draft: emptyDraft(), isNew: true })}>
          <Plus size={14} /> {t("prov.add")}
        </button>
      </div>
    </div>

    {error !== null && <div className="set-error" role="alert">{error}</div>}
    {providers === null && error === null && <p className="set-empty" role="status">{t("prov.loading")}</p>}
    {providers !== null && <>
      {(providers.list.length > 10 || search.length > 0) && <label className="providers-search">
        <MagnifyingGlass size={15} aria-hidden="true" />
        <input type="search" value={search} placeholder={t("prov.search_providers")}
          aria-label={t("prov.search_providers")} onChange={(event) => setSearch(event.target.value)} />
      </label>}
      {groups.filter((group) => group.list.length > 0).map((group) => <section className="providers-group" key={group.name} aria-label={group.name}>
        <h2 className="providers-group-title">{group.name}<span>{group.list.length}</span></h2>
        <div className="providers-list">
          {group.list.map((spec) => <ProviderRow key={spec.id} spec={spec} source={sourceOf(spec)}
            poll={providers.polls[spec.id]} status={statusOf(spec)} pollError={pollErrors[spec.id]} now={now}
            muted={group.muted} current={currentProvider === spec.id} locked={refreshing || asking.has(spec.id)}
            onAsk={() => void askRow(spec)}
            onEdit={(focusKey = false) => setEditing({ draft: draftFromSpec(spec), isNew: false, focusKey })}
            onDelete={() => void remove(spec.id).then(onChanged)} />)}
        </div>
      </section>)}
      {visible.length === 0 && <p className="set-empty" role="status">{t("prov.search_empty")}</p>}
    </>}
    <RoutingSection models={models} onChanged={onChanged} />
  </>;
}
