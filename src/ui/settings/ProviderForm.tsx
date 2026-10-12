/**
 * Provider description form (§10): identity, protocol, base URL, headers,
 * key, models.
 *
 * Presentation only — draft state and command calls live in `useProviderForm`.
 * The key section appears exactly when the draft's headers mention `{key}`;
 * a provider that needs no key gets neither a required field nor a missing-key
 * warning (§10). The field itself is write-only: what is typed goes to the
 * keychain and is never rendered back.
 */

import { Check } from "@phosphor-icons/react";

import type { Protocol, ProviderSpec } from "../../core/client";
import { t } from "../../i18n";
import { PROTOCOL_NAME, ProtocolMark } from "../brand";
import { HeadersEditor } from "./HeadersEditor";
import { ModelsList } from "./ModelsList";
import { useProviderForm } from "./useProviderForm";
import type { ProviderDraft } from "./provider";

const PROTOCOLS: Protocol[] = ["anthropic", "openai"];

export function ProviderForm({
  draft: initial,
  isNew,
  onSave,
  onSaved,
  onCancel,
}: {
  draft: ProviderDraft;
  isNew: boolean;
  onSave: (spec: ProviderSpec) => Promise<boolean>;
  onSaved: () => void;
  onCancel: () => void;
}) {
  const form = useProviderForm({ draft: initial, isNew, onSave, onSaved });
  const { draft, keyState } = form;
  const canSave = !form.busy && form.problems.length === 0;
  const keyFilled = form.keyValue.trim().length > 0;
  const hasId = draft.id.trim().length > 0;

  return (
    <div className="prov-form">
      <div className="prov-form-head">
        {isNew ? t("prov.form_new") : draft.displayName || draft.id}
      </div>

      <div className="prov-grid">
        <label className="prov-field">
          <span className="prov-label">{t("prov.form_name")}</span>
          <input
            className="prov-input"
            value={draft.displayName}
            spellCheck={false}
            onChange={(e) => form.patch({ displayName: e.target.value })}
          />
        </label>
        <label className="prov-field">
          <span className="prov-label">{t("prov.form_id")}</span>
          {/* The id is the identity and the keychain name: editing it on an
              existing provider would create a second one and orphan the key. */}
          <input
            className="prov-input prov-input-mono"
            value={draft.id}
            readOnly={!isNew}
            spellCheck={false}
            aria-readonly={!isNew}
            onChange={(e) => form.patch({ id: e.target.value })}
          />
        </label>
      </div>
      {!isNew && <div className="prov-hint">{t("prov.form_id_hint")}</div>}

      <div className="prov-grid">
        <div className="prov-field" role="group" aria-label={t("prov.form_protocol")}>
          <span className="prov-label">{t("prov.form_protocol")}</span>
          <div className="segments">
            {PROTOCOLS.map((protocol) => (
              <button
                type="button"
                key={protocol}
                className="segment"
                aria-pressed={draft.protocol === protocol}
                onClick={() => form.patch({ protocol })}
              >
                <ProtocolMark protocol={protocol} size={13} />
                {PROTOCOL_NAME[protocol]}
              </button>
            ))}
          </div>
        </div>
        <label className="prov-field">
          <span className="prov-label">{t("prov.form_base_url")}</span>
          <input
            className="prov-input prov-input-mono"
            value={draft.baseUrl}
            spellCheck={false}
            onChange={(e) => form.patch({ baseUrl: e.target.value })}
          />
          {/* Частая ошибка — адрес сайта вместо адреса API: запрос уходит на
              веб-сервер и отвечает HTML-страницей 404. */}
          <span className="prov-hint">{t("prov.form_base_url_hint")}</span>
        </label>
      </div>

      <section className="prov-section">
        <span className="prov-label">{t("prov.form_headers")}</span>
        <div className="prov-hint">{t("prov.form_headers_hint")}</div>
        <HeadersEditor
          headers={draft.headers}
          onPatch={form.patchHeader}
          onAdd={form.addHeader}
          onRemove={form.removeHeader}
        />
      </section>

      {keyState.kind !== "not_needed" && keyState.kind !== "loading" && (
        <section className="prov-section">
          <span className="prov-label">{t("prov.form_key_title")}</span>
          <div className="prov-hint">{t("prov.form_key_hint")}</div>

          {keyState.kind === "keychain" && (
            <div className="prov-source prov-source-ok"><Check size={12} weight="bold" /> {t("prov.key_source_keychain")}</div>
          )}
          {keyState.kind === "environment" && (
            <div className="prov-source prov-source-ok"><Check size={12} weight="bold" /> {t("prov.key_source_environment")}</div>
          )}
          {keyState.kind === "none" && (
            <div className="prov-source prov-source-warn">{t("prov.key_source_none")}</div>
          )}
          {keyState.kind === "unknown" && (
            <div className="prov-source">{t("prov.key_source_unknown")}</div>
          )}

          <div className="prov-row">
            <input
              className="prov-input prov-input-mono prov-key-input"
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={form.keyValue}
              aria-label={t("prov.form_key_title")}
              onChange={(e) => form.setKeyValue(e.target.value)}
            />
            <button
              type="button"
              className="btn btn-small"
              disabled={!keyFilled || !hasId || form.keyBusy}
              onClick={() => void form.saveKey()}
            >
              {t("prov.key_save")}
            </button>
            {keyState.kind === "keychain" && (
              <button
                type="button"
                className="btn btn-small btn-danger"
                disabled={form.keyBusy}
                onClick={() => void form.forgetKey()}
              >
                {t("prov.key_forget")}
              </button>
            )}
          </div>
          {form.keyError && <div className="prov-problems">{form.keyError}</div>}
        </section>
      )}

      {keyState.kind === "not_needed" && (
        <div className="prov-hint">{t("prov.key_not_needed")}</div>
      )}

      <section className="prov-section">
        <span className="prov-label">{t("prov.form_models")}</span>
        <div className="prov-hint">{t("prov.form_models_hint")}</div>
        <ModelsList
          models={draft.models}
          busy={form.modelsBusy}
          error={form.modelsError}
          onAsk={form.askForModels}
        />
      </section>

      {form.error && <div className="prov-problems">{form.error}</div>}
      {form.problems.length > 0 && (
        <div className="prov-problems">
          {form.problems.map((problem, index) => (
            <div key={index}>{t(problem.key, problem.params)}</div>
          ))}
        </div>
      )}

      <div className="prov-form-foot">
        <button type="button" className="btn" onClick={onCancel}>
          {t("prov.form_cancel")}
        </button>
        <button
          type="button"
          className="btn btn-accent"
          disabled={!canSave}
          onClick={() => void form.submit()}
        >
          {t("prov.form_save")}
        </button>
      </div>
    </div>
  );
}
