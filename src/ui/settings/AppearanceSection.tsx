/**
 * Appearance (§10): theme, accent, gutter and diff row height.
 *
 * Everything here is app-level, and everything applies at once in every window:
 * `setPreferences` writes the file and broadcasts the settings, and
 * `usePreferences` in each window sets the attributes on `<html>`. A theme that
 * changed only where it was picked would be an interface lying about its state (§0).
 *
 * There is no font size here, though §10 names it: the typography in §2 is written
 * in pixel sizes, not a scale, so a "Text size" row would change nothing — see
 * README.
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
