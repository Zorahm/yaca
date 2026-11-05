/** Read-only model summary and the expanded editor share the same effective values. */
import { ArrowClockwise, Check, Copy, Eye, EyeSlash, PencilSimple, Question } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import { tokens } from "../format";
import { editCache, editContext, editEffort, editMaxOutput, editPrice, extractDateAlias,
  formatPrice, formatTokens, getModelCapabilities, getModelPriceStatus, hasEdits, isEdited,
  parsePriceInput, parseTokensInput, resetField, validContextSize, type ProviderModelItem } from "./provider";

export function ModelRow({
  model,
  isExpanded,
  isLocal,
  copiedId,
  onToggleExpand,
  onCopyId,
  onUpdate,
  onReset,
}: {
  model: ProviderModelItem;
  isExpanded: boolean;
  isLocal: boolean;
  copiedId: string | null;
  onToggleExpand: () => void;
  onCopyId: (id: string, e: React.MouseEvent) => void;
  onUpdate: (updated: ProviderModelItem) => void;
  onReset: () => void;
}) {
  const isEnabled = model.enabled;
  const isEditedContext = isEdited(model, "contextMax");
  const isEditedPrice = isEdited(model, "inputPrice") || isEdited(model, "outputPrice");
  const isEditedOutput = isEdited(model, "maxOutput");
  const isEditedEffort = isEdited(model, "supportsEffort");
  const hasAnyOverride = hasEdits(model);

  const priceStatus = getModelPriceStatus(model, isLocal);
  const capabilities = getModelCapabilities(model);
  const dateAlias = extractDateAlias(model.id);
  // A number equal to the default is only a default when nobody stated it.
  const contextIsGuess = model.contextSource === "default_guess" && !isEditedContext;
  const outputIsGuess = model.fetched?.maxOutput === undefined && !isEditedOutput;

  const toggleEnabled = (e: React.MouseEvent) => {
    e.stopPropagation();
    onUpdate({ ...model, enabled: !isEnabled });
  };

  // Badges
  const isFree = priceStatus.kind === "free";
  const isDeprecated =
    (model.expirationDate && new Date(model.expirationDate) < new Date()) ||
    model.id.includes(":deprecated");

  return (
    <div className={`prov-row-wrap ${isExpanded ? "expanded" : ""} ${!isEnabled ? "disabled" : ""}`}>
      {/* Collapsed Row */}
      <div className="prov-row-collapsed" onClick={onToggleExpand}>
        {/* Visibility Toggle */}
        <button
          type="button"
          className={`prov-vis-btn ${isEnabled ? "active" : ""}`}
          onClick={toggleEnabled}
          title={t("prov.toggle_visibility")}
        >
          {isEnabled ? <Eye size={14} /> : <EyeSlash size={14} />}
        </button>

        {/* Model Name & ID */}
        <div className="prov-cell-model">
          <div className="prov-model-title">
            <span>{model.displayName}</span>
            {isFree && <span className="badge-free">{t("prov.badge_free")}</span>}
            {isDeprecated && (
              <span className="badge-deprecated">{t("prov.badge_deprecated")}</span>
            )}
            {model.byHand && (
              <span className="badge-manual">{t("prov.badge_manual")}</span>
            )}
          </div>
          <div className="prov-model-meta">
            <span className="prov-mono-id" title={model.id}>
              {dateAlias.date ? dateAlias.base : model.id}
            </span>
            {dateAlias.date && (
              <span className="prov-date-chip">{dateAlias.date}</span>
            )}
            <button
              type="button"
              className="prov-copy-id-btn"
              onClick={(e) => onCopyId(model.id, e)}
              title={t("prov.copy_id")}
            >
              {copiedId === model.id ? (
                <Check size={11} className="copy-ok" />
              ) : (
                <Copy size={11} />
              )}
            </button>
          </div>
        </div>

        {/* Context Column */}
        <div className="prov-cell-context">
          {contextIsGuess ? (
            <span className="prov-val-default" title={t("prov.context_hint")}>
              {tokens(model.contextMax)} ({t("prov.default_short")}){" "}
              <Question size={11} />
            </span>
          ) : isEditedContext ? (
            <span className="prov-val-edited">
              {tokens(model.contextMax)}
              <span className="dot-edited" title={t("prov.edited")}>
                •
              </span>
            </span>
          ) : (
            <span className="prov-val-api">{tokens(model.contextMax)}</span>
          )}
          {model.maxOutput > 0 && (
            <span className={`prov-subline ${outputIsGuess ? "prov-val-default" : ""}`}>
              {t("prov.output_up_to", { n: tokens(model.maxOutput) })}
              {outputIsGuess && <> ({t("prov.default_short")})</>}
            </span>
          )}
        </div>

        {/* Pricing Column */}
        <div className="prov-cell-pricing">
          {priceStatus.kind === "free" ? (
            <span className="prov-price-free">{t("prov.price_free")}</span>
          ) : priceStatus.kind === "variable" ? (
            <span className="prov-price-variable">{t("prov.price_variable")}</span>
          ) : priceStatus.kind === "unknown" ? (
            <span className="prov-price-unknown">
              — <span className="badge-warn">{t("prov.price_specify")}</span>
            </span>
          ) : (
            <span className="prov-price-known">
              {priceStatus.input === undefined ? "—" : "$" + formatPrice(priceStatus.input)} / {priceStatus.output === undefined ? "—" : "$" + formatPrice(priceStatus.output)}
              {(priceStatus.input === undefined || priceStatus.output === undefined) && <span className="badge-warn">{t("prov.price_specify")}</span>}
              {isEditedPrice && (
                <span className="dot-edited" title={t("prov.edited")}>
                  •
                </span>
              )}
            </span>
          )}
        </div>

        {/* Capabilities Column */}
        <div className="prov-cell-caps">
          {capabilities.map((cap) => (
            <span key={cap} className="chip-cap">
              {cap === "tools"
                ? t("prov.cap_tools")
                : cap === "vision"
                  ? t("prov.cap_vision")
                  : cap === "reasoning"
                    ? t("prov.cap_reasoning")
                    : t("prov.cap_structured")}
            </span>
          ))}
        </div>

        {/* Edit Action Button */}
        <button
          type="button"
          className="prov-edit-btn"
          onClick={(e) => {
            e.stopPropagation();
            onToggleExpand();
          }}
          title={isExpanded ? t("prov.form_cancel") : t("prov.edit_row")}
        >
          <PencilSimple size={13} />
        </button>
      </div>

      {/* Expanded Editing Panel */}
      {isExpanded && (
        <div className="prov-row-expanded">
          <div className="prov-expanded-grid">
            {/* Context */}
            <span className="prov-expanded-label">{t("prov.table_context")}</span>
            <div className="prov-input-with-tag">
              <NumberField
                label={t("prov.table_context")}
                value={model.contextMax}
                format={formatTokens}
                parse={parseTokensField}
                onCommit={(value) => onUpdate(editContext(model, value))}
              />
              <span className="prov-unit">{t("prov.tokens_unit")}</span>
            </div>
            <FieldState
              edited={isEditedContext}
              stated={
                model.fetched?.contextMax !== undefined
                  ? formatTokens(model.fetched.contextMax)
                  : undefined
              }
              onReset={() => onUpdate(resetField(model, "contextMax"))}
            />

            {/* Max Output */}
            <span className="prov-expanded-label">{t("prov.form_model_output")}</span>
            <div className="prov-input-with-tag">
              <NumberField
                label={t("prov.form_model_output")}
                value={model.maxOutput}
                format={formatTokens}
                parse={parseTokensField}
                onCommit={(value) => onUpdate(editMaxOutput(model, value))}
              />
              <span className="prov-unit">{t("prov.tokens_unit")}</span>
            </div>
            <FieldState
              edited={isEditedOutput}
              stated={
                model.fetched?.maxOutput !== undefined
                  ? formatTokens(model.fetched.maxOutput)
                  : undefined
              }
              onReset={() => onUpdate(resetField(model, "maxOutput"))}
            />

            {/* input price */}
            <span className="prov-expanded-label">{t("prov.price_input")}</span>
            <div className="prov-input-with-tag">
              <span className="prov-symbol">$</span>
              <NumberField label={t("prov.price_input")} value={model.pricing.input}
                format={formatPrice} parse={parsePriceInput}
                onCommit={(value) => onUpdate(editPrice(model, "input", value))} />
              <span className="prov-unit">{t("prov.per_1m")}</span>
            </div>
            <FieldState edited={isEdited(model, "inputPrice")}
              stated={(model.fetched?.inputPrice ?? model.fetched?.pricing?.input) !== undefined
                ? '$' + formatPrice((model.fetched?.inputPrice ?? model.fetched?.pricing?.input)!) : undefined}
              onReset={() => onUpdate(resetField(model, "inputPrice"))} />

            {/* output price */}
            <span className="prov-expanded-label">{t("prov.price_output")}</span>
            <div className="prov-input-with-tag">
              <span className="prov-symbol">$</span>
              <NumberField label={t("prov.price_output")} value={model.pricing.output}
                format={formatPrice} parse={parsePriceInput}
                onCommit={(value) => onUpdate(editPrice(model, "output", value))} />
              <span className="prov-unit">{t("prov.per_1m")}</span>
            </div>
            <FieldState edited={isEdited(model, "outputPrice")}
              stated={(model.fetched?.outputPrice ?? model.fetched?.pricing?.output) !== undefined
                ? '$' + formatPrice((model.fetched?.outputPrice ?? model.fetched?.pricing?.output)!) : undefined}
              onReset={() => onUpdate(resetField(model, "outputPrice"))} />

            {(["cacheRead", "cacheWrite"] as const).map((field) => model[field] !== undefined && (
              <CacheField key={field} model={model} field={field} onUpdate={onUpdate} />
            ))}

            {/* Effort */}
            <span className="prov-expanded-label">{t("prov.effort_title")}</span>
            <div className="prov-effort-controls">
              <button
                type="button"
                className={`prov-filter-pill ${model.supportsEffort ? "active" : ""}`}
                onClick={() => onUpdate(editEffort(model, !model.supportsEffort))}
              >
                {t("prov.cap_reasoning")}
              </button>
              {model.supportsEffort && (["low", "medium", "high"] as const).map((level) => (
                <span key={level} className="chip-cap">{t(`effort.${level}`)}</span>
              ))}
            </div>
            <FieldState
              edited={isEditedEffort}
              stated={
                model.fetched?.supportsEffort !== undefined
                  ? model.fetched.supportsEffort
                    ? t("prov.cap_reasoning")
                    : "—"
                  : undefined
              }
              onReset={() => onUpdate(resetField(model, "supportsEffort"))}
            />
          </div>

          {/* Expanded Bottom Actions */}
          <div className="prov-expanded-foot">
            {/* Nothing to reset to for a row the provider has never listed:
                its numbers were typed in, and the defaults are not "its data". */}
            {hasAnyOverride && model.fetched !== undefined && (
              <button type="button" className="btn btn-small btn-row" onClick={onReset}>
                {t("prov.reset_to_provider")}
              </button>
            )}
            <button type="button" className="btn btn-small btn-accent" onClick={onToggleExpand}>
              {t("prov.form_cancel")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function CacheField({ model, field, onUpdate }: {
  model: ProviderModelItem;
  field: "cacheRead" | "cacheWrite";
  onUpdate: (model: ProviderModelItem) => void;
}) {
  const value = model[field];
  if (value === undefined) return null;
  const stated = model.fetched?.[field];
  return <>
    <span className="prov-expanded-label">{t(field === "cacheRead" ? "prov.cache_read" : "prov.cache_write")}</span>
    <div className="prov-input-with-tag">
      <span className="prov-symbol">$</span>
      <NumberField label={t(field === "cacheRead" ? "prov.cache_read" : "prov.cache_write")}
        value={value} format={formatPrice} parse={parsePriceInput}
        onCommit={(price) => onUpdate(editCache(model, field, price))} />
      <span className="prov-unit">{t("prov.per_1m")}</span>
    </div>
    <FieldState edited={isEdited(model, field)}
      stated={stated === undefined ? undefined : `$${formatPrice(stated)}`}
      onReset={() => onUpdate(resetField(model, field))} />
  </>;
}

/** Tokens typed in a field: `1M`, `200k`, `1 000 000`. */
function parseTokensField(text: string): number | null {
  const parsed = parseTokensInput(text);
  return parsed !== null && validContextSize(parsed) ? parsed : null;
}

/**
 * A number field of the expanded row.
 *
 * The text on screen is a draft of its own: `1M`, `200k` and `1 000 000` are
 * the same number, and reformatting under the caret on every keystroke is what
 * makes such a field unusable. The draft follows the model whenever the model
 * changes from elsewhere — a poll landing with new numbers, a reset — but never
 * while the field holds the caret.
 *
 * The field used to be seeded once, when the row was opened: a poll that
 * answered afterwards left the numbers on screen at their old values, so what
 * was typed went into a row that no longer matched what was shown — the bug
 * that read as "the fields were not applied".
 */
function NumberField({
  label,
  value,
  format,
  parse,
  onCommit,
}: {
  label: string;
  value: number;
  format: (value: number) => string;
  parse: (text: string) => number | null;
  onCommit: (value: number) => void;
}) {
  const [text, setText] = useState(() => format(value));
  const focused = useRef(false);

  useEffect(() => {
    if (!focused.current) setText(format(value));
  }, [value, format]);

  return (
    <input
      className="prov-input prov-input-mono prov-input-dense"
      aria-label={label}
      spellCheck={false}
      value={text}
      onFocus={() => {
        focused.current = true;
      }}
      onBlur={() => {
        focused.current = false;
        // Half-typed text has no number behind it: the field shows what the
        // model actually holds rather than what was left on the keyboard.
        setText(format(value));
      }}
      onChange={(event) => {
        setText(event.target.value);
        const parsed = parse(event.target.value);
        if (parsed !== null) onCommit(parsed);
      }}
    />
  );
}

/**
 * Where a field's number comes from, beside the field itself: what the provider
 * said, whether a hand edit overrides it, and a way back to the provider.
 */
function FieldState({
  edited,
  stated,
  onReset,
}: {
  edited: boolean;
  /** The provider's number, already formatted; absent when it said nothing. */
  stated?: string;
  onReset: () => void;
}) {
  return (
    <div className="prov-api-hint">
      {stated !== undefined && <span>{t("prov.from_api", { value: stated })}</span>}
      {edited && (
        <>
          <span className="badge-edited">{t("prov.edited")}</span>
          {stated !== undefined && (
            <button
              type="button"
              className="prov-reset-field"
              title={t("prov.reset_field")}
              aria-label={t("prov.reset_field")}
              onClick={onReset}
            >
              <ArrowClockwise size={11} />
            </button>
          )}
        </>
      )}
    </div>
  );
}
