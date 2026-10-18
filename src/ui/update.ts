/**
 * The update check as the screens see it: what the last answer was, whether a
 * check is running now, and why the last one failed.
 *
 * One answer per application, not per screen. The shell keeps it and
 * broadcasts it (`update.rs`), so the sidebar card and the About
 * screen read the same fact, whichever of them started the check.
 */

import { useCallback, useEffect, useState } from "react";

import { checkForUpdates, onUpdate, updateStatus, type UpdateCheck } from "../core/client";

export type UpdateView = {
  /** `null` until a check has finished. */
  check: UpdateCheck | null;
  checking: boolean;
  /** A code from the shell (`offline`, `rate_limited`, `status:502`, ...). */
  error: string | null;
  /** When the answer on screen arrived, in this session. */
  checkedAt: Date | null;
  run: () => void;
};

export function useUpdate(): UpdateView {
  const [check, setCheck] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [checkedAt, setCheckedAt] = useState<Date | null>(null);

  useEffect(() => {
    let live = true;
    let off: (() => void) | null = null;

    void updateStatus().then(
      (answer) => {
        if (live && answer !== null) setCheck((now) => now ?? answer);
      },
      () => undefined,
    );
    void onUpdate((answer) => {
      if (!live) return;
      setCheck(answer);
      setError(null);
      setCheckedAt(new Date());
    }).then((unlisten) => {
      if (live) off = unlisten;
      else unlisten();
    });

    return () => {
      live = false;
      off?.();
    };
  }, []);

  const run = useCallback(() => {
    setChecking(true);
    setError(null);
    checkForUpdates().then(
      (answer) => {
        setCheck(answer);
        setCheckedAt(new Date());
        setChecking(false);
      },
      (e: unknown) => {
        setError(typeof e === "string" ? e : "offline");
        setChecking(false);
      },
    );
  }, []);

  return { check, checking, error, checkedAt, run };
}

/** The words for a failure code, as a dictionary key. */
export function updateErrorKey(code: string) {
  if (code === "offline") return "about.update_offline" as const;
  if (code === "rate_limited") return "about.update_rate_limited" as const;
  return "about.update_failed" as const;
}
