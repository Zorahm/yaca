import { describe, expect, it } from "vitest";

import type { ModelInfo } from "../../core/client";
import { modelKey } from "../modelSearch";
import { configText, emptyPicks, NO_MODEL } from "./routingWizard";

const model = (providerId: string, id: string): ModelInfo =>
  ({ providerId, id, displayName: id }) as ModelInfo;

const small = model("local", "qwen3:1.7b");
const mid = model("gateway", "sonnet");
const big = model("gateway", "opus");
const models = [small, mid, big];

const picks = {
  decider: modelKey(small),
  fallback: modelKey(mid),
  strong: modelKey(big),
};

describe("router quick setup", () => {
  it("builds a configuration out of three picked models", () => {
    const text = configText(models, picks);
    expect(text).not.toBeNull();

    const cfg = JSON.parse(text ?? "{}");
    expect(cfg.decisionModel).toEqual({ provider: "local", model: "qwen3:1.7b", timeoutMs: 3000 });
    expect(cfg.default).toEqual({ provider: "gateway", model: "sonnet" });
    expect(cfg.decisions[0].model).toEqual({ provider: "gateway", model: "opus" });
  });

  it("asks one question and routes on it", () => {
    const cfg = JSON.parse(configText(models, picks) ?? "{}");
    expect(cfg.signals.map((s: { name: string }) => s.name)).toEqual(["needs_reasoning"]);
    // The rule reads the signal the question defines, by the same name.
    expect(cfg.decisions[0].rules.signal.name).toBe("needs_reasoning");
  });

  it("refuses to build while any pick is missing", () => {
    expect(configText(models, emptyPicks)).toBeNull();
    expect(configText(models, { ...picks, strong: NO_MODEL })).toBeNull();
  });

  it("refuses a model the list no longer has", () => {
    expect(configText([small, mid], picks)).toBeNull();
  });

  it("tells models apart by provider as well as by id", () => {
    const twin = model("other", "sonnet");
    const cfg = JSON.parse(
      configText([small, mid, twin, big], { ...picks, fallback: modelKey(twin) }) ?? "{}",
    );
    expect(cfg.default.provider).toBe("other");
  });
});
