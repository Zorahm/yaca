/**
 * Внешний вид (§10): тема, акцент, жёлоб и высота строки диффа.
 *
 * Всё здесь — уровень приложения, и всё применяется сразу и во всех окнах:
 * `setPreferences` пишет файл и рассылает настройки, а `usePreferences` в
 * каждом окне ставит атрибуты на `<html>`. Тема, сменившаяся только там, где
 * её выбрали, была бы интерфейсом, врущим про состояние (§0).
 *
 * Размера шрифтов тут нет, хотя §10 его называет: типографика §2 записана
 * пиксельными размерами, а не шкалой, и строка «Размер текста» ничего бы не
 * поменяла — см. README, «Чего нет».
 */

import type { Preferences, Theme } from "../../core/client";
import { t } from "../../i18n";
import { Row, Segments, Switch } from "./Row";

export function AppearanceSection({
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
          <h1 className="set-title">{t("look.title")}</h1>
          <p className="set-lede">{t("look.lede")}</p>
        </div>
      </div>

      <div className="set-rows">
        <Row name={t("look.theme")} hint={t("look.theme_hint")}>
          <Segments<Theme>
            label={t("look.theme")}
            value={prefs.theme}
            options={[
              { value: "paper", label: t("look.theme_paper") },
              { value: "night", label: t("look.theme_night") },
              { value: "system", label: t("look.theme_system") },
            ]}
            onChange={(theme) => onPatch({ theme })}
          />
        </Row>

        <Row name={t("look.line_numbers")} hint={t("look.line_numbers_hint")}>
          <Switch
            on={prefs.lineNumbers}
            label={t("look.line_numbers")}
            onChange={(lineNumbers) => onPatch({ lineNumbers })}
          />
        </Row>

        <Row name={t("look.compact")} hint={t("look.compact_hint")}>
          <Switch
            on={prefs.compact}
            label={t("look.compact")}
            onChange={(compact) => onPatch({ compact })}
          />
        </Row>
      </div>
    </>
  );
}
