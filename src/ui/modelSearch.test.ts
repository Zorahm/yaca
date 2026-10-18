/**
 * The model menu's list for a query (§7.1): groups, search, recent.
 */

import { describe, expect, it } from "vitest";

import type { ModelInfo } from "../core/client";
import {
  groupsOf,
  matches,
  modelKey,
  parseRecent,
  recentOf,
  withRecent,
} from "./modelSearch";

function model(providerId: string, id: string, displayName = id): ModelInfo {
  return {
    id,
    providerId,
    displayName,
    contextMax: 128000,
    maxOutput: 8000,
    pricing: { input: 0, output: 0 },
    supportsEffort: false,
    supportsThinking: false,
    contextSource: "default_guess",
    supportsImages: true,
  };
}

const NAMES: Record<string, string> = {
  openai: "OpenAI",
  openrouter: "OpenRouter",
  local: "Мой шлюз",
};
const nameOf = (provider: string) => NAMES[provider] ?? provider;

const MODELS = [
  model("openai", "gpt-5", "GPT-5"),
  model("openai", "gpt-5-mini", "GPT-5 mini"),
  model("openrouter", "anthropic/claude-sonnet-5", "Anthropic: Claude Sonnet 5"),
  model("openrouter", "openai/gpt-5", "OpenAI: GPT-5"),
  model("local", "qwen3-coder:30b"),
];

function ids(groups: ReturnType<typeof groupsOf>): string[] {
  return groups.map((g) => `${g.provider}: ${g.models.map((m) => m.id).join(", ")}`);
}

describe("matches", () => {
  it("looks in the name, the id and the provider's name", () => {
    const sonnet = MODELS[2];
    expect(matches(sonnet, "OpenRouter", "sonnet")).toBe(true);
    expect(matches(sonnet, "OpenRouter", "anthropic/claude")).toBe(true);
    expect(matches(sonnet, "OpenRouter", "openrouter")).toBe(true);
    expect(matches(sonnet, "OpenRouter", "haiku")).toBe(false);
  });

  it("wants every word, wherever each one sits", () => {
    expect(matches(MODELS[2], "OpenRouter", "open son")).toBe(true);
    expect(matches(MODELS[2], "OpenRouter", "open mini")).toBe(false);
  });

  it("ignores case and stray spaces", () => {
    expect(matches(MODELS[0], "OpenAI", "  GPT  ")).toBe(true);
  });

  it("finds a provider by a name that is not in its id", () => {
    expect(matches(MODELS[4], "Мой шлюз", "шлюз")).toBe(true);
  });
});

describe("groupsOf", () => {
  it("groups by provider in the order the models arrived", () => {
    expect(ids(groupsOf(MODELS, nameOf, ""))).toEqual([
      "openai: gpt-5, gpt-5-mini",
      "openrouter: anthropic/claude-sonnet-5, openai/gpt-5",
      "local: qwen3-coder:30b",
    ]);
  });

  it("drops a provider none of whose models match", () => {
    expect(ids(groupsOf(MODELS, nameOf, "mini"))).toEqual(["openai: gpt-5-mini"]);
  });

  it("keeps the same model id apart under each provider serving it", () => {
    expect(ids(groupsOf(MODELS, nameOf, "gpt-5"))).toEqual([
      "openai: gpt-5, gpt-5-mini",
      "openrouter: openai/gpt-5",
    ]);
  });

  it("answers nothing for a query nobody matches", () => {
    expect(groupsOf(MODELS, nameOf, "llama")).toEqual([]);
  });
});

describe("recent models", () => {
  const gpt = modelKey(MODELS[0]);
  const sonnet = modelKey(MODELS[2]);

  it("puts the latest pick first and never lists one twice", () => {
    expect(withRecent([gpt, sonnet], sonnet)).toEqual([sonnet, gpt]);
  });

  it("keeps five", () => {
    const six = ["a", "b", "c", "d", "e", "f"].reduce<string[]>(withRecent, []);
    expect(six).toEqual(["f", "e", "d", "c", "b"]);
  });

  it("offers only the ones still served, most recent first", () => {
    const gone = modelKey(model("removed", "old-model"));
    expect(recentOf(MODELS, [sonnet, gone, gpt]).map((m) => m.id)).toEqual([
      "anthropic/claude-sonnet-5",
      "gpt-5",
    ]);
  });

  it("tells one provider's model from another's with the same id", () => {
    const twice = [model("a", "same"), model("b", "same")];
    expect(recentOf(twice, [modelKey(twice[1])]).map((m) => m.providerId)).toEqual(["b"]);
  });

  it("reads a stored list and nothing else", () => {
    expect(parseRecent(JSON.stringify([gpt, sonnet]))).toEqual([gpt, sonnet]);
    expect(parseRecent(null)).toEqual([]);
    expect(parseRecent("{ not json")).toEqual([]);
    expect(parseRecent('{"a":1}')).toEqual([]);
    expect(parseRecent('["ok", 3, null]')).toEqual(["ok"]);
  });
});
