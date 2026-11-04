/**
 * The provider's model list (§10).
 *
 * Shows models from the provider's API, manually added models, and manual overrides.
 * Data states: from API, manually edited, unknown.
 * Effective values: override ?? fetched ?? default.
 */

import {
  ArrowClockwise,
  CaretDown,
  CaretRight,
  Info,
  MagnifyingGlass,
  Plus,
  Question,
  X,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import { ModelRow } from "./ModelRow";
import { pollAge } from "./providerStatus";

import type { ModelInfo } from "../../core/client";
import { t, tp } from "../../i18n";
import {
  DEFAULT_CONTEXT_MAX,
  DEFAULT_MAX_OUTPUT,
  extractVendor,
  getModelPriceStatus,
  parsePriceInput,
  parseTokensInput,
  resetModelToProvider,
  type ProviderModelItem,
} from "./provider";

type FilterType = "all" | "enabled" | "edited" | "no_price";

export function ModelsList({
  models,
  busy,
  answered,
  error,
  isLocal = false,
  onAsk,
  onModelsChange,
  saveState = "idle",
  canAsk = true,
  checkedAt,
}: {
  models: ModelInfo[];
  busy: boolean;
  answered: boolean;
  error: string | null;
  isLocal?: boolean;
  onAsk: () => void;
  onModelsChange: (models: ModelInfo[]) => void;
  saveState?: "idle" | "saving" | "saved" | "error";
  canAsk?: boolean;
  checkedAt?: number;
}) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(timer);
  }, []);
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<FilterType>("all");
  const [collapsedVendors, setCollapsedVendors] = useState<Set<string>>(new Set());
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [showAddForm, setShowAddForm] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [hasChanges, setHasChanges] = useState(false);

  // New model form draft
  const [addId, setAddId] = useState("");
  const [addName, setAddName] = useState("");
  const [addContext, setAddContext] = useState("1 000 000");
  const [addOutput, setAddOutput] = useState("128 000");
  const [addPriceIn, setAddPriceIn] = useState("");
  const [addPriceOut, setAddPriceOut] = useState("");
  const [addTools, setAddTools] = useState(true);
  const [addVision, setAddVision] = useState(false);
  const [addReasoning, setAddReasoning] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);

  const copyId = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    void navigator.clipboard.writeText(id);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1500);
  };

  const toggleVendor = (vendor: string) => {
    setCollapsedVendors((now) => {
      const next = new Set(now);
      if (next.has(vendor)) next.delete(vendor);
      else next.add(vendor);
      return next;
    });
  };

  const updateModel = (updated: ProviderModelItem) => {
    setHasChanges(true);
    const nextModels = models.map((m) => (m.id === updated.id ? updated : m));
    onModelsChange(nextModels);
  };

  const resetModel = (id: string) => {
    setHasChanges(true);
    const target = models.find((m) => m.id === id) as ProviderModelItem | undefined;
    if (!target) return;
    const reset = resetModelToProvider(target);
    updateModel(reset);
  };

  const submitAddModel = (e: React.FormEvent) => {
    e.preventDefault();
    const id = addId.trim();
    const name = addName.trim() || id;
    if (!id) {
      setAddError(t("prov.error_name"));
      return;
    }
    if (models.some((m) => m.id === id)) {
      setAddError(t("prov.error_header_dup", { name: id }));
      return;
    }

    const context = parseTokensInput(addContext);
    const output = parseTokensInput(addOutput);
    const pIn = parsePriceInput(addPriceIn);
    const pOut = parsePriceInput(addPriceOut);
    if ((addContext.trim() && context === null) || (addOutput.trim() && output === null)) {
      setAddError(t("prov.error_context", { name }));
      return;
    }
    if ((addPriceIn.trim() && pIn === null) || (addPriceOut.trim() && pOut === null)) {
      setAddError(t("prov.error_price", { name }));
      return;
    }
    const ctx = context ?? DEFAULT_CONTEXT_MAX;
    const out = output ?? DEFAULT_MAX_OUTPUT;

    // Every number typed here is a hand edit, and is recorded as one: the
    // provider does not list this model, so there is nothing under it, and a
    // refresh must not turn the row into the defaults.
    const newModel: ProviderModelItem = {
      id,
      providerId: "",
      displayName: name,
      contextMax: ctx,
      maxOutput: out,
      pricing: { input: pIn ?? 0, output: pOut ?? 0 },
      pricingByHand: pIn !== null || pOut !== null,
      supportsEffort: addReasoning,
      supportsThinking: addReasoning,
      contextSource: context === null ? "default_guess" : "hand",
      supportsImages: addVision,
      byHand: true,
      enabled: true,
      overrides: {
        contextMax: context ?? undefined,
        maxOutput: output ?? undefined,
        inputPrice: pIn ?? undefined,
        outputPrice: pOut ?? undefined,
        supportsEffort: addReasoning,
      },
      supportedParameters: addTools ? ["tools"] : [],
      inputModalities: addVision ? ["text", "image"] : ["text"],
    };

    setHasChanges(true);
    const nextModels = [...models, newModel];
    onModelsChange(nextModels);

    // Reset add form
    setAddId("");
    setAddName("");
    setAddContext("1 000 000");
    setAddOutput("128 000");
    setAddPriceIn("");
    setAddPriceOut("");
    setAddTools(true);
    setAddVision(false);
    setAddReasoning(false);
    setAddError(null);
    setShowAddForm(false);
    setExpandedId(id);
  };

  // Filter models
  const filteredModels = useMemo(() => {
    return models.filter((m) => {
      const item = m as ProviderModelItem;
      // Search query
      if (search.trim()) {
        const q = search.toLowerCase().trim();
        const matchesName = m.displayName.toLowerCase().includes(q);
        const matchesId = m.id.toLowerCase().includes(q);
        if (!matchesName && !matchesId) return false;
      }
      // Filter tab
      if (filter === "enabled") {
        if (item.enabled === false) return false;
      } else if (filter === "edited") {
        const isEdited =
          m.contextSource === "hand" ||
          m.pricingByHand ||
          item.overrides !== undefined ||
          m.byHand;
        if (!isEdited) return false;
      } else if (filter === "no_price") {
        const status = getModelPriceStatus(m, isLocal);
        if (status.kind !== "unknown" && !(status.kind === "known" && (status.input === undefined || status.output === undefined))) return false;
      }
      return true;
    });
  }, [models, search, filter, isLocal]);

  // Group by vendor if >= 2 vendors exist
  const vendorGroups = useMemo(() => {
    const vendors = new Set<string>();
    for (const m of filteredModels) {
      const v = extractVendor(m.id);
      if (v) vendors.add(v);
    }

    if (vendors.size < 2) {
      return null;
    }

    const groups: Array<{ vendor: string; list: ModelInfo[] }> = [];
    const grouped = new Map<string, ModelInfo[]>();

    for (const m of filteredModels) {
      const v = extractVendor(m.id) || "other";
      if (!grouped.has(v)) grouped.set(v, []);
      grouped.get(v)!.push(m);
    }

    for (const [vendor, list] of grouped) {
      groups.push({ vendor, list });
    }
    return groups;
  }, [filteredModels]);

  return (
    <div className="prov-models-container">
      {/* Top Header Bar */}
      <div className="prov-models-head-bar">
        <div className="prov-models-explanation-row">
          <span>{t("prov.models_explanation")}</span>
          <button
            type="button"
            className="prov-icon-btn"
            onClick={() => setShowHelp(!showHelp)}
            title={t("prov.models_help")}
            aria-label={t("prov.models_help")}
          >
            <Question size={13} />
          </button>
        </div>

        <div className="prov-models-status-group">
          {error ? (
            <>
              <span className="prov-poll-error">{t("prov.poll_error_note")}</span>
              <button
                type="button"
                className="btn btn-small btn-row"
                onClick={onAsk}
                disabled={busy || !canAsk}
              >
                <ArrowClockwise size={13} className={busy ? "spin" : ""} /> {t("prov.retry")}
              </button>
            </>
          ) : (
            <>
              <span className="prov-poll-status">
                {models.length === 0
                  ? t("prov.models_none")
                  : answered
                    ? t("prov.poll_status_updated", {
                        time: checkedAt === undefined ? t("prov.just_now") : pollAge(checkedAt, now),
                        count: tp("prov.models_count", models.length),
                      })
                    : tp("prov.models_count", models.length)}
              </span>
              <button
                type="button"
                className="btn btn-small btn-row"
                onClick={onAsk}
                disabled={busy || !canAsk}
              >
                <ArrowClockwise size={13} className={busy ? "spin" : ""} />{" "}
                {busy ? t("prov.models_asking") : t("prov.models_ask")}
              </button>
            </>
          )}

          <button
            type="button"
            className="btn btn-small btn-row"
            onClick={() => setShowAddForm(!showAddForm)}
          >
            <Plus size={13} /> {t("prov.add_model_manual")}
          </button>
        </div>
      </div>

      {/* Expandable Help */}
      {showHelp && (
        <div className="prov-help-box">
          <p>{t("prov.form_models_hint")}</p>
          <p>{t("prov.context_hint")}</p>
          <p>{t("prov.price_hint")}</p>
        </div>
      )}

      {/* Changes Notification Banner */}
      {hasChanges && (
        <div className="prov-notice-banner">
          <Info size={14} weight="bold" />
          <span>{t("prov.models_notice_reopen")}</span>
          {saveState !== "idle" && <span className="prov-saved-badge" role="status">
            {t(saveState === "saved" ? "prov.saved_indicator" : saveState === "saving" ? "prov.saving_indicator" : "prov.save_failed")}
          </span>}
        </div>
      )}

      {/* Manual Model Form Modal/Card */}
      {showAddForm && (
        <form className="prov-add-model-card" onSubmit={submitAddModel}>
          <div className="prov-add-model-head">
            <span className="prov-add-model-title">{t("prov.add_model_title")}</span>
            <button
              type="button"
              className="prov-icon-btn"
              onClick={() => setShowAddForm(false)}
            >
              <X size={14} />
            </button>
          </div>

          <div className="prov-add-model-grid">
            <label className="prov-field">
              <span className="prov-label">{t("prov.add_model_id")} *</span>
              <input
                className="prov-input prov-input-mono"
                required
                placeholder="vendor/model-name"
                value={addId}
                onChange={(e) => setAddId(e.target.value)}
              />
            </label>

            <label className="prov-field">
              <span className="prov-label">{t("prov.form_name")} *</span>
              <input
                className="prov-input"
                placeholder={t("prov.form_name")}
                value={addName}
                onChange={(e) => setAddName(e.target.value)}
              />
            </label>

            <label className="prov-field">
              <span className="prov-label">{t("prov.table_context")}</span>
              <input
                className="prov-input prov-input-mono"
                placeholder="1M / 1 000 000"
                value={addContext}
                onChange={(e) => setAddContext(e.target.value)}
              />
            </label>

            <label className="prov-field">
              <span className="prov-label">{t("prov.form_model_output")}</span>
              <input
                className="prov-input prov-input-mono"
                placeholder="128k / 128 000"
                value={addOutput}
                onChange={(e) => setAddOutput(e.target.value)}
              />
            </label>

            <label className="prov-field">
              <span className="prov-label">{t("prov.price_input")}</span>
              <input
                className="prov-input prov-input-mono"
                placeholder="$5"
                value={addPriceIn}
                onChange={(e) => setAddPriceIn(e.target.value)}
              />
            </label>

            <label className="prov-field">
              <span className="prov-label">{t("prov.price_output")}</span>
              <input
                className="prov-input prov-input-mono"
                placeholder="$25"
                value={addPriceOut}
                onChange={(e) => setAddPriceOut(e.target.value)}
              />
            </label>
          </div>

          <div className="prov-add-model-checks">
            <label className="prov-check-label">
              <input
                type="checkbox"
                checked={addTools}
                onChange={(e) => setAddTools(e.target.checked)}
              />
              <span>{t("prov.cap_tools")}</span>
            </label>
            <label className="prov-check-label">
              <input
                type="checkbox"
                checked={addVision}
                onChange={(e) => setAddVision(e.target.checked)}
              />
              <span>{t("prov.cap_vision")}</span>
            </label>
            <label className="prov-check-label">
              <input
                type="checkbox"
                checked={addReasoning}
                onChange={(e) => setAddReasoning(e.target.checked)}
              />
              <span>{t("prov.cap_reasoning")}</span>
            </label>
          </div>

          {addError && <div className="prov-problems">{addError}</div>}

          <div className="prov-add-model-foot">
            <button
              type="button"
              className="btn btn-small"
              onClick={() => setShowAddForm(false)}
            >
              {t("prov.form_cancel")}
            </button>
            <button type="submit" className="btn btn-small btn-accent">
              {t("prov.add_model_submit")}
            </button>
          </div>
        </form>
      )}

      {/* Search and Filters Bar */}
      <div className="prov-controls-bar">
        <div className="prov-search-box">
          <MagnifyingGlass size={13} />
          <input
            className="prov-search-input"
            type="text"
            placeholder={t("prov.search_models")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          {search && (
            <button
              type="button"
              className="prov-icon-btn"
              onClick={() => setSearch("")}
            >
              <X size={12} />
            </button>
          )}
        </div>

        <div className="prov-filters">
          {(
            [
              ["all", t("prov.filter_all")],
              ["enabled", t("prov.filter_enabled")],
              ["edited", t("prov.filter_edited")],
              ["no_price", t("prov.filter_no_price")],
            ] as const
          ).map(([key, label]) => (
            <button
              type="button"
              key={key}
              className={`prov-filter-pill ${filter === key ? "active" : ""}`}
              onClick={() => setFilter(key)}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Models Table */}
      <div className="prov-table">
        <div className="prov-table-head">
          <span />
          <span>{t("prov.table_model")}</span>
          <span>{t("prov.table_context")}</span>
          <span>{t("prov.table_pricing")}</span>
          <span>{t("prov.table_capabilities")}</span>
          <span />
        </div>

        {filteredModels.length === 0 ? (
          <div className="prov-table-empty">
            {models.length === 0 ? t("prov.models_none") : t("prov.models_none")}
          </div>
        ) : vendorGroups ? (
          vendorGroups.map((group) => {
            const isCollapsed = collapsedVendors.has(group.vendor);
            return (
              <div key={group.vendor} className="prov-vendor-section">
                <div
                  className="prov-vendor-header"
                  onClick={() => toggleVendor(group.vendor)}
                >
                  {isCollapsed ? <CaretRight size={12} /> : <CaretDown size={12} />}
                  <span className="prov-vendor-name">{group.vendor}</span>
                  <span className="prov-vendor-count">({group.list.length})</span>
                </div>
                {!isCollapsed &&
                  group.list.map((model) => (
                    <ModelRow
                      key={model.id}
                      model={model as ProviderModelItem}
                      isExpanded={expandedId === model.id}
                      isLocal={isLocal}
                      copiedId={copiedId}
                      onToggleExpand={() =>
                        setExpandedId(expandedId === model.id ? null : model.id)
                      }
                      onCopyId={copyId}
                      onUpdate={updateModel}
                      onReset={() => resetModel(model.id)}
                    />
                  ))}
              </div>
            );
          })
        ) : (
          filteredModels.map((model) => (
            <ModelRow
              key={model.id}
              model={model as ProviderModelItem}
              isExpanded={expandedId === model.id}
              isLocal={isLocal}
              copiedId={copiedId}
              onToggleExpand={() =>
                setExpandedId(expandedId === model.id ? null : model.id)
              }
              onCopyId={copyId}
              onUpdate={updateModel}
              onReset={() => resetModel(model.id)}
            />
          ))
        )}
      </div>
    </div>
  );
}
