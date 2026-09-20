/**
 * Копирование в буфер обмена с коротким «Скопировано».
 *
 * Буфер может быть недоступен: молчать об этом честнее, чем рисовать успех
 * над неудачей (§0). Одна на все панели и блок вывода — логика с таймером
 * сброса не должна жить в трёх копиях; сам таймер — в `useFlash`.
 */

import { useCallback } from "react";

import { useFlash } from "./useFlash";

export function useCopy(resetMs = 1200): {
  copied: boolean;
  copy: (text: string) => void;
} {
  const { shown, flash } = useFlash(resetMs);

  const copy = useCallback(
    (text: string) => {
      void navigator.clipboard.writeText(text).then(
        flash,
        // Недоступность буфера не исправить отсюда — и нечего показывать.
        () => {},
      );
    },
    [flash],
  );

  return { copied: shown, copy };
}
