import { useCallback, useEffect, useRef, useState } from "react";
import { globalModels, saveGlobalModels, type ModelDefaults } from "../../core/client";

export function useModelDefaults(onChanged: () => void) {
  const [values, setValues] = useState<ModelDefaults | null>(null);
  const current = useRef<ModelDefaults | null>(null);
  const writes = useRef(Promise.resolve());
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void globalModels().then((next) => {
      if (live) { current.current = next; setValues(next); }
    }, (e: unknown) => live && setError(String(e)));
    return () => { live = false; };
  }, []);
  const patch = useCallback((change: Partial<ModelDefaults>) => {
    if (!current.current) return;
    const next = { ...current.current, ...change };
    current.current = next;
    setValues(next);
    // Navigation cannot cancel pending edits, and older writes cannot win.
    writes.current = writes.current.then(async () => {
      try { await saveGlobalModels(next); setError(null); onChanged(); }
      catch (e) { setError(String(e)); }
    });
  }, [onChanged]);
  return { values, error, patch };
}
