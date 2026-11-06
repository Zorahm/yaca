import { useState } from "react";
import type { ProviderSpec } from "../../core/client";
import { DECISION_PROVIDER_ID } from "../../core/provider";
import { t } from "../../i18n";
import { ModelsList } from "./ModelsList";
import { draftFromSpec } from "./provider";
import { Select } from "./Row";
import { useProviderForm } from "./useProviderForm";
import { useProviders } from "./useProviders";

/** Model facts use the same editor and serialized writes as before the move. */
export function ModelCatalog({ onChanged }: { onChanged: () => void }) {
  const { providers, error, save } = useProviders();
  const [selected, setSelected] = useState<string | null>(null);
  const choices = providers?.list.filter((spec) => spec.id !== DECISION_PROVIDER_ID) ?? [];
  const spec = choices.find((spec) => spec.id === selected) ?? choices.find((spec) => spec.models.length > 0) ?? choices[0];
  return <section className="set-model-catalog">
    <h2 className="set-subname">{t("prov.form_models")}</h2>
    {error && <div className="set-error" role="alert">{error}</div>}
    {spec && <>
      <Select label={t("prov.title")} align="start" value={spec.id}
        options={choices.map((spec) => ({ value: spec.id, label: spec.displayName }))} onChange={setSelected} />
      <ProviderModelEditor key={spec.id} spec={spec} taken={choices.map((spec) => spec.id)}
        checkedAt={providers?.polls[spec.id]?.checkedAt}
        onSave={async (next) => { const saved = await save(next); if (saved) onChanged(); return saved; }} />
    </>}
  </section>;
}

function ProviderModelEditor({ spec, taken, checkedAt, onSave }: {
  spec: ProviderSpec;
  taken: string[];
  checkedAt?: number;
  onSave: (spec: ProviderSpec) => Promise<boolean>;
}) {
  const form = useProviderForm({ draft: draftFromSpec(spec), isNew: false, taken, onSave, onSaved: () => {}, checkedAt });
  return <>
    {form.error && <div className="set-error" role="alert">{form.error}</div>}
    <ModelsList models={form.draft.models} busy={form.modelsBusy || form.keyBusy}
      answered={form.modelsAnswered} error={form.modelsError}
      isLocal={/localhost|127\.0\.0\.1|\[::1\]/.test(spec.baseUrl)}
      onAsk={form.askForModels} onModelsChange={form.changeModels}
      saveState={form.modelsSaveState} checkedAt={form.modelsCheckedAt}
      canAsk={["keychain", "environment", "not_needed"].includes(form.keyState.kind)} />
  </>;
}
