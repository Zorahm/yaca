import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { Markdown } from "./Markdown";

describe("chat Markdown", () => {
  it("keeps commit details under their numbered item and renders code inside bold text", () => {
    const text = [
      "1. **`cd4dc3d` fix(release)**",
      "",
      "   - First detail",
      "   - Second detail",
      "",
      "2. **`7c1bdbd` feat(reasoning)**",
    ].join("\n");

    const html = renderToStaticMarkup(createElement(Markdown, { text }));
    expect(html).toContain(
      '<ol start="1" class="md-loose"><li><p><strong><code class="code-inline">cd4dc3d</code> fix(release)</strong></p>',
    );
    expect(html).toContain(
      '<ul><li><span class="md-tight">First detail</span></li><li><span class="md-tight">Second detail</span></li></ul></li><li>',
    );
    expect(html).toContain('<strong><code class="code-inline">7c1bdbd</code> feat(reasoning)</strong>');
  });

  it("honors the start of a separate ordered list", () => {
    const html = renderToStaticMarkup(createElement(Markdown, { text: "1. First\n\n- Note\n\n3. Third\n4. Fourth" }));
    expect(html).toContain('<ol start="3">');
  });

  it("renders an unfinished code fence and preserves its contents while streaming", () => {
    const html = renderToStaticMarkup(createElement(Markdown, {
      text: "````text\n\tIndented\n```\n[ref]: example.md\n<script>alert(1)</script>",
    }));
    expect(html).toContain('<pre class="output">\tIndented\n```\n[ref]: example.md\n&lt;script&gt;alert(1)&lt;/script&gt;</pre>');
  });

  it("shows external link destinations as text", () => {
    const html = renderToStaticMarkup(createElement(Markdown, { text: "[**Docs**](https://example.com)" }));
    expect(html).toContain("<strong>Docs</strong>");
    expect(html).toContain("(https://example.com)");
    expect(html).not.toContain("<a ");
  });
});
