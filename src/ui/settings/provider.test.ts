/**
 * The provider-draft logic that the panel form runs on every keystroke.
 *
 * The critical case is the §0 one: an empty header value is a legitimate
 * override — that is how a local gateway drops `authorization` — and must not
 * be mistaken for a missing one.
 *
 * Provider metadata passes through; only manually edited context sizes need
 * validation before they can be saved.
 */

import { describe, expect, it } from "vitest";

import type { ProviderSpec } from "../../core/client";
import {
  DEFAULT_CONTEXT_MAX,
  DEFAULT_MAX_OUTPUT,
  draftFromSpec,
  draftProblems,
  draftToSpec,
  editContext,
  editCache,
  editEffort,
  editMaxOutput,
  editPrice,
  hasEdits,
  isEdited,
  mergeAnswer,
  emptyDraft,
  mergedHeaders,
  needsKey,
  newProviderId,
  parseTokensInput,
  parsePriceInput,
  formatTokens,
  formatPrice,
  getModelPriceStatus,
  getModelCapabilities,
  extractVendor,
  extractDateAlias,
  resetField,
  resetModelToProvider,
  type ProviderDraft,
  type ProviderModelItem,
} from "./provider";

function problemsOf(draft: ProviderDraft): string[] {
  return draftProblems(draft).map((problem) => problem.key);
}

describe("needsKey", () => {
  it("is true for the anthropic defaults, which carry {key} themselves", () => {
    expect(needsKey("anthropic", [])).toBe(true);
  });

  it("is true for the openai defaults", () => {
    expect(needsKey("openai", [])).toBe(true);
  });

  it("is false when a custom header overrides the key away", () => {
    // The README example: a local Ollama that gets `authorization: ""`.
    expect(
      needsKey("openai", [{ name: "authorization", value: "" }]),
    ).toBe(false);
  });

  it("is false when no header mentions the key at all", () => {
    expect(
      needsKey("anthropic", [{ name: "x-api-key", value: "" }]),
    ).toBe(false);
  });

  it("is true when any header asks for the key", () => {
    // The openai default carries {key}, so "no custom header mentions it"
    // must still override the default away before the provider is keyless.
    const keyless: ProviderDraft = {
      ...emptyDraft(),
      protocol: "openai",
      headers: [{ name: "authorization", value: "" }],
    };
    expect(needsKey(keyless.protocol, keyless.headers)).toBe(false);
    expect(
      needsKey(keyless.protocol, [
        ...keyless.headers,
        { name: "x-custom", value: "Bearer {key}" },
      ]),
    ).toBe(true);
  });
});

describe("mergedHeaders", () => {
  it("lets spec headers replace protocol defaults, not only extend them", () => {
    expect(mergedHeaders("openai", [{ name: "authorization", value: "" }])).toEqual([
      ["accept", "text/event-stream"],
      ["authorization", ""],
      ["content-type", "application/json"],
    ]);
  });

  it("ignores name-less rows instead of clobbering a default", () => {
    expect(mergedHeaders("openai", [{ name: "  ", value: "x" }])).toEqual([
      ["accept", "text/event-stream"],
      ["authorization", "Bearer {key}"],
      ["content-type", "application/json"],
    ]);
  });
});

