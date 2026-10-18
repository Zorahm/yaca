/**
 * Notifications and language (§10, §14, §15).
 *
 * Two app-level settings in one section: both are about the window, not the
 * project, and each is too little for a section of its own.
 *
 * The language changes in place, without a reload: the dictionary is a module
 * variable, and the tree re-renders reading the new one (`src/ui/prefs.ts`).
 * Reloading the windows is not an option — in the main one it would close open chats.
 */

import type { Language, Preferences } from "../../core/client";
import { LOCALES, LOCALE_CODES, t } from "../../i18n";
import { Row, Select, Switch } from "./Row";

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
          <Select<Language>
            label={t("notices.language")}
            value={prefs.language}
            options={[
              { value: "system", label: t("notices.language_system") },
              // Each language in itself, from the registry: a new one shows
              // up here without this file knowing.
              ...LOCALE_CODES.map((code) => ({ value: code, label: LOCALES[code].name })),
            ]}
            onChange={(language) => onPatch({ language })}
          />
        </Row>
      </div>
    </>
  );
}
