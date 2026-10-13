/**
 * Panel data and command calls for the providers panel (§10).
 *
 * Every shell command returns the whole `Providers` — the same "state leaves
 * the core whole" rule the event stream follows — so one list is kept and
 * replaced by whatever a call returns. `list()` is the call that can fail on
 * a broken `providers.json`, and the failure is kept, not swallowed: showing
 * an empty list would claim "no providers" while the truth is that the file
 * did not parse (§0).
 *
 * A save also asks a provider that has no models yet (`save_provider`). Its
 * refusal is not the save failing — the provider is in the file — so the save
 * reports success and the refusal is shown above the list, where the row it
 * is about says "not asked yet".
 */

import { useCallback, useEffect, useState } from "react";

import {
  providers as api,
  type ProviderSpec,
  type Providers,
} from "../../core/client";
import { t } from "../../i18n";

export function useProviders() {
  const [data, setData] = useState<Providers | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    api
      .list()
      .then((next) => {
        if (live) {
          setData(next);
          setError(null);
        }
      })
      .catch((e: unknown) => {
        if (live) setError(String(e));
      });
    return () => {
      live = false;
    };
  }, []);

  const run = useCallback(
    async (action: () => Promise<Providers>): Promise<boolean> => {
      try {
        setData(await action());
        setError(null);
        return true;
      } catch (e) {
        setError(String(e));
        return false;
      }
    },
    [],
  );

  const save = useCallback(async (spec: ProviderSpec): Promise<boolean> => {
    try {
      const asked = await api.save(spec);
      setData(asked.providers);
      setError(
        asked.pollError === null
          ? null
          : t("prov.saved_unasked", {
              name: spec.displayName,
              error: asked.pollError,
            }),
      );
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  }, []);
  const remove = useCallback((id: string) => run(() => api.remove(id)), [run]);

  return { providers: data, error, save, remove };
}
