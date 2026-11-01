import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { ErrorBlock } from "./Blocks";
import { setLocale, t } from "../i18n";

describe("error explanations", () => {
  it("shows a localized explanation and retains the raw network cause in details", () => {
    setLocale("en");
    const html = renderToStaticMarkup(createElement(ErrorBlock, {
      code: "network", message: "error decoding response body: unexpected EOF",
      retryable: true, onRetry: () => {},
    }));
    expect(html).toContain(t("err.network_help"));
    expect(html).toContain("<details");
    expect(html).toContain("unexpected EOF");
    expect(html).toContain(t("err.retry"));
  });

  it("keeps unfamiliar failures readable and respects retryability", () => {
    const html = renderToStaticMarkup(createElement(ErrorBlock, {
      code: "custom_failure", message: "A specific failure", retryable: false, onRetry: () => {},
    }));
    expect(html).toContain("A specific failure");
    expect(html).toContain("custom_failure");
    expect(html).not.toContain("<button");
  });
});
