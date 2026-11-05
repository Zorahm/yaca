import { describe, expect, it } from "vitest";
import { compactPluginPath, marketplaceSourceTarget, newerPluginVersion, showMarketplaceSource } from "./pluginPresentation";

describe("plugin presentation", () => {
  it("abbreviates the home directory and keeps the plugin name", () => {
    expect(compactPluginPath("C:/Users/test/.claude/plugins/marketplaces/local-desktop-app-uploads/plugins/zowork"))
      .toBe("~/.claude/plugins/…/zowork");
    expect(compactPluginPath("C:\\Users\\test\\plugins\\short")).toBe("~/plugins/short");
  });
  it("does not advertise downgrades or opaque revisions as updates", () => {
    expect(newerPluginVersion("0.6.0", "0.7.0")).toBe(true);
    expect(newerPluginVersion("0.7.0", "0.6.0")).toBe(false);
    expect(newerPluginVersion("1.0.0-beta", "1.0.0")).toBe(true);
    expect(newerPluginVersion("abcdef", "123456")).toBe(false);
  });
  it("uses browser URLs for repositories and paths for local sources", () => {
    expect(marketplaceSourceTarget("anthropics/claude-plugins-official")).toEqual({ kind: "url", value: "https://github.com/anthropics/claude-plugins-official" });
    expect(marketplaceSourceTarget("git@github.com:owner/repo.git")).toEqual({ kind: "url", value: "https://github.com/owner/repo" });
    expect(marketplaceSourceTarget("C:/plugins/local")).toEqual({ kind: "path", value: "C:/plugins/local" });
    expect(marketplaceSourceTarget("javascript:alert(1)")).toBeNull();
  });
  it("hides a repository source that repeats the marketplace name", () => {
    expect(showMarketplaceSource("claude-plugins-official", "anthropics/claude-plugins-official")).toBe(false);
    expect(showMarketplaceSource("impeccable", "https://github.com/owner/another.git")).toBe(true);
    expect(showMarketplaceSource("impeccable", "https://github.com/owner/impeccable.git")).toBe(true);
  });
});
