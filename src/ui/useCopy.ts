/**
 * Copy to the clipboard with a short "Copied".
 *
 * The clipboard may be unavailable: it is more honest to say nothing than to
 * draw success over a failure (§0). One hook for all the panels and the output
 * block: the reset-timer logic must not live in three copies, and the timer
 * itself lives in `useFlash`.
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
        // An unavailable clipboard cannot be fixed from here, and there is nothing to show.
        () => {},
      );
    },
    [flash],
  );

  return { copied: shown, copy };
}