describe("drafts and specs", () => {
  const spec: ProviderSpec = {
    id: "ollama",
    displayName: "Ollama",
    protocol: "openai",
    baseUrl: "http://localhost:11434",
    headers: { authorization: "" },
    models: [
      {
        id: "qwen3-coder:30b",
        providerId: "ollama",
        displayName: "Qwen3 Coder 30B",
        contextMax: 256000,
        maxOutput: 32000,
        pricing: { input: 0, output: 0 },
        pricingByHand: false,
        supportsEffort: false,
        supportsThinking: false,
        contextSource: "default_guess",
        byHand: false,
        enabled: true,
        supportsImages: true,
      },
    ],
  };

  it("round-trips a spec through a draft unchanged", () => {
    expect(draftToSpec(draftFromSpec(spec))).toEqual(spec);
  });

  it("keeps a context override while updating the provider's other fields", () => {
    const previous = editContext(spec.models[0], 750_000);
    const reported = { ...spec.models[0], maxOutput: 64_000, contextMax: 1_000_000 };
    const merged = mergeAnswer([previous], [reported]);

    expect(merged[0].contextMax).toBe(750_000);
    expect(merged[0].contextSource).toBe("hand");
    // The answer is kept apart, so the edit can be undone and the poll can
    // replace it without touching the edit.
    expect(merged[0].fetched?.maxOutput).toBe(64_000);
    expect(merged[0].overrides).toEqual({ contextMax: 750_000 });
    expect(merged[0].maxOutput).toBe(64_000);
    expect(draftToSpec({ ...draftFromSpec(spec), models: merged })?.models).toEqual(merged);
  });

  it("preserves a manually entered price when models refresh", () => {
    const previous = editPrice(spec.models[0], "input", 2);
    const reported = { ...spec.models[0], maxOutput: 64_000 };
    const merged = mergeAnswer([previous], [reported])[0];

    expect(merged.pricingByHand).toBe(true);
    expect(merged.pricing).toEqual({ input: 2, output: 0 });
    expect(merged.maxOutput).toBe(64_000);
  });

  it("keeps a maximum output typed in by hand across a refresh", () => {
    // The bug from the screenshot: 8 192 typed over came back as the
    // provider's number at the next poll.
    const previous = editMaxOutput(spec.models[0], 393_216);
    const merged = mergeAnswer([previous], [{ ...spec.models[0] }])[0];

    expect(merged.maxOutput).toBe(393_216);
    expect(merged.overrides).toEqual({ maxOutput: 393_216 });
    // The provider's own number is still kept under it, for the reset.
    expect(merged.fetched?.maxOutput).toBe(32_000);
  });

  it("records what the provider said even when nothing was edited", () => {
    // Without this the panel had nothing to show under "from API", and
    // "reset to provider data" had nothing to go back to.
    const merged = mergeAnswer([], [spec.models[0]])[0];
    expect(merged.fetched).toEqual({
      contextMax: undefined,
      maxOutput: 32_000,
      pricing: undefined,
      supportsEffort: false,
    });
    expect(merged.overrides).toBeUndefined();
  });

  it("keeps a row switched off switched off", () => {
    const saved = [{ ...spec.models[0], enabled: false }];
    const merged = mergeAnswer(saved, [{ ...spec.models[0] }]);
    expect(merged[0].enabled).toBe(false);
  });

  it.each([NaN, 0, -1, 1.5, 4294967296])("rejects an invalid manual context: %s", (contextMax) => {
    const draft = draftFromSpec(spec);
    draft.models = [{ ...draft.models[0], contextSource: "hand", contextMax }];
    expect(problemsOf(draft)).toContain("prov.error_context");
    expect(draftToSpec(draft)).toBeNull();
  });

  it("accepts an empty model list: nobody has asked the provider yet", () => {
    const draft = draftFromSpec(spec);
    draft.models = [];
    expect(draftProblems(draft)).toEqual([]);
    expect(draftToSpec(draft)?.models).toEqual([]);
  });

  it("passes the provider's answer through untouched", () => {
    // Including a zero price: a local model really is free, and a paid one
    // the price list has not heard of is counted as free too.
    const draft = draftFromSpec(spec);
    expect(draftToSpec(draft)?.models).toEqual(spec.models);
  });
});

describe("draftProblems", () => {
  it("demands a name and a base URL, and no id", () => {
    const keys = problemsOf(emptyDraft());
    expect(keys).toHaveLength(2);
    expect(keys).toContain("prov.error_name");
    expect(keys).toContain("prov.error_url");
  });

  it("demands a name on every header row and forbids duplicates", () => {
    const draft = emptyDraft();
    draft.headers = [
      { name: "  ", value: "x" },
      { name: "x-a", value: "1" },
      { name: "x-a", value: "2" },
    ];
    const keys = problemsOf(draft);
    expect(keys).toContain("prov.error_header_name");
    expect(keys).toContain("prov.error_header_dup");
  });

  it("refuses to build a spec while problems remain", () => {
    expect(draftToSpec(emptyDraft())).toBeNull();
  });
});

