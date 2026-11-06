/**
 * Draft types and validation for the providers panel (§10).
 *
 * The `{key}` rule itself lives in `core/provider` — the one mirror of
 * `registry.rs`, shared with the stand's registry. This module is what the
 * form edits: drafts, and the problems that keep a draft from becoming a
 * spec.
 *
 * Models come from the provider, with optional context-window overrides.
 */

import type { Key, Params } from "../../i18n";
import {
  mergedHeaders as mergedHeaderPairs,
  needsKey as needsKeyPairs,
  KEY_PLACEHOLDER,
  isPreset,
} from "../../core/provider";
import type { ModelInfo, Protocol, ProviderSpec } from "../../core/client";
import type { ModelFacts } from "../../core/bindings/ModelFacts";
import type { Pricing } from "../../core/bindings/Pricing";
import type { ReasoningCapabilities } from "../../core/bindings/ReasoningCapabilities";
import type { Effort } from "../../core/client";

export type { Pricing };

export { KEY_PLACEHOLDER, isPreset };

export const MAX_CONTEXT_TOKENS = 0xffff_ffff;

/** What `unknown_model` gives a model nobody said anything about — the same
    numbers as `usage::DEFAULT_CONTEXT_MAX` and `usage::DEFAULT_MAX_OUTPUT`. */
export const DEFAULT_CONTEXT_MAX = 1_000_000;
export const DEFAULT_MAX_OUTPUT = 8_192;

/**
 * A model row as the panel edits it (§10).
 *
 * The numbers are the effective ones — `override ?? fetched ?? default` — and
 * the core keeps the two sources apart (`ModelFacts`) so a refresh can replace
 * one without eating the other. Capabilities and expiry travel through the
 * same registry as the numbers, so reopening the form preserves its chips.
 */
export type ProviderModelItem = ModelInfo;

/** Parses human token counts: "1M", "1.5M", "200k", "1 000 000", "128000". */
export function parseTokensInput(str: string): number | null {
  const clean = str.trim().replace(/\s+/g, "").replace(/,/g, "");
  if (!clean) return null;

  const match = clean.match(/^(\d+(?:\.\d+)?)\s*([kmg])?$/i);
  if (!match) return null;

  const num = parseFloat(match[1]);
  if (!Number.isFinite(num) || num <= 0) return null;

  const unit = match[2]?.toLowerCase();
  let mult = 1;
  if (unit === "k") mult = 1_000;
  else if (unit === "m") mult = 1_000_000;
  else if (unit === "g") mult = 1_000_000_000;

  const result = Math.round(num * mult);
  return validContextSize(result) ? result : null;
}

/** Parses price string: "$5", "$ 5", "5", "5.25", "$0.50", "0.5". */
export function parsePriceInput(str: string): number | null {
  const clean = str.trim().replace(/^\$\s*/, "").replace(/\s+/g, "").replace(/,/g, ".");
  if (!clean) return null;

  if (!/^\d+(?:\.\d+)?$/.test(clean)) return null;
  const num = Number(clean);
  if (!Number.isFinite(num) || num < 0) return null;

  return Math.round(num * 1_000_000) / 1_000_000;
}

/** Format tokens with space grouping: 1 000 000 */
export function formatTokens(n: number): string {
  return n.toLocaleString("ru-RU").replace(/\u00A0/g, " ");
}

/** Format price cleanly: 5 -> "5", 0.5 -> "0.5" */
export function formatPrice(price: number): string {
  if (price === 0) return "0";
  return Number(price.toFixed(6)).toString();
}

export type PriceStatus =
  | { kind: "free" }
  | { kind: "variable" }
  | { kind: "unknown" }
  | { kind: "known"; input?: number; output?: number };

export function getModelPriceStatus(model: ModelInfo, isLocal = false): PriceStatus {
  if (model.pricing.input === -1 || model.pricing.output === -1 || model.id === "openrouter/auto") {
    return { kind: "variable" };
  }

  const legacy = model.overrides?.pricing;
  const inputKnown = model.overrides?.inputPrice !== undefined || legacy !== undefined ||
    model.fetched?.inputPrice !== undefined || model.fetched?.pricing !== undefined ||
    (model.pricingByHand && model.overrides?.outputPrice === undefined) || model.pricing.input > 0;
  const outputKnown = model.overrides?.outputPrice !== undefined || legacy !== undefined ||
    model.fetched?.outputPrice !== undefined || model.fetched?.pricing !== undefined ||
    (model.pricingByHand && model.overrides?.inputPrice === undefined) || model.pricing.output > 0;
  if (isLocal || model.id.includes(":free") || (inputKnown && outputKnown && model.pricing.input === 0 && model.pricing.output === 0)) {
    return { kind: "free" };
  }
  if (inputKnown || outputKnown) {
    return { kind: "known", input: inputKnown ? model.pricing.input : undefined, output: outputKnown ? model.pricing.output : undefined };
  }
  return { kind: "unknown" };
}

