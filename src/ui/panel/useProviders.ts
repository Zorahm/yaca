/**
 * Panel data and command calls for the providers panel (§10).
 *
 * Every shell command returns the whole `Providers` — the same "state leaves
 * the core whole" rule the event stream follows — so one list is kept and
 * replaced by whatever a call returns. `list()` is the call that can fail on
 * a broken `providers.json`, and the failure is kept, not swallowed: showing
 * an empty list would claim "no providers" while the truth is that the file
 * did not parse (§0).
 */

import { useCallback, useEffect, useState } from "react";

import {
  providers as api,
  type ProviderSpec,
  type Providers,
} from "../../core/client";

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

  const save = useCallback(
    (spec: ProviderSpec) => run(() => api.save(spec)),
    [run],
  );
  const remove = useCallback((id: string) => run(() => api.remove(id)), [run]);

  return { providers: data, error, save, remove };
}
