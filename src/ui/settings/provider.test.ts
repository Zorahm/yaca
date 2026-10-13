/**
 * The provider-draft logic that the panel form runs on every keystroke.
 *
 * The critical case is the §0 one: an empty header value is a legitimate
 * override — that is how a local gateway drops `authorization` — and must not
 * be mistaken for a missing one.
 *
 * Models are not validated here at all any more: they arrive from the provider
 * (`discoverModels`), and second-guessing its own answer would mean the window
 * having an opinion about what the provider serves.
 */

import { describe, expect, it } from "vitest";

import type { ProviderSpec } from "../../core/client";
import {
  draftFromSpec,
  draftProblems,
  draftToSpec,
  emptyDraft,
  mergedHeaders,
  needsKey,
  newProviderId,
  type ProviderDraft,
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
        supportsEffort: false,
        supportsThinking: false,
        contextSource: "default_guess",
        supportsImages: true,
      },
    ],
  };

  it("round-trips a spec through a draft unchanged", () => {
    expect(draftToSpec(draftFromSpec(spec))).toEqual(spec);
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
