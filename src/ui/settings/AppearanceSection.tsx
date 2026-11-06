/** Application appearance and language, with a live preview of visual choices. */

import type { Language, Preferences, Theme } from "../../core/client";
import { LOCALES, LOCALE_CODES, t } from "../../i18n";
import { Row, Segments, Select, Switch } from "./Row";
import { AppearancePreview } from "./AppearancePreview";

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
          <h1 className="set-title">{t("settings.general")}</h1>
          <p className="set-lede">{t("settings.general_hint")}</p>
        </div>
      </div>

      <div className="set-general-layout"><div className="set-rows">
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

        <Row name={t("notices.language")}>
          <Select<Language> label={t("notices.language")} value={prefs.language}
            options={[{ value: "system", label: t("notices.language_system") }, ...LOCALE_CODES.map((code) => ({ value: code, label: LOCALES[code].name }))]}
            onChange={(language) => onPatch({ language })} />
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
        <Row name={t("notices.toasts")}>
          <Switch on={prefs.notifications} label={t("notices.toasts")} onChange={(notifications) => onPatch({ notifications })} />
        </Row>
      </div><AppearancePreview prefs={prefs} /></div>
    </>
  );
}
