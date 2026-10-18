/**
 * Project settings in the §10 window: read, edit, write.
 *
 * Written at once, on every edit. The section has no "Save" button: it would let the
 * window show one thing while `.yaca/settings.json` held another (§0). The price is
 * a write on every click, and it is small: the file is tiny and rarely clicked.
 *
 * A write error does not roll the screen back. The chosen value stays on screen, and
 * next to it a row with the refusal stands: a window that jumped back to the old value
 * and at the same time said "not written" would tell two stories at once.
 *
 * A broken file arrives as a refusal from the core and stays one: an empty list of
 * rules in place of a typo is an interface that lied that there were no rules.
 */

import { useCallback, useEffect, useState } from "react";

import { projectSettings, saveProjectSettings, type Settings } from "../../core/client";

export type ProjectSettings = {
  /** `null` means still reading, or the read failed; see `error`. */
  settings: Settings | null;
  error: string | null;
  /** A whole-settings edit: takes the current settings, returns the new ones. */
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
