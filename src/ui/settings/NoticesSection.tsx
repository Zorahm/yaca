/**
 * Уведомления и язык (§10, §14, §15).
 *
 * Два уровня приложения в одном разделе: и то и другое — про окно, а не про
 * проект, и обоих слишком мало на раздел поодиночке.
 *
 * Язык меняется на месте, без перезагрузки: словарь — модульная переменная,
 * а перерисовка дерева читает уже новый (`src/ui/prefs.ts`). Перезагружать
 * окна нельзя — в главном от этого закрылись бы открытые чаты.
 */

import type { Language, Preferences } from "../../core/client";
import { t } from "../../i18n";
import { Row, Segments, Switch } from "./Row";

export function NoticesSection({
  prefs,
  onPatch,
}: {
  prefs: Preferences;
  onPatch: (change: Partial<Preferences>) => void;
}) {
  return (
    <>
      <div className="set-head">
        <div>
          <h1 className="set-title">{t("notices.title")}</h1>
          <p className="set-lede">{t("notices.lede")}</p>
        </div>
      </div>

      <div className="set-rows">
        <Row name={t("notices.toasts")} hint={t("notices.toasts_hint")}>
          <Switch
            on={prefs.notifications}
            label={t("notices.toasts")}
            onChange={(notifications) => onPatch({ notifications })}
          />
        </Row>

        <Row name={t("notices.language")} hint={t("notices.language_hint")}>
          <Segments<Language>
            label={t("notices.language")}
            value={prefs.language}
            options={[
              { value: "ru", label: t("notices.language_ru") },
              { value: "en", label: t("notices.language_en") },
              { value: "system", label: t("notices.language_system") },
            ]}
            onChange={(language) => onPatch({ language })}
          />
        </Row>
      </div>
    </>
  );
}
