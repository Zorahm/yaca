/**
 * Модели проекта (§10): с чего начинается новая сессия и чем сжимается
 * история.
 *
 * Раздел проектный, а не общий: §10 перечисляет «Модели» в одном списке с
 * провайдерами, но лежат они в разных местах — провайдеры в конфиге
 * приложения, а выбор модели в `.yaca/settings.json`. Смешать их значило бы
 * поменять модель «для всех» и не сказать об этом.
 *
 * «Для подагентов» и «лимиты» §10 тоже называет — их здесь нет: подагентов в
 * ядре нет вовсе, а лимита, который кто-то читает, не существует.
 *
 * Выбор — из моделей настроенных провайдеров, и только из них. Провайдер без
 * моделей в §7.1 не появляется, и подставлять его сюда значило бы предложить
 * начать разговор, который не начнётся (§10).
 */

import type { ModelInfo, Settings } from "../../core/client";
import { t } from "../../i18n";
import { Row, Select } from "./Row";

/**
 * Значение «не задано» в списке. Не пустая строка: её `Select` держит за
 * «ничего не выбрано», а угловые скобки не встречаются в идентификаторах
 * моделей.
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
              // Провайдер едет с моделью: §10 говорит, что модель приводит
              // своего, и записать модель без него значило бы оставить в
              // файле пару, которая может не сойтись.
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

/** Модель или «не задано»: три строки раздела отличаются только подписями. */
function ModelSelect({
  label,
  unset,
  value,
  models,
  onChange,
}: {
  label: string;
  /** Как названо «не задано» — у каждой строки оно значит своё. */
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