describe("newProviderId", () => {
  it("is made from the name", () => {
    expect(newProviderId("My Gateway", "https://gate.example.com", [])).toBe(
      "my-gateway",
    );
    expect(newProviderId("  LM Studio (local)  ", "", [])).toBe("lm-studio-local");
  });

  it("steps aside from an id that is already there", () => {
    expect(newProviderId("OpenAI", "", ["openai"])).toBe("openai-2");
    expect(newProviderId("OpenAI", "", ["openai", "openai-2"])).toBe("openai-3");
  });

  it("falls back to the host when the name has nothing Latin in it", () => {
    expect(newProviderId("Мой шлюз", "http://localhost:11434", [])).toBe(
      "localhost",
    );
    expect(newProviderId("Мой шлюз", "https://gate.example.com/v1", [])).toBe(
      "gate-example-com",
    );
  });

  it("still gives an id when neither says anything", () => {
    expect(newProviderId("Шлюз", "не адрес", [])).toBe("provider");
  });
});

describe("a new provider's id", () => {
  const draft: ProviderDraft = {
    ...emptyDraft(),
    displayName: "OpenAI",
    baseUrl: "https://proxy.example.com",
  };

  it("is assigned when the spec is built, clear of the ones taken", () => {
    expect(draftToSpec(draft, ["openai", "anthropic"])?.id).toBe("openai-2");
  });

  it("is left alone once the provider has one: a rename must not move the key", () => {
    const saved = { ...draft, id: "gateway", displayName: "Renamed" };
    expect(draftToSpec(saved, ["gateway"])?.id).toBe("gateway");
  });
});

