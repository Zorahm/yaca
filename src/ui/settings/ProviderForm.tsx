/**
 * Provider description form (§10): name, protocol, base URL, headers, key,
 * models.
 *
 * Presentation only — draft state and command calls live in `useProviderForm`.
 *
 * Three shapes of one form:
 *
 * - A preset already knows its name, protocol and address, so those are
 *   folded under "Advanced" and what is left is the one thing it waits for,
 *   the key.
 * - A provider of one's own shows everything.
 * - A new one shows everything but the models: it has no id until it is saved
 *   (`newProviderId`), so there is nobody to ask yet, and the shell asks on
 *   the save.
 *
 * The id has no field in any of them. It is the keychain name and the entry in
 * `providers.json` — bookkeeping, assigned once.
 *
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
import { isPreset, type ProviderDraft } from "./provider";

/** OpenAI first: it is what a gateway of one's own almost always speaks. */
const PROTOCOLS: Protocol[] = ["openai", "anthropic"];

export function ProviderForm({
  draft: initial,
  isNew,
  taken,
  onSave,
  onSaved,
  onCancel,
}: {
  draft: ProviderDraft;
  isNew: boolean;
  /** The ids already in the list (`newProviderId`). */
  taken: ReadonlyArray<string>;
  onSave: (spec: ProviderSpec) => Promise<boolean>;
  onSaved: () => void;
  onCancel: () => void;
}) {
  const form = useProviderForm({ draft: initial, isNew, taken, onSave, onSaved });
  const { draft, keyState } = form;
  const canSave = !form.busy && form.problems.length === 0;
  const keyFilled = form.keyValue.trim().length > 0;
  const preset = !isNew && isPreset(draft.id);

  const description = (
    <>
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
    </>
  );

  const keySection = keyState.kind !== "not_needed" && keyState.kind !== "loading" && (
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
      {/* A new provider says the same thing in the hint under the field. */}
      {keyState.kind === "unknown" && !isNew && (
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
        {/* A new provider has no id to keep a key under yet: its key goes
            with "Save provider", and a button here would have nowhere to
            put it. */}
        {!isNew && (
          <button
            type="button"
            className="btn btn-small"
            disabled={!keyFilled || form.keyBusy}
            onClick={() => void form.saveKey()}
          >
            {t("prov.key_save")}
          </button>
        )}
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
      {isNew && <div className="prov-hint">{t("prov.form_key_new_hint")}</div>}
      {form.keyError && <div className="prov-problems">{form.keyError}</div>}
    </section>
  );

  return (
    <div className="prov-form">
      <div className="prov-form-head">
        {isNew ? t("prov.form_new") : draft.displayName || draft.id}
      </div>

      {!preset && description}

      {keySection}

      {keyState.kind === "not_needed" && (
        <div className="prov-hint">{t("prov.key_not_needed")}</div>
      )}

      {!isNew && (
        <section className="prov-section">
          <span className="prov-label">{t("prov.form_models")}</span>
          <div className="prov-hint">{t("prov.form_models_hint")}</div>
          <ModelsList
            models={draft.models}
            busy={form.modelsBusy || form.keyBusy}
            error={form.modelsError}
            onAsk={form.askForModels}
          />
        </section>
      )}

      {preset && (
        <details className="prov-more">
          <summary>{t("prov.form_more")}</summary>
          <div className="prov-more-body">{description}</div>
        </details>
      )}

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