export function getModelCapabilities(model: ModelInfo): string[] {
  const caps: string[] = [];
  const item = model as ProviderModelItem;
  const params = item.supportedParameters ?? [];
  const modalities = item.inputModalities ?? [];

  const hasTools = params.some((p) => p.includes("tool") || p === "tools" || p === "function_calling");
  if (hasTools) caps.push("tools");

  const hasVision = modalities.includes("image") || (model.byHand && model.supportsImages);
  if (hasVision) caps.push("vision");

  const hasReasoning =
    model.reasoning !== undefined ||
    model.supportsEffort ||
    model.supportsThinking ||
    params.some((p) => p === "reasoning" || p === "reasoning_effort");
  if (hasReasoning) caps.push("reasoning");

  const hasStructured = params.some(
    (p) => p.includes("structured") || p.includes("json") || p === "response_format",
  );
  if (hasStructured) caps.push("structured");

  return caps;
}

export function extractVendor(modelId: string): string {
  const at = modelId.indexOf("/");
  return at !== -1 ? modelId.slice(0, at) : "";
}

export function extractDateAlias(modelId: string): { base: string; date?: string } {
  const match = modelId.match(/^(.*?)[-_](\d{8}|\d{4}-\d{2}-\d{2})$/);
  if (match) {
    return { base: match[1], date: match[2] };
  }
  return { base: modelId };
}

/** The fields of a row a hand may edit, and a reset puts back. */
export const EDITABLE_FIELDS = ["contextMax", "maxOutput", "pricing", "supportsEffort", "reasoning", "cacheRead", "cacheWrite", "inputPrice", "outputPrice"] as const;
export type EditableField = (typeof EDITABLE_FIELDS)[number];

/** Does this field carry a hand edit right now? */
export function isEdited(model: ModelInfo, field: EditableField): boolean {
  return model.overrides?.[field] !== undefined ||
    ((field === "inputPrice" || field === "outputPrice") && model.overrides?.pricing !== undefined);
}

/** Is any field of the row edited by hand? */
export function hasEdits(model: ModelInfo): boolean {
  const overrides = model.overrides;
  if (overrides === undefined) return false;
  return EDITABLE_FIELDS.some((field) => overrides[field] !== undefined);
}

/**
 * Puts a hand edit on a row.
 *
 * Both copies are written: the number the session and the row use, and the
 * `overrides` entry that says it was typed. Only the second is read by the
 * next poll, which is what keeps the edit from being replaced by whatever the
 * provider answers then.
 */
function withOverride(
  model: ProviderModelItem,
  field: EditableField,
  value: number | boolean | Pricing | ReasoningCapabilities,
): ProviderModelItem {
  const overrides: ModelFacts = { ...model.overrides, [field]: value };
  const edited: ProviderModelItem = { ...model, overrides };
  applyOverride(edited, field, value);
  return edited;
}

/** Writes one effective field from its override, with the marks the core reads. */
function applyOverride(
  model: ProviderModelItem,
  field: EditableField,
  value: number | boolean | Pricing | ReasoningCapabilities,
): void {
  switch (field) {
    case "contextMax":
      model.contextMax = value as number;
      model.contextSource = "hand";
      break;
    case "maxOutput":
      model.maxOutput = value as number;
      break;
    case "pricing":
      model.pricing = value as Pricing;
      model.pricingByHand = true;
      break;
    case "supportsEffort":
      model.supportsEffort = value as boolean;
      break;
    case "reasoning":
      model.reasoning = value as ReasoningCapabilities;
      break;
    case "cacheRead":
    case "cacheWrite":
      model[field] = value as number;
      break;
    case "inputPrice":
    case "outputPrice":
      model.pricing = { ...model.pricing, [field === "inputPrice" ? "input" : "output"]: value as number };
      model.pricingByHand = true;
      break;
  }
}

/** A context window typed in by hand. */
export function editContext(model: ProviderModelItem, contextMax: number): ProviderModelItem {
  return withOverride(model, "contextMax", contextMax);
}

