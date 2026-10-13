/**
 * Провайдеры (§10): список, описание целиком, модели, ключи.
 *
 * Раньше это была выдвижная панель §1.1 — промежуточный шаг, записанный в §16
 * п. 30 как «до окна настроек». Окно приехало, панель ушла.
 *
 * Главное в строке — кружок слева: залитый мхом значит «провайдер ответил
 * списком моделей», пустой — «его не спрашивали». §10 говорит «настроен —
 * значит, ответил», и от этого зависит всё остальное: неопрошенный не
 * появится в выборе модели §7.1, а домашний экран §9.1 не начнёт разговор.
 * Раньше это приходилось выводить из числа моделей в строке меты.
 *
 * `id` из строки убран: он почти всегда — имя в нижнем регистре, то есть шум.
 * В форме его теперь тоже нет: он назначается сам при создании
 * (`newProviderId`). Вместо него справа форма протокола — единственное, чего
 * нельзя угадать по адресу.
 *
 * Предустановленные без кнопки удаления: ядро всё равно откажется, а кнопка,
 * чьё нажатие всегда ошибка, врёт про то, что она кнопка (§0).
 */

import { ArrowClockwise, CaretLeft, PencilSimple, Plus, X } from "@phosphor-icons/react";
import { useState } from "react";

import type { ProviderSpec } from "../../core/client";
import { t, tp } from "../../i18n";
import { PROTOCOL_NAME, ProviderMark } from "../brand";
import { ProviderForm } from "./ProviderForm";
import { draftFromSpec, emptyDraft, isPreset, type ProviderDraft } from "./provider";
import { useProviders } from "./useProviders";

type Editing = { draft: ProviderDraft; isNew: boolean };

export function ProvidersSection({ onChanged }: { onChanged: () => void }) {
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

      {/* `providers === null` без ошибки — это миг до того, как список приехал;
          с ошибкой правда показана выше, а не пустым списком (§0). */}
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
  // Предустановленные не удаляются: ядро вернуло бы их следующей загрузкой.
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
        {/* Опрос — самое частое, что делают с ненастроенным провайдером, и
            кнопка стоит там, где видно беду: раньше за ней надо было открыть
            форму и пролистать её до конца. */}
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
