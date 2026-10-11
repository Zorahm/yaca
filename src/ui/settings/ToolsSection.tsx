/**
 * Инструменты (§10).
 *
 * Сегодня в разделе три настоящие строки: шелл команд, `robots.txt` и ключ
 * веб-поиска. Шелл выбирают только там, где есть из чего: на Windows, между
 * PowerShell 7 и Windows PowerShell. Где он один, строки нет.
 * Включение тулов, таймаут команд и деньлист §10 тоже называет, но за ними нет
 * ничего в ядре: набор тулов — фиксированный `Tools::standard()`, а таймаут
 * команды приходит от модели в `timeout_ms`. Переключатель, за которым нет
 * команды, — это интерфейс, врущий про состояние (§0), поэтому их здесь и нет.
 *
 * Ключ ведёт себя ровно как ключ провайдера: поле только на запись, значение
 * уезжает в кейчейн под своим именем и на экран не возвращается, а показан
 * источник — кейчейн, переменная окружения или «нет».
 */

import { Check } from "@phosphor-icons/react";
import { useCallback, useEffect, useState } from "react";

import {
  forgetApiKey,
  installedShells,
  searchKeySource,
  SEARCH_KEY_ITEM,
  setApiKey,
  type KeySource,
  type Preferences,
  type ShellChoice,
} from "../../core/client";
import { t, type Key } from "../../i18n";
import { Row, Segments, Switch } from "./Row";

/** Как каждый шелл назван в строке выбора. Имена собственные, но через словарь:
    «Авто» рядом с ними — слово, и в компоненте строк не бывает (§14). */
const SHELL_NAME: Record<ShellChoice, Key> = {
  auto: "tools.shell_auto",
  pwsh: "tools.shell_pwsh",
  windowsPowerShell: "tools.shell_windows",
};

export function ToolsSection({
  prefs,
  onPatch,
}: {
  prefs: Preferences;
  onPatch: (change: Partial<Preferences>) => void;
}) {
  const [source, setSource] = useState<KeySource | null>(null);
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Шеллы этой машины. Пока не ответили и где их нет — строки нет. */
  const [shells, setShells] = useState<ShellChoice[]>([]);

  useEffect(() => {
    void installedShells().then(setShells, () => setShells([]));
  }, []);

  const refresh = useCallback(() => {
    void searchKeySource().then(setSource, (e: unknown) => setError(String(e)));
  }, []);

  useEffect(refresh, [refresh]);

  const save = () => {
    setBusy(true);
    setError(null);
    void setApiKey(value, SEARCH_KEY_ITEM)
      .then(() => setValue(""))
      .catch((e: unknown) => setError(String(e)))
      .finally(() => {
        setBusy(false);
        refresh();
      });
  };

  const forget = () => {
    setBusy(true);
    setError(null);
    void forgetApiKey(SEARCH_KEY_ITEM)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => {
        setBusy(false);
        refresh();
      });
  };

  return (
    <>
      <div className="set-head">
        <div>
          <h1 className="set-title">{t("tools.title")}</h1>
          <p className="set-lede">{t("tools.lede")}</p>
        </div>
      </div>

      {error !== null && <div className="set-error">{error}</div>}

      <div className="set-rows">
        {shells.length > 0 && (
          <Row name={t("tools.shell")} hint={t("tools.shell_hint")}>
            <Segments<ShellChoice>
              label={t("tools.shell")}
              value={prefs.shell}
              options={(["auto", ...shells] as ShellChoice[]).map((value) => ({
                value,
                label: t(SHELL_NAME[value]),
              }))}
              onChange={(shell) => onPatch({ shell })}
            />
          </Row>
        )}

        <Row name={t("tools.robots")} hint={t("tools.robots_hint")}>
          <Switch
            on={prefs.respectRobots}
            label={t("tools.robots")}
            onChange={(respectRobots) => onPatch({ respectRobots })}
          />
        </Row>
      </div>

      <section className="prov-section">
        <span className="prov-label">{t("tools.search_key")}</span>
        <div className="prov-hint">{t("tools.search_key_hint")}</div>

        {source === "keychain" && (
          <div className="prov-source prov-source-ok">
            <Check size={12} weight="bold" /> {t("prov.key_source_keychain")}
          </div>
        )}
        {source === "environment" && (
          <div className="prov-source prov-source-ok">
            <Check size={12} weight="bold" /> {t("prov.key_source_environment")}
          </div>
        )}
        {source === "none" && (
          <div className="prov-source prov-source-warn">{t("prov.key_source_none")}</div>
        )}

        <div className="prov-row">
          <input
            className="prov-input prov-input-mono prov-key-input"
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={value}
            aria-label={t("tools.search_key")}
            onChange={(e) => setValue(e.target.value)}
          />
          <button
            type="button"
            className="btn btn-small"
            disabled={busy || value.trim().length === 0}
            onClick={save}
          >
            {t("prov.key_save")}
          </button>
          {source === "keychain" && (
            <button
              type="button"
              className="btn btn-small btn-danger"
              disabled={busy}
              onClick={forget}
            >
              {t("prov.key_forget")}
            </button>
          )}
        </div>
      </section>
    </>
  );
}