/** A maximum output typed in by hand. */
export function editMaxOutput(model: ProviderModelItem, maxOutput: number): ProviderModelItem {
  return withOverride(model, "maxOutput", maxOutput);
}

/** One side of the price typed in by hand; the other keeps its value. */
export function editPrice(
  model: ProviderModelItem,
  side: "input" | "output",
  value: number,
): ProviderModelItem {
  return withOverride(migratePriceOverride(model), side === "input" ? "inputPrice" : "outputPrice", value);
}

/** Read legacy paired overrides without letting one price pin the other. */
function migratePriceOverride(model: ProviderModelItem): ProviderModelItem {
  const paired = model.overrides?.pricing;
  if (paired === undefined) return model;
  const overrides = { ...model.overrides, inputPrice: paired.input, outputPrice: paired.output };
  delete overrides.pricing;
  return { ...model, overrides };
}

/** Whether the model takes `effort`, set by hand. */
export function editEffort(
  model: ProviderModelItem,
  supportsEffort: boolean,
): ProviderModelItem {
  return withOverride(model, "supportsEffort", supportsEffort);
}

/** A manual list replaces an unknown or incorrect catalog report. */
export function editEffortLevels(model: ProviderModelItem, levels: Effort[]): ProviderModelItem {
  return withOverride(editEffort(model, levels.length > 0), "reasoning", {
    mandatory: false,
    supportsMaxTokens: false,
    ...model.reasoning,
    supportedEfforts: levels,
  });
}

export function editCache(model: ModelInfo, field: "cacheRead" | "cacheWrite", value: number): ModelInfo {
  return withOverride(model, field, value);
}

/**
 * Takes one hand edit back, leaving the others on the row.
 *
 * What is left is what the provider (or a catalog) said, and the default when
 * nobody said anything.
 */
export function resetField(model: ProviderModelItem, field: EditableField): ProviderModelItem {
  if (field === "inputPrice" || field === "outputPrice") model = migratePriceOverride(model);
  const overrides = { ...model.overrides };
  delete overrides[field];
  const reset: ProviderModelItem = {
    ...model,
    overrides: hasAny(overrides) ? overrides : undefined,
  };
  const facts = model.fetched;
  switch (field) {
    case "contextMax":
      reset.contextMax = facts?.contextMax ?? DEFAULT_CONTEXT_MAX;
      reset.contextSource = facts?.contextMax !== undefined ? "provider_reported" : "default_guess";
      break;
    case "maxOutput":
      reset.maxOutput = facts?.maxOutput ?? DEFAULT_MAX_OUTPUT;
      break;
    case "pricing":
      reset.pricing = facts?.pricing ?? { input: 0, output: 0 };
      reset.pricingByHand = false;
      break;
    case "supportsEffort":
      reset.supportsEffort = facts?.supportsEffort ?? false;
      break;
    case "reasoning":
      reset.reasoning = facts?.reasoning;
      break;
    case "cacheRead":
    case "cacheWrite":
      reset[field] = facts?.[field];
      break;
    case "inputPrice":
    case "outputPrice": {
      const side = field === "inputPrice" ? "input" : "output";
      reset.pricing = { ...model.pricing, [side]: facts?.[field] ?? facts?.pricing?.[side] ?? 0 };
      reset.pricingByHand = overrides.inputPrice !== undefined || overrides.outputPrice !== undefined || overrides.pricing !== undefined;
      break;
    }
  }
  return reset;
}

/** Is there anything left in a set of hand edits? */
function hasAny(facts: ModelFacts): boolean {
  return EDITABLE_FIELDS.some((field) => facts[field] !== undefined);
}

/** Every hand edit off a row at once: back to what the provider answered. */
export function resetModelToProvider(model: ProviderModelItem): ProviderModelItem {
  const facts = model.fetched;
  return {
    ...model,
    overrides: undefined,
    contextMax: facts?.contextMax ?? DEFAULT_CONTEXT_MAX,
    contextSource: facts?.contextMax !== undefined ? "provider_reported" : "default_guess",
    maxOutput: facts?.maxOutput ?? DEFAULT_MAX_OUTPUT,
    pricing: { input: facts?.inputPrice ?? facts?.pricing?.input ?? 0, output: facts?.outputPrice ?? facts?.pricing?.output ?? 0 },
    pricingByHand: false,
    supportsEffort: facts?.supportsEffort ?? false,
    reasoning: facts?.reasoning,
    cacheRead: facts?.cacheRead,
    cacheWrite: facts?.cacheWrite,
  };
}

