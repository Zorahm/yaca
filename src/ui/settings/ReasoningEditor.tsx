import type { ModelInfo } from "../../core/client";
import { EFFORT_KEY, EFFORT_LEVELS, effortLevels } from "../../core/reasoning";
import { t } from "../../i18n";
import { editEffort, editEffortLevels } from "./provider";

export function ReasoningEditor({ model, onUpdate }: {
  model: ModelInfo;
  onUpdate: (model: ModelInfo) => void;
}) {
  const levels = effortLevels(model);
  const configuredLevels = effortLevels({ ...model, supportsEffort: true });
  const unknownLevels = model.reasoning?.supportedEfforts === undefined &&
    (model.reasoning !== undefined || model.supportsEffort);
  return (
    <div className="prov-reasoning-editor">
      <div className="prov-effort-controls">
        {configuredLevels.length > 0 && <button type="button" className={`prov-filter-pill ${model.supportsEffort ? "active" : ""}`}
          aria-pressed={model.supportsEffort}
          onClick={() => onUpdate(editEffort(model, !model.supportsEffort))}>
          {t("prov.effort_enabled")}
        </button>}
        {levels.map((level) => <span key={level} className="chip-cap">{t(EFFORT_KEY[level])}</span>)}
        {unknownLevels && <span className="prov-api-hint">{t("prov.effort_unknown")}</span>}
      </div>
      <details>
        <summary>{t("prov.effort_customize")}</summary>
        <div className="prov-effort-controls">
          {EFFORT_LEVELS.map((level) => (
            <button key={level} type="button"
              className={`prov-filter-pill ${configuredLevels.includes(level) ? "active" : ""}`}
              aria-pressed={configuredLevels.includes(level)}
              disabled={level === "none" && model.reasoning?.mandatory}
              onClick={() => onUpdate(editEffortLevels(model,
                configuredLevels.includes(level) ? configuredLevels.filter((item) => item !== level) : [...configuredLevels, level]))}>
              {t(EFFORT_KEY[level])}
            </button>
          ))}
        </div>
      </details>
    </div>
  );
}
