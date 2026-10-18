/**
 * A short "it worked" mark on a button.
 *
 * Lives apart from `useCopy` because it has two readers: the clipboard and file
 * saving (§9). A reset timer kept in two copies would diverge on the first edit.
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
