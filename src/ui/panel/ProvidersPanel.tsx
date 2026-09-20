/**
 * Providers panel (§10) — the interim home of provider editing until the
 * settings window (§10) exists: the list, adding, editing, deleting.
 *
 * An interim step by agreement, recorded in §16: the window with side
 * navigation stays the goal, and this panel exists so the shell commands stop
 * being callable only by hand.
 *
 * The list comes from the shell whole; a file that did not parse shows its
 * error instead of an empty list, which would claim "no providers" (§0).
 * The presets, `anthropic` and `openai`, get no delete button — the core
 * refuses them anyway, and a button whose press always errors would lie about
 * being an option.
 */

import { PencilSimple, Plus, X } from "@phosphor-icons/react";
import { useState } from "react";

import type { ProviderSpec } from "../../core/client";
import { t, tp } from "../../i18n";
import { Panel } from "./Panel";
import { ProviderForm } from "./ProviderForm";
import { draftFromSpec, emptyDraft, isPreset, type ProviderDraft } from "./provider";
import { useProviders } from "./useProviders";

type Editing = { draft: ProviderDraft; isNew: boolean };

export function ProvidersPanel({
  withoutTopBar = false,
  onClose,
}: {
  /** The home screen §9.1 has no top bar, so the panel covers the full height. */
  withoutTopBar?: boolean;
  onClose: () => void;
}) {
  const { providers, error, save, remove } = useProviders();
  const [editing, setEditing] = useState<Editing | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  return (
    <Panel
      open
      onOpenChange={(next) => !next && onClose()}
      title={t("prov.title")}
      className={withoutTopBar ? "panel-cover" : undefined}
    >
      {error && <div className="panel-error">{error}</div>}

      {/* `providers === null` with no error is the moment before the list
          arrives; with an error the truth is shown above, not an empty list. */}
      <div className="panel-body">
        {providers === null ? null : editing ? (
          <ProviderForm
            key={editing.isNew ? "new" : editing.draft.id}
            draft={editing.draft}
            isNew={editing.isNew}
            onSave={save}
            onSaved={() => setEditing(null)}
            onCancel={() => setEditing(null)}
          />
        ) : (
          <>
            <div className="prov-toolbar">
              <button
                type="button"
                className="btn btn-small"
                onClick={() => setEditing({ draft: emptyDraft(), isNew: true })}
              >
                <Plus size={14} /> {t("prov.add")}
              </button>
            </div>

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
                  void remove(spec.id);
                }}
              />
            ))}
          </>
        )}
      </div>
    </Panel>
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
  // Предустановленные не удаляются: ядро вернуло бы их следующей загрузкой.
  const builtin = isPreset(spec.id);

  return (
    <div className="prov">
      <button type="button" className="prov-main" onClick={onEdit} title={t("prov.edit_row")}>
        <span className="prov-name">{spec.displayName}</span>
        <span className="prov-meta">
          {spec.id} · {spec.protocol} · {spec.baseUrl}
        </span>
        <span className="prov-meta">{tp("prov.models_count", spec.models.length)}</span>
      </button>

      <div className="prov-actions">
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
