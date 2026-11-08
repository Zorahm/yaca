import type { ProviderSpec } from "../../core/client";
import { t } from "../../i18n";
import { ModelsList } from "./ModelsList";
import { draftFromSpec } from "./provider";
import { providerAddress } from "./providerStatus";
import { useProviderForm } from "./useProviderForm";

export function ModelCatalog({ spec, checkedAt, onSave }: {
  spec: ProviderSpec;
  checkedAt?: number;
  onSave: (spec: ProviderSpec) => Promise<boolean>;
}) {
  const form = useProviderForm({ draft: draftFromSpec(spec), isNew: false, taken: [], onSave, onSaved: () => {}, checkedAt });
  return <section aria-label={t("prov.form_models")}>
    <h2 className="set-subname">{t("prov.form_models")}</h2>
    <ModelsList models={form.draft.models} busy={form.modelsBusy || form.keyBusy}
      answered={form.modelsAnswered} error={form.modelsError}
      isLocal={providerAddress(spec.baseUrl).local}
      onAsk={form.askForModels} onModelsChange={form.changeModels}
      saveState={form.modelsSaveState} checkedAt={form.modelsCheckedAt}
      canAsk={["keychain", "environment", "not_needed"].includes(form.keyState.kind)} />
  </section>;
}