/** Context sizes cross the bridge as Rust's unsigned 32-bit token counts. */
export function validContextSize(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= MAX_CONTEXT_TOKENS;
}

/**
 * What the provider answered, laid over the rows the form already holds (§10).
 *
 * `fetched` becomes the answer, a hand edit stays where it was, and the numbers
 * on screen are the two put together: `override ?? fetched ?? default`. A row
 * written by hand and no longer served is kept, and stays marked.
 *
 * The previous version of this function read `fetched` only for rows that had
 * been edited already — which is why a fresh answer never showed where its
 * numbers came from, and "reset to provider data" had nothing to reset to.
 */
export function mergeAnswer(saved: ModelInfo[], models: ModelInfo[]): ModelInfo[] {
  const merged: ModelInfo[] = models.map((fresh) => {
    const previous = saved.find((m) => m.id === fresh.id) as ProviderModelItem | undefined;
    const item: ProviderModelItem = {
      ...fresh,
      // The core hands its answer back with the two sources already told apart;
      // only a raw answer (the stand, a shape without the fields) is read here.
      fetched: (fresh as ProviderModelItem).fetched ?? factsOf(fresh),
      overrides: previous ? handEditsOf(previous) : (fresh as ProviderModelItem).overrides,
      ...(previous ? { enabled: previous.enabled, byHand: previous.byHand } : {}),
    };
    return applyAll(item);
  });

  for (const previous of saved) {
    if (previous.byHand && !merged.some((m) => m.id === previous.id)) {
      merged.push(previous as ProviderModelItem);
    }
  }
  return merged;
}

/** What an answer states about a row, as a source of facts.
    Numbers equal to the defaults read as "not stated": the core keeps one
    number, not two, and a provider stating exactly 8 192 output tokens cannot
    be told apart from one that states nothing. */
function factsOf(model: ModelInfo): ModelFacts {
  return {
    contextMax: model.contextSource === "default_guess" ? undefined : model.contextMax,
    maxOutput: model.maxOutput !== DEFAULT_MAX_OUTPUT ? model.maxOutput : undefined,
    pricing: model.pricing.input > 0 || model.pricing.output > 0 ? model.pricing : undefined,
    supportsEffort: model.supportsEffort,
    reasoning: model.reasoning,
    cacheRead: model.cacheRead,
    cacheWrite: model.cacheWrite,
  };
}

/** The hand edits of a row, including ones written before `overrides` existed:
    `contextSource: hand` and `pricingByHand` were the only marks then. */
function handEditsOf(previous: ProviderModelItem): ModelFacts | undefined {
  const facts: ModelFacts = { ...previous.overrides };
  if (facts.contextMax === undefined && previous.contextSource === "hand") {
    facts.contextMax = previous.contextMax;
  }
  if (facts.pricing === undefined && facts.inputPrice === undefined && facts.outputPrice === undefined && previous.pricingByHand) {
    facts.pricing = previous.pricing;
  }
  return hasAny(facts) ? facts : undefined;
}

/** Lays every hand edit of a row over its numbers. */
function applyAll(model: ProviderModelItem): ProviderModelItem {
  for (const field of EDITABLE_FIELDS) {
    const value = model.overrides?.[field];
    if (value !== undefined) applyOverride(model, field, value);
  }
  return model;
}

/** Header rows as the editor holds them: a name-less row is a validation
    problem, not a header, so it never reaches the merge. */
export function mergedHeaders(
  protocol: Protocol,
  headers: Array<HeaderDraft>,
): Array<[string, string]> {
  return mergedHeaderPairs(
    protocol,
    headers
      .filter((header) => header.name.trim() !== "")
      .map((header) => [header.name.trim(), header.value] as const),
  );
}

/** `ProviderSpec::needs_key` over the same draft rows. */
export function needsKey(
  protocol: Protocol,
  headers: Array<HeaderDraft>,
): boolean {
  return needsKeyPairs(
    protocol,
    headers
      .filter((header) => header.name.trim() !== "")
      .map((header) => [header.name.trim(), header.value] as const),
  );
}

// --- drafts -----------------------------------------------------------------

export type HeaderDraft = { name: string; value: string };

export type ProviderDraft = {
  /** Empty until the provider is saved for the first time: see `newProviderId`. */
  id: string;
  displayName: string;
  protocol: Protocol;
  baseUrl: string;
  headers: Array<HeaderDraft>;
  /** As the provider reported them (`discoverModels`), or as they were saved. */
  models: Array<ModelInfo>;
};

