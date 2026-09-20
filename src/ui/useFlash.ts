/**
 * Короткая отметка «получилось» на кнопке.
 *
 * Живёт отдельно от `useCopy`, потому что читателей у неё двое: буфер обмена
 * и сохранение файла (§9). Таймер сброса в двух копиях разошёлся бы на первой
 * же правке.
 */

import { useCallback, useEffect, useRef, useState } from "react";

export function useFlash(resetMs = 1200): { shown: boolean; flash: () => void } {
  const [shown, setShown] = useState(false);
  const timer = useRef(0);

  const flash = useCallback(() => {
    setShown(true);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setShown(false), resetMs);
  }, [resetMs]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  return { shown, flash };
}
