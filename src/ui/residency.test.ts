import { describe, expect, it } from "vitest";

import { createResidency, residencyKey } from "./residency";

describe("residency", () => {
  it("marks each model by its provider and id", async () => {
    const residency = createResidency(
      async (provider): Promise<Record<string, boolean>> =>
        provider === "local" ? { small: true, large: false } : {},
    );

    const marks = await residency(["local", "anthropic"]);

    expect(marks.get(residencyKey("local", "small"))).toBe(true);
    expect(marks.get(residencyKey("local", "large"))).toBe(false);
    expect(marks.size).toBe(2);
  });

  it("stops asking a provider that said nothing", async () => {
    const asked: string[] = [];
    const residency = createResidency(async (provider) => {
      asked.push(provider);
      return {};
    });

    await residency(["openrouter"]);
    await residency(["openrouter"]);

    expect(asked).toEqual(["openrouter"]);
  });

  it("asks again a provider that did not answer", async () => {
    // A local server that is not up yet may well be up on the next open.
    let calls = 0;
    const residency = createResidency(async () => {
      calls += 1;
      throw new Error("connection refused");
    });

    expect((await residency(["local"])).size).toBe(0);
    await residency(["local"]);

    expect(calls).toBe(2);
  });

  it("asks each provider once however many models it has", async () => {
    const asked: string[] = [];
    const residency = createResidency(async (provider) => {
      asked.push(provider);
      return { a: true };
    });

    await residency(["local", "local", ""]);

    // An empty id is a model from before providers had ids: nobody to ask.
    expect(asked).toEqual(["local"]);
  });
});
