/**
 * Project models (§10): what a new session starts with and what the history is
 * compacted with.
 *
 * The section is per project, not global: §10 lists "Models" in one list with the
 * providers, but they live in different places — providers in the app config, and
 * the model choice in `.yaca/settings.json`. Mixing them would mean changing the
 * model "for everyone" without saying so.
 *
 * §10 also names "For subagents" and "limits" — they are not here: there are no
 * subagents in the core at all, and no limit that anyone reads exists.
 *
 * The choice is made from the models of configured providers, and only from them.
 * A provider without models does not appear in §7.1, and offering it here would
 * suggest starting a conversation that will not start (§10).
 */

import type { ModelInfo, Settings } from "../../core/client";
import { t } from "../../i18n";
import { Row, Select } from "./Row";

/**
 * The "not set" value in the list. Not an empty string: its `Select` takes that for
 * "nothing selected", and angle brackets never occur in model identifiers.
 */
const NONE = "<none>";

export function ModelsSection({
  settings,
  models,
  onPatch,
}: {
  settings: Settings;
  models: ModelInfo[];
  onPatch: (change: (settings: Settings) => Settings) => void;
}) {
  const known = models.length > 0;

  return (
    <>
      <div className="set-head">
        <div>
          <h1 className="set-title">{t("models.title")}</h1>
          <p className="set-lede">{t("models.lede")}</p>
        </div>
      </div>

      {!known && <div className="set-empty">{t("models.none")}</div>}

      <div className="set-rows">
        <Row name={t("models.default")} hint={t("models.default_hint")}>
          <ModelSelect
            label={t("models.default")}
            unset={t("models.first_configured")}
            value={settings.model}
            models={models}
            onChange={(model) => {
              // The provider travels with the model: §10 says a model brings its own
              // provider, and writing the model without it would leave a pair in the
              // file that may not match.
              const provider = models.find((m) => m.id === model)?.providerId ?? null;
              onPatch((now) => ({ ...now, model, provider }));
            }}
          />
        </Row>

        <Row name={t("models.compact")} hint={t("models.compact_hint")}>
          <ModelSelect
            label={t("models.compact")}
            unset={t("models.compact_same")}
            value={settings.compactModel}
            models={models}
            onChange={(compactModel) => onPatch((now) => ({ ...now, compactModel }))}
          />
        </Row>

        <Row name={t("models.subagent")} hint={t("models.subagent_hint")}>
          <ModelSelect
            label={t("models.subagent")}
            unset={t("models.compact_same")}
            value={settings.subagentModel}
            models={models}
            onChange={(subagentModel) => onPatch((now) => ({ ...now, subagentModel }))}
          />
        </Row>
      </div>
    </>
  );
}

/** A model or "not set": the section's three rows differ only in their labels. */
function ModelSelect({
  label,
  unset,
  value,
  models,
  onChange,
}: {
  label: string;
  /** What "not set" is called — each row means something different by it. */
  unset: string;
  value: string | null;
  models: ModelInfo[];
  onChange: (model: string | null) => void;
}) {
  return (
    <Select
      mono
      label={label}
      value={value ?? NONE}
      options={[
        { value: NONE, label: unset },
        ...models.map((model) => ({ value: model.id, label: model.displayName })),
      ]}
      onChange={(next) => onChange(next === NONE ? null : next)}
    />
  );
}
