/**
 * Инструменты (§10).
 *
 * Сегодня в разделе две настоящие строки: `robots.txt` и ключ веб-поиска.
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
  searchKeySource,
  SEARCH_KEY_ITEM,
  setApiKey,
  type KeySource,
  type Preferences,
} from "../../core/client";
import { t } from "../../i18n";
import { Row, Switch } from "./Row";

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
