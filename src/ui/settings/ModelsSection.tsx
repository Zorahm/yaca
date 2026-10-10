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
import { Row } from "./Row";

/** Значение «не задано» в `<select>`: пустая строка, а не отсутствие опции. */
const NONE = "";

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
          <select
            className="prov-input prov-input-mono"
            aria-label={t("models.default")}
            value={settings.model ?? NONE}
            onChange={(e) => {
              const model = e.target.value === NONE ? null : e.target.value;
              // Провайдер едет с моделью: §10 говорит, что модель приводит
              // своего, и записать модель без него значило бы оставить в
              // файле пару, которая может не сойтись.
              const provider = models.find((m) => m.id === model)?.providerId ?? null;
              onPatch((now) => ({ ...now, model, provider }));
            }}
          >
            <option value={NONE}>{t("models.first_configured")}</option>
            {models.map((model) => (
              <option key={`${model.providerId}/${model.id}`} value={model.id}>
                {model.displayName}
              </option>
            ))}
          </select>
        </Row>

        <Row name={t("models.compact")} hint={t("models.compact_hint")}>
          <select
            className="prov-input prov-input-mono"
            aria-label={t("models.compact")}
            value={settings.compactModel ?? NONE}
            onChange={(e) => {
              const compactModel = e.target.value === NONE ? null : e.target.value;
              onPatch((now) => ({ ...now, compactModel }));
            }}
          >
            <option value={NONE}>{t("models.compact_same")}</option>
            {models.map((model) => (
              <option key={`${model.providerId}/${model.id}`} value={model.id}>
                {model.displayName}
              </option>
            ))}
          </select>
        </Row>

        <Row name={t("models.subagent")} hint={t("models.subagent_hint")}>
          <select
            className="prov-input prov-input-mono"
            aria-label={t("models.subagent")}
            value={settings.subagentModel ?? NONE}
            onChange={(e) => {
              const subagentModel = e.target.value === NONE ? null : e.target.value;
              onPatch((now) => ({ ...now, subagentModel }));
            }}
          >
            <option value={NONE}>{t("models.compact_same")}</option>
            {models.map((model) => (
              <option key={`${model.providerId}/${model.id}`} value={model.id}>
                {model.displayName}
              </option>
            ))}
          </select>
        </Row>
      </div>
    </>
  );
}
