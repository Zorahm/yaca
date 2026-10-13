/**
 * Draft state and command calls behind `ProviderForm`.
 *
 * The form is presentation; everything mutable lives here. The key handling
 * follows §0 literally: the key field is write-only, the value goes to the
 * keychain under the provider's id and is never kept in the draft, in an
 * error message or anywhere else.
 *
 * Whether the key section exists at all is computed locally from the draft
 * (`needsKey`, the same rule the core applies to the saved spec): a provider
 * whose headers never mention `{key}` gets neither a required key field nor
 * a warning about a missing key (§10).
 *
 * Models are asked for, not typed, and mostly not asked for from here. The
 * shell asks on its own the moment a key is stored (`set_api_key`) and when a
 * provider is saved without a list (`save_provider`), and this hook only takes
 * the answer. What is left here is the button: `ask()` sends the draft, so a
 * lookup after an edit of the address goes to the new one.
 *
 * A new provider has no id until it is saved (`newProviderId`), so it has
 * nothing to keep a key under and nothing to be asked as. Its key travels
 * with the save, and the shell asks it then.
 */

import { useCallback, useEffect, useMemo, useState } from "react";

import {
  apiKeySource,
  discoverModels,
  forgetApiKey,
  setApiKey,
  type Asked,
  type ModelInfo,
  type ProviderSpec,
} from "../../core/client";
import {
  draftProblems,
  draftToSpec,
  needsKey,
  type HeaderDraft,
  type KeyState,
  type ProviderDraft,
} from "./provider";

/** The provider's models in what the shell answered, or `null` if it is not there. */
function modelsIn(asked: Asked, id: string): Array<ModelInfo> | null {
  return asked.providers.list.find((p) => p.id === id)?.models ?? null;
}

