import { describe, expect, it } from "vitest";
import type { ModelInfo } from "./client";
import { effectiveEffort, effortLevels } from "./reasoning";
import { editEffortLevels, mergeAnswer, resetModelToProvider } from "../ui/settings/provider";

const gemini: ModelInfo = {
  id: "models/gemini-3.8-flash", providerId: "google", displayName: "Gemini",
  contextMax: 1_000_000, maxOutput: 8192, pricing: { input: 0, output: 0 },
  pricingByHand: false, supportsEffort: true, supportsThinking: false,
  supportsImages: true, contextSource: "provider_reported", byHand: false, enabled: true,
  reasoning: { supportedEfforts: ["low", "medium", "high"], defaultEffort: "medium", mandatory: true, supportsMaxTokens: false },
};

describe("model reasoning controls", () => {
  it("offers Gemini's three depths and falls back to provider defaults for old max picks", () => {
    expect(effortLevels(gemini)).toEqual(["low", "medium", "high"]);
    expect(effectiveEffort(gemini, "max")).toBe("auto");
    expect(effectiveEffort(gemini, "low")).toBe("low");
    expect(effectiveEffort(gemini, "auto")).toBe("auto");
  });

  it("does not invent controls from a tag or allow mandatory reasoning to be disabled", () => {
    expect(effortLevels({ ...gemini, reasoning: undefined })).toEqual([]);
    expect(effortLevels(undefined)).toEqual([]);
    expect(effortLevels({ ...gemini, supportsEffort: false })).toEqual([]);
    expect(effortLevels({ ...gemini, reasoning: { ...gemini.reasoning!, supportedEfforts: ["none", "high"] } })).toEqual(["high"]);
  });

  it("keeps hand-edited levels across refresh and restores catalog levels on reset", () => {
    const edited = editEffortLevels(gemini, ["low", "high"]);
    const refreshed = mergeAnswer([edited], [{ ...gemini, fetched: { reasoning: gemini.reasoning, supportsEffort: true } }])[0];
    expect(effortLevels(refreshed)).toEqual(["low", "high"]);
    expect(effortLevels(resetModelToProvider(refreshed))).toEqual(["low", "medium", "high"]);
  });
});
