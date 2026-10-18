/**
 * Providers (§10): the list, the full description, models, keys.
 *
 * This used to be the slide-out panel §1.1 — an intermediate step, written in §16
 * item 30 as "before the settings window". The window arrived, the panel went away.
 *
 * The main thing in a row is the dot on the left: filled with moss means "the
 * provider answered with a list of models", empty means "it was not asked". §10
 * says "configured — means answered", and everything else depends on it: an unasked
 * provider will not appear in the model choice §7.1, and the home screen §9.1 will
 * not start a conversation. Before, this had to be inferred from the model count in
 * the meta line.
 *
 * `id` was removed from the row: it is almost always the name in lower case, which
 * is noise. It is not in the form either: it is assigned automatically on creation
 * (`newProviderId`). Instead, on the right is the protocol form — the only thing that
 * cannot be guessed from the address.
 *
 * Built-in providers have no delete button: the core would refuse anyway, and a
 * button whose press is always an error lies about being a button (§0).
 */

import { ArrowClockwise, CaretLeft, PencilSimple, Plus, X } from "@phosphor-icons/react";
import { useState } from "react";

import type { ModelInfo, ProviderSpec } from "../../core/client";
import { t, tp } from "../../i18n";
import { PROTOCOL_NAME, ProviderMark } from "../brand";
import { ProviderForm } from "./ProviderForm";
import { draftFromSpec, emptyDraft, isPreset, type ProviderDraft } from "./provider";
import { RoutingSection } from "./RoutingSection";
import { useProviders } from "./useProviders";

type Editing = { draft: ProviderDraft; isNew: boolean };

export function ProvidersSection({
  models,
  onChanged,
}: {
  /** Every model the providers listed: the router picks its three from them. */
  models: ModelInfo[];
  onChanged: () => void;
}) {
  const { providers, error, save, remove } = useProviders();
  const [editing, setEditing] = useState<Editing | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  const done = () => {
    setEditing(null);
    onChanged();
  };

  if (editing !== null) {
    return (
      <>
        <button type="button" className="prov-back" onClick={() => setEditing(null)}>
          <CaretLeft size={13} /> {t("prov.title")}
        </button>
        <ProviderForm
          key={editing.isNew ? "new" : editing.draft.id}
          draft={editing.draft}
          isNew={editing.isNew}
          taken={providers?.list.map((spec) => spec.id) ?? []}
          onSave={save}
          onSaved={done}
          onCancel={() => setEditing(null)}
        />
      </>
    );
  }

  return (
    <>
      <div className="set-head">
        <div>
          <h1 className="set-title">{t("prov.title")}</h1>
          <p className="set-lede">{t("prov.lede")}</p>
        </div>
        <button
          type="button"
          className="btn btn-small"
          onClick={() => setEditing({ draft: emptyDraft(), isNew: true })}
        >
          <Plus size={14} /> {t("prov.add")}
        </button>
      </div>

      {error !== null && <div className="set-error">{error}</div>}

      {/* `providers === null` without an error is the moment before the list arrives;
          with an error the truth is shown above, not as an empty list (§0). */}
      {providers !== null && (
        <>
          <div className="prov-list">
            {providers.list.map((spec) => (
              <ProviderRow
                key={spec.id}
                spec={spec}
                confirming={confirming === spec.id}
                onEdit={() => setEditing({ draft: draftFromSpec(spec), isNew: false })}
                onDeleteStart={() => setConfirming(spec.id)}
                onDeleteCancel={() => setConfirming(null)}
                onDelete={() => {
                  setConfirming(null);
                  void remove(spec.id).then(onChanged);
                }}
              />
            ))}
          </div>

          <div className="prov-note">
            <span className="prov-dot" aria-hidden="true" />
            <span>{t("prov.unasked_note")}</span>
          </div>
        </>
      )}

      {/* The router is made of the providers above, so it is set up under them. */}
      <RoutingSection models={models} onChanged={onChanged} />
    </>
  );
}

function ProviderRow({
  spec,
  confirming,
  onEdit,
  onDeleteStart,
  onDeleteCancel,
  onDelete,
}: {
  spec: ProviderSpec;
  confirming: boolean;
  onEdit: () => void;
  onDeleteStart: () => void;
  onDeleteCancel: () => void;
  onDelete: () => void;
}) {
  // Built-in providers are not deleted: the core would bring them back on the next load.
  const builtin = isPreset(spec.id);
  const answered = spec.models.length > 0;

  return (
    <div className="prov">
      <span
        className={answered ? "prov-dot prov-dot-on" : "prov-dot"}
        title={answered ? t("prov.answered") : t("prov.unasked")}
      />

      <button type="button" className="prov-main" onClick={onEdit} title={t("prov.edit_row")}>
        <span className="prov-name">
          <ProviderMark id={spec.id} size={14} />
          <span className="prov-name-text">{spec.displayName}</span>
        </span>
        <span className="prov-meta">
          {spec.baseUrl} ·{" "}
          {answered ? tp("prov.models_count", spec.models.length) : t("prov.unasked")}
        </span>
      </button>

      <span className="prov-proto">{PROTOCOL_NAME[spec.protocol]}</span>

      <div className="prov-actions">
        {/* Asking for models is the most common thing done with an unconfigured
            provider, and the button sits where the trouble is visible: before, one had
            to open the form and scroll it to the end. */}
        {!answered && (
          <button type="button" className="btn btn-small" onClick={onEdit}>
            <ArrowClockwise size={13} /> {t("prov.models_ask")}
          </button>
        )}

        <button
          type="button"
          className="btn btn-row"
          aria-label={t("prov.edit_row")}
          title={t("prov.edit_row")}
          onClick={onEdit}
        >
          <PencilSimple size={14} />
        </button>

        {!builtin &&
          (confirming ? (
            <button type="button" className="btn btn-row btn-danger" onClick={onDelete}>
              {t("prov.delete_confirm")}
            </button>
          ) : (
            <button
              type="button"
              className="btn btn-row btn-danger"
              aria-label={t("prov.delete")}
              title={t("prov.delete")}
              onClick={onDeleteStart}
              onBlur={onDeleteCancel}
            >
              <X size={14} />
            </button>
          ))}
      </div>
    </div>
  );
}