describe("model card data parsers and helpers", () => {
  it("parses human token input sizes", () => {
    expect(parseTokensInput("1M")).toBe(1_000_000);
    expect(parseTokensInput("1.5m")).toBe(1_500_000);
    expect(parseTokensInput("200k")).toBe(200_000);
    expect(parseTokensInput("200 K")).toBe(200_000);
    expect(parseTokensInput("1 000 000")).toBe(1_000_000);
    expect(parseTokensInput("1,000,000")).toBe(1_000_000);
    expect(parseTokensInput("128000")).toBe(128_000);
    expect(parseTokensInput("0")).toBeNull();
    expect(parseTokensInput("-100")).toBeNull();
    expect(parseTokensInput("abc")).toBeNull();
  });

  it("parses human price inputs", () => {
    expect(parsePriceInput("$5")).toBe(5);
    expect(parsePriceInput("$ 5.5")).toBe(5.5);
    expect(parsePriceInput("0.50")).toBe(0.5);
    expect(parsePriceInput("0")).toBe(0);
    expect(parsePriceInput("$0")).toBe(0);
    expect(parsePriceInput("-5")).toBeNull();
    expect(parsePriceInput("xyz")).toBeNull();
  });

  it("formats tokens and prices cleanly", () => {
    expect(formatTokens(1_000_000)).toBe("1 000 000");
    expect(formatPrice(5)).toBe("5");
    expect(formatPrice(0.5)).toBe("0.5");
    expect(formatPrice(25.125)).toBe("25.125");
  });

  it("distinguishes free, variable, unknown and known prices", () => {
    const baseModel: ProviderModelItem = {
      id: "test",
      providerId: "test",
      displayName: "Test",
      contextMax: 1_000_000,
      maxOutput: 32_000,
      pricing: { input: 0, output: 0 },
      pricingByHand: false,
      supportsEffort: false,
      supportsThinking: false,
      contextSource: "provider_reported",
      supportsImages: true,
      byHand: false,
      enabled: true,
    };

    // Unknown: remote provider with 0 price and no byHand. Zero is not
    // "free" by itself — it is "free or unknown", and the panel must say
    // which one it can stand behind.
    expect(getModelPriceStatus(baseModel, false)).toEqual({ kind: "unknown" });
    // A stated zero is free; silence remains unknown.
    expect(
      getModelPriceStatus({ ...baseModel, fetched: { pricing: { input: 0, output: 0 } } }, false),
    ).toEqual({ kind: "free" });

    // Free: local provider
    expect(getModelPriceStatus(baseModel, true)).toEqual({ kind: "free" });

    // Free: model has :free suffix
    expect(getModelPriceStatus({ ...baseModel, id: "meta-llama/llama-3:free" }, false)).toEqual({ kind: "free" });

    // Free: explicitly set by hand as 0
    expect(getModelPriceStatus({ ...baseModel, pricingByHand: true }, false)).toEqual({ kind: "free" });

    // Variable: openrouter/auto or negative price
    expect(getModelPriceStatus({ ...baseModel, id: "openrouter/auto" }, false)).toEqual({ kind: "variable" });
    expect(getModelPriceStatus({ ...baseModel, pricing: { input: -1, output: -1 } }, false)).toEqual({ kind: "variable" });

    // Known: positive pricing
    expect(getModelPriceStatus({ ...baseModel, pricing: { input: 5, output: 25 } }, false)).toEqual({
      kind: "known",
      input: 5,
      output: 25,
    });
  });

  it("detects capabilities from supported parameters and flags", () => {
    const model: ProviderModelItem = {
      id: "claude-opus",
      providerId: "anthropic",
      displayName: "Claude Opus",
      contextMax: 1_000_000,
      maxOutput: 128_000,
      pricing: { input: 5, output: 25 },
      pricingByHand: false,
      supportsEffort: true,
      supportsThinking: true,
      contextSource: "provider_reported",
      supportsImages: true,
      byHand: false,
      enabled: true,
      supportedParameters: ["tools", "reasoning", "structured_outputs"],
      inputModalities: ["text", "image"],
    };

    const caps = getModelCapabilities(model);
    expect(caps).toContain("tools");
    expect(caps).toContain("vision");
    expect(caps).toContain("reasoning");
    expect(caps).toContain("structured");
  });

  it("extracts vendor and date aliases", () => {
    expect(extractVendor("anthropic/claude-3-opus")).toBe("anthropic");
    expect(extractVendor("gpt-4o")).toBe("");

    expect(extractDateAlias("claude-opus-4-5-20251101")).toEqual({
      base: "claude-opus-4-5",
      date: "20251101",
    });
    expect(extractDateAlias("gpt-4o-2024-08-06")).toEqual({
      base: "gpt-4o",
      date: "2024-08-06",
    });
    expect(extractDateAlias("qwen3-coder")).toEqual({ base: "qwen3-coder" });
  });

  it("resets overrides back to provider fetched data", () => {
    const overridden = {
      ...editContext(
        { ...editPrice(specModel("claude-opus-5"), "input", 10), contextMax: 500_000 } as ProviderModelItem,
        500_000,
      ),
      fetched: {
        contextMax: 1_000_000,
        maxOutput: 128_000,
        pricing: { input: 5, output: 25 },
        supportsEffort: true,
      },
    } as ProviderModelItem;
    overridden.maxOutput = 64_000;

    const reset = resetModelToProvider(overridden);
    expect(reset.contextMax).toBe(1_000_000);
    expect(reset.contextSource).toBe("provider_reported");
    expect(reset.pricing).toEqual({ input: 5, output: 25 });
    expect(reset.pricingByHand).toBe(false);
    expect(reset.maxOutput).toBe(128_000);
    expect(reset.supportsEffort).toBe(true);
    expect(reset.overrides).toBeUndefined();
  });

  it("resets one field and leaves the other edits on the row", () => {
    const edited = editPrice(editMaxOutput(specModel("m"), 20_000), "output", 9);
    const reset = resetField(edited, "maxOutput");

    expect(reset.maxOutput).toBe(DEFAULT_MAX_OUTPUT);
    expect(reset.overrides).toEqual({ outputPrice: 9 });
    expect(reset.pricing.output).toBe(9);
  });

  it("clears the last edit off the row instead of leaving an empty one", () => {
    const reset = resetField(editMaxOutput(specModel("m"), 20_000), "maxOutput");
    expect(reset.overrides).toBeUndefined();
    expect(hasEdits(reset)).toBe(false);
  });

  it("marks only the field that was typed in", () => {
    const edited = editEffort(specModel("m"), true);
    expect(isEdited(edited, "supportsEffort")).toBe(true);
    expect(isEdited(edited, "contextMax")).toBe(false);
    expect(edited.supportsEffort).toBe(true);
  });

  it("refreshes the other price and resets only the edited side", () => {
    const edited = editPrice(specModel("private/model"), "input", 0.15);
    const fresh = { ...specModel("private/model"), pricing: { input: 1, output: 3 }, fetched: { pricing: { input: 1, output: 3 } } };
    const merged = mergeAnswer([edited], [fresh])[0];
    expect(merged.pricing).toEqual({ input: 0.15, output: 3 });
    expect(isEdited(merged, "outputPrice")).toBe(false);
    expect(resetField(merged, "inputPrice").pricing).toEqual({ input: 1, output: 3 });
  });

  it("keeps an unknown price side distinct from a manually entered zero", () => {
    expect(getModelPriceStatus(editPrice(specModel("m"), "input", 0))).toEqual({ kind: "known", input: 0, output: undefined });
    expect(getModelPriceStatus(editPrice(editPrice(specModel("m"), "input", 0), "output", 0))).toEqual({ kind: "free" });
    expect(getModelCapabilities(specModel("m"))).not.toContain("vision");
  });

  it("preserves and resets cache prices across a refresh", () => {
    const edited = editCache(specModel("m"), "cacheRead", 0.01);
    const merged = mergeAnswer([edited], [{ ...specModel("m"), cacheRead: 0.05, fetched: { cacheRead: 0.05 } }])[0];
    expect(merged.cacheRead).toBe(0.01);
    expect(resetField(merged, "cacheRead").cacheRead).toBe(0.05);
  });

  it("rejects trailing garbage in price input", () => {
    expect(parsePriceInput("5oops")).toBeNull();
  });

  it("preserves manual models (byHand: true) across provider refreshes", () => {
    const manualModel: ProviderModelItem = {
      ...specModel("custom-my-model"),
      providerId: "custom",
      displayName: "Custom My Model",
      contextMax: 128_000,
      maxOutput: 8_000,
      pricing: { input: 1, output: 2 },
      pricingByHand: true,
      contextSource: "hand",
      supportsImages: false,
      byHand: true,
      overrides: { contextMax: 128_000, maxOutput: 8_000, pricing: { input: 1, output: 2 } },
    };

    const fetchedModel: ProviderModelItem = {
      ...specModel("remote-model"),
      providerId: "custom",
      displayName: "Remote Model",
      contextMax: 200_000,
      maxOutput: 16_000,
      pricing: { input: 2, output: 4 },
      contextSource: "provider_reported",
    };

    const saved = [manualModel];
    const reported = [fetchedModel];
    const merged = mergeAnswer(saved, reported);

    expect(merged).toHaveLength(2);
    expect(merged.find((m) => m.id === "custom-my-model")).toBeDefined();
    expect(merged.find((m) => m.id === "remote-model")).toBeDefined();
  });
});

/** One row with nothing stated about it, for the helpers above. */
function specModel(id: string): ProviderModelItem {
  return {
    id,
    providerId: "test",
    displayName: id,
    contextMax: DEFAULT_CONTEXT_MAX,
    maxOutput: DEFAULT_MAX_OUTPUT,
    pricing: { input: 0, output: 0 },
    pricingByHand: false,
    supportsEffort: false,
    supportsThinking: false,
    contextSource: "default_guess",
    supportsImages: true,
    byHand: false,
    enabled: true,
  };
}
