/**
 * Настройки проекта в окне §10: прочитать, поправить, записать.
 *
 * Пишется сразу, каждой правкой. Кнопки «Сохранить» у раздела нет: она
 * разрешила бы окну показывать одно, а `.yaca/settings.json` держать другое
 * (§0). Цена — запись на каждый щелчок, и она невелика: файл маленький, а
 * щёлкают в нём редко.
 *
 * Ошибка записи не откатывает экран. Показано остаётся выбранное, а рядом
 * стоит строка с отказом: окно, прыгнувшее обратно на старое значение и
 * одновременно сказавшее «не записать», рассказывало бы две истории сразу.
 *
 * Битый файл приезжает отказом ядра и остаётся отказом: пустой список правил
 * на месте опечатки — это интерфейс, соврав­ший, что правил нет.
 */

import { useCallback, useEffect, useState } from "react";

import { projectSettings, saveProjectSettings, type Settings } from "../../core/client";

export type ProjectSettings = {
  /** `null` — ещё читаем или прочитать не вышло; смотри `error`. */
  settings: Settings | null;
  error: string | null;
  /** Правка целиком: берёт текущие настройки, отдаёт новые. */
  patch: (change: (settings: Settings) => Settings) => void;
};

export function useProjectSettings(project: string | null): ProjectSettings {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (project === null) {
      setSettings(null);
      return;
    }
    let live = true;
    setSettings(null);
    setError(null);
    void projectSettings(project).then(
      (next) => live && setSettings(next),
      (e) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, [project]);

  const patch = useCallback(
    (change: (settings: Settings) => Settings) => {
      if (project === null) return;
      setSettings((now) => {
        if (now === null) return now;
        const next = change(now);
        setError(null);
        void saveProjectSettings(project, next).catch((e: unknown) => setError(String(e)));
        return next;
      });
    },
    [project],
  );

  return { settings, error, patch };
}
