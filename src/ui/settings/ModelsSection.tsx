import { useState } from "react";
import type { ModelDefaults, ModelInfo } from "../../core/client";
import { t } from "../../i18n";
import { chatModels, modelKey, modelLabel } from "../modelSearch";
import { Row, Select } from "./Row";
import { RoutingSection } from "./RoutingSection";
import { ModelCatalog } from "./ModelCatalog";

const NONE = "<none>";

export function ModelChoices({ values, models, inherited = false, onPatch }: {
  values: ModelDefaults;
  models: ModelInfo[];
  inherited?: boolean;
  onPatch: (change: Partial<ModelDefaults>) => void;
}) {
  const choices = chatModels(models);
  const selected = choices.find((m) => m.id === values.model && (!values.provider || m.providerId === values.provider));
  const mainValue = selected ? modelKey(selected) : values.model ? `${values.provider ?? ""}\n${values.model}` : NONE;
  const mainOptions = choices.map((m) => ({ value: modelKey(m), label: `${modelLabel(m)} · ${m.providerId}` }));
  if (mainValue !== NONE && !mainOptions.some((o) => o.value === mainValue)) mainOptions.unshift({ value: mainValue, label: values.model! });
  return <div className="set-rows">
    <Row name={t("models.default")} hint={t("models.default_hint")}>
      <Select label={t("models.default")} value={mainValue}
        options={[{ value: NONE, label: t(inherited ? "settings.inherit" : "models.first_configured") }, ...mainOptions]}
        onChange={(value) => {
          const model = choices.find((m) => modelKey(m) === value);
          onPatch({ model: model?.id ?? null, provider: model?.providerId ?? null });
        }} />
    </Row>
    {(["compactModel", "subagentModel"] as const).map((field) => {
      const label = t(field === "compactModel" ? "models.compact" : "models.subagent");
      const options = Array.from(new Map(choices.map((m) => [m.id, { value: m.id, label: modelLabel(m) }])).values());
      const value = values[field];
      if (value && !options.some((o) => o.value === value)) options.unshift({ value, label: value });
      return <Row key={field} name={label} hint={t(field === "compactModel" ? "models.compact_hint" : "models.subagent_hint")}>
        <Select label={label} value={value ?? NONE}
          options={[{ value: NONE, label: t(inherited ? "settings.inherit" : "models.compact_same") }, ...options]}
          onChange={(next) => onPatch({ [field]: next === NONE ? null : next })} />
      </Row>;
    })}
  </div>;
}

export function ModelsSection({ values, models, onPatch, onChanged }: {
  values: ModelDefaults;
  models: ModelInfo[];
  onPatch: (change: Partial<ModelDefaults>) => void;
  onChanged: () => void;
}) {
  const local = chatModels(models).filter((m) => /^(?:[a-z]:[/\\]|[/\\]|~[/\\])/i.test(m.id) || /\.(?:gguf|ggml|safetensors|bin)$/i.test(m.id));
  return <>
    <div className="set-head"><div>
      <h1 className="set-title">{t("settings.models")}</h1>
      <p className="set-lede">{t("settings.models_hint")}</p>
    </div></div>
    <h2 className="set-subname">{t("settings.defaults")}</h2>
    <ModelChoices values={values} models={models} onPatch={onPatch} />
    <RoutingSection models={models} onChanged={onChanged} />
    <section className="set-local-models">
      <h2 className="set-subname">{t("settings.local_models")}</h2>
      <p className="set-subhint">{t("settings.local_models_hint")}</p>
      {local.length === 0 && <p className="set-empty">{t("settings.local_none")}</p>}
      {local.map((model) => <LocalName key={modelKey(model)} model={model}
        onSave={(name) => {
          const names = { ...values.names };
          if (name) names[modelKey(model)] = name;
          else delete names[modelKey(model)];
          onPatch({ names });
        }} />)}
    </section>
    <ModelCatalog onChanged={onChanged} />
  </>;
}

function LocalName({ model, onSave }: { model: ModelInfo; onSave: (name: string) => void }) {
  const [name, setName] = useState(modelLabel(model));
  return <form className="set-local-name" onSubmit={(event) => { event.preventDefault(); onSave(name.trim()); }}>
    <code title={model.id}>{model.providerId} · {model.id}</code>
    <input className="prov-input" aria-label={t("settings.model_name")} value={name} onChange={(e) => setName(e.target.value)} />
    <button type="submit" className="btn btn-small">{t("project.save")}</button>
  </form>;
}
