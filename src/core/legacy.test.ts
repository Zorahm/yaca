/**
 * The composer drafts (§7.4) written by the edoC build come back under the new
 * names — and a draft typed after the rename is not overwritten by an older
 * one.
 */

import { describe, expect, it } from "vitest";

import { adoptStorage } from "./legacy";

/** `Storage` as the browser gives it: the tests run in node, which has none. */
function storage(entries: Record<string, string>): Storage {
  const map = new Map(Object.entries(entries));
  return {
    get length() {
      return map.size;
    },
    key: (i) => [...map.keys()][i] ?? null,
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => void map.set(key, value),
    removeItem: (key) => void map.delete(key),
    clear: () => map.clear(),
  };
}

function contents(s: Storage): Record<string, string> {
  const out: Record<string, string> = {};
  for (let i = 0; i < s.length; i += 1) {
    const key = s.key(i);
    if (key !== null) out[key] = s.getItem(key) ?? "";
  }
  return out;
}

describe("adoptStorage", () => {
  it("renames every old key and leaves no copy behind", () => {
    const s = storage({
      "edoc.draft.ses_1": "почини даты",
      "edoc.draft.ses_2": "и тесты",
      unrelated: "как было",
    });

    adoptStorage(s);

    expect(contents(s)).toEqual({
      "yaca.draft.ses_1": "почини даты",
      "yaca.draft.ses_2": "и тесты",
      unrelated: "как было",
    });
  });

  it("keeps a draft written under the new name", () => {
    const s = storage({
      "edoc.draft.ses_1": "старый",
      "yaca.draft.ses_1": "новый",
    });

    adoptStorage(s);

    expect(contents(s)).toEqual({ "yaca.draft.ses_1": "новый" });
  });
});
