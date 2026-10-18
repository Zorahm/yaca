import { describe, expect, it } from "vitest";

import { parseInline, parseMarkdown, plainText, slugOf } from "./markdownTree";

describe("markdown blocks", () => {
  it("reads ATX and setext headings with unique anchors", () => {
    const tree = parseMarkdown("# Title\n\nSub\n---\n\n## Title\n");
    expect(tree.map((b) => b.t === "heading" && [b.level, b.id])).toEqual([
      [1, "title"],
      [2, "sub"],
      [2, "title-1"],
    ]);
  });

  it("keeps an unclosed fence as code", () => {
    const [block] = parseMarkdown("```ts\nlet a = 1;");
    expect(block).toEqual({ t: "code", info: "ts", text: "let a = 1;" });
  });

  it("nests lists and reads task boxes", () => {
    const [list] = parseMarkdown("- [x] done\n- [ ] todo\n  - inner\n  - inner 2\n");
    if (list.t !== "list") throw new Error("not a list");
    expect(list.items.map((i) => i.task)).toEqual([true, false]);
    const nested = list.items[1].blocks.find((b) => b.t === "list");
    expect(nested && nested.t === "list" && nested.items).toHaveLength(2);
  });

  it("starts an ordered list where the file says", () => {
    const [list] = parseMarkdown("3. a\n4. b\n");
    expect(list).toMatchObject({ t: "list", ordered: true, start: 3 });
  });

  it("marks a list loose when items are separated by blank lines", () => {
    const [tight] = parseMarkdown("- a\n- b\n");
    const [loose] = parseMarkdown("- a\n\n- b\n");
    expect(tight).toMatchObject({ loose: false });
    expect(loose).toMatchObject({ loose: true });
  });

  it("puts blocks inside a quote", () => {
    const [quote] = parseMarkdown("> # Head\n> text\n");
    if (quote.t !== "quote") throw new Error("not a quote");
    expect(quote.blocks.map((b) => b.t)).toEqual(["heading", "para"]);
  });

  it("reads tables and rules, and drops comments", () => {
    const tree = parseMarkdown("<!-- hidden -->\n\n| a | b |\n|---|--:|\n| 1 | 2 |\n\n***\n");
    expect(tree.map((b) => b.t)).toEqual(["table", "hr"]);
  });

  it("resolves reference links", () => {
    const [para] = parseMarkdown("see [docs][d]\n\n[d]: https://example.com \"Docs\"\n");
    if (para.t !== "para") throw new Error("not a paragraph");
    expect(para.c[1]).toMatchObject({ t: "link", href: "https://example.com", title: "Docs" });
  });
});

describe("markdown inline", () => {
  it("handles marks, nested marks and code that shields them", () => {
    const nodes = parseInline("**bold *and em***, ~~gone~~ and `**raw**`");
    expect(nodes.map((n) => n.t)).toEqual(["strong", "text", "del", "text", "code"]);
    expect(nodes[4]).toEqual({ t: "code", v: "**raw**" });
  });

  it("keeps snake_case words intact", () => {
    expect(plainText(parseInline("a snake_case_name here"))).toBe("a snake_case_name here");
  });

  it("links, autolinks and images", () => {
    const nodes = parseInline("[a](https://x.io) <https://y.io> ![pic](p.png) www.z.io.");
    expect(nodes.filter((n) => n.t === "link")).toHaveLength(3);
    expect(nodes.find((n) => n.t === "image")).toMatchObject({ alt: "pic", src: "p.png" });
  });

  it("escapes and decodes entities", () => {
    expect(plainText(parseInline("\\*not\\* &amp; &#65;"))).toBe("*not* & A");
  });

  it("slugs headings like GitHub", () => {
    expect(slugOf("Hello, World! `x`")).toBe("hello-world-x");
  });
});
