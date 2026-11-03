/**
 * The provider's model list (§10) — asked for, not typed.
 *
 * Capabilities and prices come from the provider's own `/v1/models`.
 * The context window can be overridden when the provider omits it or reports
 * a limit that differs from the user's deployment.
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
import { tokens } from "../format";
import { MAX_CONTEXT_TOKENS, validContextSize } from "./provider";

export function ModelsList({
  models,
  busy,
  answered,
  error,
  onAsk,
  onContext,
  onPrice,
}: {
  models: ModelInfo[];
  busy: boolean;
  /** The last question asked from the form got its list: the list is the provider's word now. */
  answered: boolean;
  /** The provider's refusal. An empty list after success is not an error. */
  error: string | null;
  onAsk: () => void;
  onContext: (id: string, context: number) => void;
  onPrice: (id: string, kind: "input" | "output", value: number) => void;
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
          {answered && !busy && error === null && <> · {t("prov.answered")}</>}
        </span>
      </div>

      {error !== null && <div className="prov-problems">{error}</div>}

      {models.map((model) => (
        <div className="prov-model-row" key={model.id}>
          <span className="prov-model-name">{model.displayName}</span>
          <span className="prov-model-id">{model.id}</span>
          <label className="prov-context-field">
            <span className="prov-hint">{t("prov.form_model_context")}</span>
            <input
              className="prov-input prov-input-mono"
              type="number"
              min={1}
              max={MAX_CONTEXT_TOKENS}
              step={1}
              value={model.contextMax || ""}
              disabled={busy}
              aria-label={`${model.displayName}: ${t("prov.form_model_context")}`}
              aria-invalid={!validContextSize(model.contextMax)}
              onChange={(e) => onContext(model.id, e.target.valueAsNumber)}
            />
          </label>
          <span className="prov-model-num" title={t("prov.form_model_output")}>
            {tokens(model.maxOutput)}
          </span>
          {(["input", "output"] as const).map((kind) => (
            <label className="prov-context-field" key={kind}>
              <span className="prov-hint">{t(kind === "input" ? "prov.price_input" : "prov.price_output")}</span>
              <input className="prov-input prov-input-mono" type="number" min={0} step="any"
                value={Number.isFinite(model.pricing[kind]) ? model.pricing[kind] : ""} disabled={busy}
                aria-label={`${model.displayName}: ${t(kind === "input" ? "prov.price_input" : "prov.price_output")}`}
                aria-invalid={!Number.isFinite(model.pricing[kind]) || model.pricing[kind] < 0}
                onChange={(e) => onPrice(model.id, kind, e.target.valueAsNumber)} />
            </label>
          ))}
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
