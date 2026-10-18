import { describe, expect, it } from "vitest";

import { LOCALE_CODES, translate } from "../i18n";
import {
  alwaysOffered,
  allowDigit,
  INVARIANT_LINE,
  rowAnswer,
  unsavedLines,
  type Press,
} from "./permissionRows";

const plain: Press = { key: "2", ctrl: false, shift: false };
const press = (key: string, ctrl = false, shift = false): Press => ({ key, ctrl, shift });

/** An invariant asks with no scopes (§8): nothing can be granted for it. */
const invariant = { scopes: [] };
const ordinary = { scopes: [{ type: "tool" as const, name: "Bash" }] };

describe("row of answers for an invariant (§5.7)", () => {
  it("offers no 'always' at all", () => {
    expect(alwaysOffered(invariant)).toBe(false);
  });

  it("gives the consent the digit that is left, 2", () => {
    expect(allowDigit(invariant)).toBe("2");
    expect(rowAnswer(press("2"), false, "2")).toBe("allow");
  });

  it("does not answer digit 3 with anything", () => {
    expect(rowAnswer(press("3"), false, "2")).toBe("none");
  });

  it("keeps ⌃⇧⏎ from becoming a consent", () => {
    expect(rowAnswer(press("Enter", true, true), false, "2")).toBe("none");
  });

  it("keeps ⌃⏎ and 1 where they were", () => {
    expect(rowAnswer(press("Enter", true, false), false, "2")).toBe("allow");
    expect(rowAnswer(press("1"), false, "2")).toBe("deny");
  });
});

describe("row of answers for an ordinary request", () => {
  it("offers 'always' and puts consent on 3", () => {
    expect(alwaysOffered(ordinary)).toBe(true);
    expect(allowDigit(ordinary)).toBe("3");
    expect(rowAnswer(press("2"), true, "3")).toBe("always");
    expect(rowAnswer(press("3"), true, "3")).toBe("allow");
  });

  it("maps ⌃⇧⏎ to 'always' and ⌃⏎ to consent", () => {
    expect(rowAnswer(press("Enter", true, true), true, "3")).toBe("always");
    expect(rowAnswer(press("Enter", true, false), true, "3")).toBe("allow");
  });

  it("ignores keys that are not on the row", () => {
    expect(rowAnswer(press("x"), true, "3")).toBe("none");
    expect(rowAnswer(plain, true, "3")).toBe("always");
  });
});

describe("invariant explanation", () => {
  it("has a line for commit, push, an unresolved git command and git config", () => {
    expect(INVARIANT_LINE.gitCommit).toBe("ask.invariant.git");
    expect(INVARIANT_LINE.gitPush).toBe("ask.invariant.git");
    expect(INVARIANT_LINE.gitUnresolved).toBe("ask.invariant.git_unresolved");
    expect(INVARIANT_LINE.gitConfig).toBe("ask.invariant.git_config");
  });

  it("is translated in every language, never the raw key", () => {
    for (const key of Object.values(INVARIANT_LINE)) {
      for (const locale of LOCALE_CODES) {
        const text = translate(locale, key);
        expect(text).not.toBe(key);
        expect(text.length).toBeGreaterThan(0);
      }
    }
    expect(translate("ru", "ask.invariant.git")).toContain("только по вашей команде");
  });
});

describe("what a directory delete takes away (§5.7)", () => {
  it("counts the skipped directories that are not listed", () => {
    const lines = unsavedLines({ dirs: ["target", "node_modules"], dirCount: 5, files: 4, roots: [] });
    expect(lines.dirs).toEqual(["target", "node_modules"]);
    expect(lines.moreDirs).toBe(3);
    expect(lines.files).toBe(4);
  });

  it("has no extra directories when all of them are listed", () => {
    const lines = unsavedLines({ dirs: ["dist"], dirCount: 1, files: 0, roots: [] });
    expect(lines.moreDirs).toBe(0);
  });

  it("never reports a negative count", () => {
    const lines = unsavedLines({ dirs: ["a", "b", "c"], dirCount: 2, files: 0, roots: [] });
    expect(lines.moreDirs).toBe(0);
  });

  it("names the additional roots a write would not be covered in", () => {
    const lines = unsavedLines({ dirs: [], dirCount: 0, files: 0, roots: ["C:/other"] });
    expect(lines.roots).toEqual(["C:/other"]);
  });

  it("has the closing promise in both languages", () => {
    expect(translate("ru", "ask.unsaved_note")).toBe("Откат этого не вернёт");
    expect(translate("en", "ask.unsaved_note")).not.toBe("ask.unsaved_note");
  });
});
