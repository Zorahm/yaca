/** Provider state comes back whole; lookup failures belong to the provider row. */

import { useCallback, useEffect, useState } from "react";

import {
  providers as api,
  apiKeySource,
  type ProviderSpec,
  type Providers,
} from "../../core/client";
import { t } from "../../i18n";
import type { KeyState } from "./provider";

export function useProviders() {
  const [data, setData] = useState<Providers | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [keySources, setKeySources] = useState<Record<string, KeyState>>({});
  const [pollErrors, setPollErrors] = useState<Record<string, string | undefined>>({});

  const reload = useCallback(async () => {
    try {
      setData(await api.list());
      setPollErrors({});
      setKeySources({});
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    if (data === null) return;
    let live = true;
    void Promise.all(data.list.map(async (spec) => {
      try {
        return [spec.id, { kind: await apiKeySource(spec.id) }] as const;
      } catch {
        return [spec.id, { kind: "unknown" }] as const;
      }
    })).then((sources) => {
      if (live) setKeySources(Object.fromEntries(sources));
    });
    return () => { live = false; };
  }, [data]);

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
      setPollErrors((now) => ({ ...now, [spec.id]: undefined }));
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

  const ask = useCallback(async (spec: ProviderSpec): Promise<void> => {
    try {
      const asked = await api.ask(spec.id);
      setData(asked.providers);
      setPollErrors((now) => ({ ...now, [spec.id]: asked.pollError ?? undefined }));
    } catch (e) {
      setPollErrors((now) => ({ ...now, [spec.id]: String(e) }));
    }
  }, []);

  return { providers: data, error, keySources, pollErrors, reload, save, remove, ask };
}