/** A provider that does not exist yet. OpenAI is the common custom gateway. */
export function emptyDraft(): ProviderDraft {
  return {
    id: "",
    displayName: "",
    protocol: "openai",
    baseUrl: "",
    headers: [],
    models: [],
  };
}

export function draftFromSpec(spec: ProviderSpec): ProviderDraft {
  return {
    id: spec.id,
    displayName: spec.displayName,
    protocol: spec.protocol,
    baseUrl: spec.baseUrl,
    headers: Object.entries(spec.headers).map(([name, value]) => ({
      name,
      value: value ?? "",
    })),
    models: spec.models,
  };
}

// --- id ---------------------------------------------------------------------

/**
 * The id a new provider is saved under.
 *
 * Nobody types it: it is the name of the keychain item and of the entry in
 * `providers.json`, which is bookkeeping, and a field for it was a question
 * the person had no reason to have an answer to. It is made from the name
 * once, when the provider is created, and never again — renaming a provider
 * must not move its key.
 *
 * A name with nothing Latin in it («Мой шлюз») falls back to the host of the
 * address, and that to a plain word; `taken` keeps it from landing on a
 * provider that is already there, which `Providers::put` would replace.
 */
export function newProviderId(
  displayName: string,
  baseUrl: string,
  taken: ReadonlyArray<string>,
): string {
  const base = slug(displayName) || slug(hostOf(baseUrl)) || "provider";
  let id = base;
  for (let n = 2; taken.includes(id); n += 1) id = `${base}-${n}`;
  return id;
}

function slug(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function hostOf(baseUrl: string): string {
  try {
    return new URL(baseUrl.trim()).hostname;
  } catch {
    return "";
  }
}

// --- validation -------------------------------------------------------------

/** A problem is a dictionary key with its holes, not a rendered string: the
    language of the window is the panel's business, not this module's. */
export type Problem = { key: Key; params?: Params };

function validate(
  draft: ProviderDraft,
  taken: ReadonlyArray<string>,
): {
  problems: Array<Problem>;
  spec: ProviderSpec | null;
} {
  const problems: Array<Problem> = [];

  if (!draft.displayName.trim()) problems.push({ key: "prov.error_name" });
  if (!draft.baseUrl.trim()) problems.push({ key: "prov.error_url" });
  for (const model of draft.models) {
    if ([model.pricing.input, model.pricing.output].some((price) => !Number.isFinite(price) || (price < 0 && price !== -1))) {
      problems.push({ key: "prov.error_price", params: { name: model.displayName } });
    }
    if (model.contextSource === "hand" && !validContextSize(model.contextMax)) {
      problems.push({ key: "prov.error_context", params: { name: model.displayName } });
    }
  }

  const names = new Set<string>();
  draft.headers.forEach((header, index) => {
    const name = header.name.trim();
    if (!name) {
      problems.push({ key: "prov.error_header_name", params: { n: index + 1 } });
      return;
    }
    if (names.has(name)) {
      problems.push({ key: "prov.error_header_dup", params: { name } });
    }
    names.add(name);
  });

  return {
    problems,
    spec:
      problems.length > 0
        ? null
        : {
            id:
              draft.id.trim() ||
              newProviderId(draft.displayName, draft.baseUrl, taken),
            displayName: draft.displayName.trim(),
            protocol: draft.protocol,
            baseUrl: draft.baseUrl.trim(),
            headers: Object.fromEntries(
              draft.headers
                // A name-less row is already reported as a problem; the spec
                // just never sees it. Values pass through as typed: an empty
                // value overrides a protocol default with nothing, and that
                // is a legitimate way to drop a header.
                .map((header) => [header.name.trim(), header.value])
                .filter(([name]) => name !== ""),
            ),
            // Keep provider capabilities together with the user's context edits.
            models: draft.models,
          },
  };
}

export function draftProblems(draft: ProviderDraft): Array<Problem> {
  return validate(draft, []).problems;
}

/**
 * The spec to save, or `null` while the draft has problems. `taken` is the
 * ids already in the list, for a draft that has none of its own yet.
 */
export function draftToSpec(
  draft: ProviderDraft,
  taken: ReadonlyArray<string> = [],
): ProviderSpec | null {
  return validate(draft, taken).spec;
}

/** Where the provider's key stands, as the shell reports it (`apiKeySource`). */
export type KeyState =
  | { kind: "loading" }
  | { kind: "unknown" }
  | { kind: "not_needed" }
  | { kind: "keychain" }
  | { kind: "environment" }
  | { kind: "none" };
