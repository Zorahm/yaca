/**
 * The provider's model list (§10) — asked for, not typed.
 *
 * Everything here came from the provider's own `/v1/models`: it knows its
 * context windows, its output caps and which of its models take `effort`
 * better than anyone filling in six fields per row, and a number typed by hand
 * goes stale the week a model changes. So the list is shown, not edited.
 *
 * What the row shows is what decides whether a model runs and what it costs —
 * `effort`, the caps, the price. A price of zero is not hidden: a local model
 * really is free, and a paid one the price list has not heard of is counted as
 * free too, which the human should be able to see rather than discover in the
 * ring §7.1.
 *
 * An empty list after a successful lookup stays empty. "The provider serves
 * nothing under this key" is an answer, and a provider with no models cannot
 * be chosen — which the core says plainly at the first attempt.
 */

import { t, tp } from "../../i18n";
import type { ModelInfo } from "../../core/client";
import { money, tokens } from "../format";

export function ModelsList({
  models,
  busy,
  error,
  onAsk,
}: {
  models: ModelInfo[];
  busy: boolean;
  /** Отказ провайдера. Пустой список после успеха — не ошибка. */
  error: string | null;
  onAsk: () => void;
}) {
  return (
    <div className="prov-rows">
      <div className="prov-models-head">
        <button type="button" className="btn btn-row" disabled={busy} onClick={onAsk}>
          {busy ? t("prov.models_asking") : t("prov.models_ask")}
        </button>
        <span className="prov-models-note">
          {models.length === 0
            ? t("prov.models_none")
            : tp("prov.models_count", models.length)}
        </span>
      </div>

      {error !== null && <div className="prov-problems">{error}</div>}

      {models.map((model) => (
        <div className="prov-model-row" key={model.id}>
          <span className="prov-model-name">{model.displayName}</span>
          <span className="prov-model-id">{model.id}</span>
          <span className="prov-model-num" title={t("prov.form_model_context")}>
            {tokens(model.contextMax)}
          </span>
          <span className="prov-model-num" title={t("prov.form_model_output")}>
            {tokens(model.maxOutput)}
          </span>
          <span className="prov-model-num" title={t("prov.models_price")}>
            {money(model.pricing.input)} / {money(model.pricing.output)}
          </span>
          {model.supportsEffort && (
            <span className="prov-model-flag" title={t("prov.form_model_effort")}>
              {t("prov.models_effort")}
            </span>
          )}
        </div>
      ))}
    </div>
  );
}