export function useProviderForm({
  draft: initial,
  isNew,
  taken,
  onSave,
  onSaved,
}: {
  draft: ProviderDraft;
  /** A new provider is not in the registry, so the shell cannot be asked about its key yet. */
  isNew: boolean;
  /** The ids already in the list: a new provider's own must not land on one. */
  taken: ReadonlyArray<string>;
  /** Returns whether the spec was saved; the form closes only on success. */
  onSave: (spec: ProviderSpec) => Promise<boolean>;
  onSaved: () => void;
}) {
  const [draft, setDraft] = useState<ProviderDraft>(initial);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // --- models --------------------------------------------------------------

  const [modelsBusy, setModelsBusy] = useState(false);
  const [modelsError, setModelsError] = useState<string | null>(null);

  /**
   * Ask the provider for its list.
   *
   * Asked against the draft rather than the saved spec: the address is the
   * thing people edit, and a lookup after an edit has to go to the new one. An
   * empty answer stays empty — "no models" is an answer too, and keeping the
   * previous list over it would show what the provider no longer serves.
   */
  const ask = useCallback(async (current: ProviderDraft) => {
    if (!current.id.trim() || !current.baseUrl.trim()) return;
    setModelsBusy(true);
    setModelsError(null);
    try {
      const models = await discoverModels({
        id: current.id.trim(),
        displayName: current.displayName.trim() || current.id.trim(),
        protocol: current.protocol,
        baseUrl: current.baseUrl.trim(),
        headers: Object.fromEntries(
          current.headers
            .map((header) => [header.name.trim(), header.value])
            .filter(([name]) => name !== ""),
        ),
        models: [],
      });
      setDraft((now) => ({ ...now, models }));
    } catch (e) {
      setModelsError(String(e));
    } finally {
      setModelsBusy(false);
    }
  }, []);

  const askForModels = useCallback(() => void ask(draft), [ask, draft]);

  // --- key section ---------------------------------------------------------

  const [keyValue, setKeyValue] = useState("");
  const [keyBusy, setKeyBusy] = useState(false);
  const [keyError, setKeyError] = useState<string | null>(null);
  // A new provider is never looked up, so its answer is known from the start:
  // there is nothing to say yet.
  const [askedSource, setAskedSource] = useState<KeyState>({
    kind: isNew ? "unknown" : "loading",
  });

  const loadSource = useCallback(() => {
    if (isNew) return;
    apiKeySource(initial.id).then(
      (source) => setAskedSource({ kind: source }),
      // A rejected lookup means there is nothing to say yet; "unknown" is
      // honest where "no key" would be a guess.
      () => setAskedSource({ kind: "unknown" }),
    );
  }, [isNew, initial.id]);

  useEffect(() => {
    loadSource();
  }, [loadSource]);

  // The saved provider's key source says nothing about the draft being edited:
  // whether this draft needs a key is computed from its own headers.
  const keyState: KeyState = needsKey(draft.protocol, draft.headers)
    ? askedSource
    : { kind: "not_needed" };

  async function saveKey() {
    const value = keyValue.trim();
    const id = draft.id.trim();
    if (!value || !id || keyBusy) return;
    setKeyBusy(true);
    setKeyError(null);
    setModelsError(null);
    try {
      const asked = await setApiKey(value, id);
      // The value has done its job; keeping it in the field would keep a
      // secret on screen for no reason.
      setKeyValue("");
      loadSource();
      // The key was the thing missing to ask with (§10), and the shell has
      // asked already. Its refusal belongs to the models, not to the key:
      // the key is stored either way.
      const models = modelsIn(asked, id);
      if (asked.pollError !== null) setModelsError(asked.pollError);
      else if (models !== null) setDraft((now) => ({ ...now, models }));
    } catch (e) {
      setKeyError(String(e));
    } finally {
      setKeyBusy(false);
    }
  }

  async function forgetKey() {
    const id = draft.id.trim();
    // Offered only where the source is known to be the keychain: an
    // environment variable cannot be forgotten from here, and a button
    // that leaves the state untouched would lie about it (§0).
    if (keyState.kind !== "keychain" || keyBusy) return;
    setKeyBusy(true);
    setKeyError(null);
    try {
      await forgetApiKey(id);
      loadSource();
    } catch (e) {
      setKeyError(String(e));
    } finally {
      setKeyBusy(false);
    }
  }

  // --- draft fields ----------------------------------------------------------

  const problems = useMemo(() => draftProblems(draft), [draft]);

  const patch = useCallback((next: Partial<ProviderDraft>) => {
    setDraft((now) => ({ ...now, ...next }));
  }, []);

  const patchHeader = useCallback((index: number, next: Partial<HeaderDraft>) => {
    setDraft((now) => ({
      ...now,
      headers: now.headers.map((header, at) =>
        at === index ? { ...header, ...next } : header,
      ),
    }));
  }, []);

  const addHeader = useCallback(() => {
    setDraft((now) => ({
      ...now,
      headers: [...now.headers, { name: "", value: "" }],
    }));
  }, []);

  const removeHeader = useCallback((index: number) => {
    setDraft((now) => ({
      ...now,
      headers: now.headers.filter((_, at) => at !== index),
    }));
  }, []);

  // --- submit ----------------------------------------------------------------

  async function submit() {
    let spec = draftToSpec(draft, taken);
    if (spec === null || busy) return;
    setBusy(true);
    setError(null);
    try {
      if (keyValue.trim()) {
        // The key goes first: if the keychain refuses, nothing is saved
        // half-way and the error points at the only thing that failed.
        const asked = await setApiKey(keyValue.trim(), spec.id);
        setKeyValue("");
        // A provider already in the list was asked with the new key just now,
        // and the draft's list is from before it: saving that over the answer
        // would undo the lookup. A new provider is not in the list yet —
        // nobody was asked, and the save below asks.
        const models = modelsIn(asked, spec.id);
        if (asked.pollError === null && models !== null) spec = { ...spec, models };
      }
      if (await onSave(spec)) onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return {
    draft,
    patch,
    patchHeader,
    addHeader,
    removeHeader,
    askForModels,
    modelsBusy,
    modelsError,
    problems,
    busy,
    error,
    submit,
    // Key section
    keyValue,
    setKeyValue,
    keyState,
    keyBusy,
    keyError,
    saveKey,
    forgetKey,
  };
}
