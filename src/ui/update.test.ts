/**
 * The install of a newer release, as the screens do the arithmetic of it.
 *
 * The download itself is the shell's (`update::install`): what can be wrong
 * here is the number over the bar and the words under a refusal, so those are
 * the pure parts tested.
 */

import { describe, expect, it } from "vitest";

import { installErrorKey, percentOf, updateErrorKey } from "./update";

describe("the bar of a download", () => {
  it("is whole percents of what has arrived", () => {
    expect(percentOf(0, 1000)).toBe(0);
    expect(percentOf(250, 1000)).toBe(25);
    expect(percentOf(999, 1000)).toBe(100);
  });

  it("says nothing while the size is unknown", () => {
    expect(percentOf(500, null)).toBeNull();
    expect(percentOf(500, 0)).toBeNull();
  });

  it("never leaves the bar's ends, whatever the server said the size was", () => {
    // A chunk reported after the total, or a total smaller than what arrived:
    // both happen, and neither may draw a bar wider than itself.
    expect(percentOf(1200, 1000)).toBe(100);
    expect(percentOf(-5, 1000)).toBe(0);
  });
});

describe("the words for a refusal", () => {
  it("names the reason it was given", () => {
    expect(installErrorKey("busy")).toBe("about.update_busy");
    expect(installErrorKey("signature")).toBe("about.update_signature");
    expect(installErrorKey("no_artifact")).toBe("about.update_no_artifact");
    expect(installErrorKey("dev_build")).toBe("about.update_dev");
    expect(installErrorKey("offline")).toBe("about.update_offline");
  });

  it("falls back to the generic failure for anything else", () => {
    expect(installErrorKey("install_failed")).toBe("about.update_install_error");
    expect(installErrorKey("status:502")).toBe("about.update_install_error");
  });

  it("keeps the check's codes apart from the install's", () => {
    expect(updateErrorKey("rate_limited")).toBe("about.update_rate_limited");
    expect(updateErrorKey("whatever")).toBe("about.update_failed");
  });
});
