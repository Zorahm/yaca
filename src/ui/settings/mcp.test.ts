/**
 * The MCP server form, without the form: text in, a declaration out.
 *
 * What is checked is what would otherwise reach the file wrong — an argument
 * with a space split in two, a command left under an address, a timeout
 * written by hand dropped by a save.
 */

import { describe, expect, it } from "vitest";

import type { McpServerSettings } from "../../core/client";
import { draftOf, emptyDraft, problemsOf, settingsOf, targetOf } from "./mcp";

const stdio: McpServerSettings = {
  command: "npx",
  args: ["-y", "server-files", "C:/My Projects"],
  env: { TOKEN: "${TOKEN}", MODE: "a=b" },
  cwd: "tools",
  type: null,
  url: null,
  headers: {},
  disabled: false,
  startupTimeoutSecs: 90,
  toolTimeoutSecs: null,
};

const http: McpServerSettings = {
  command: null,
  args: [],
  env: {},
  cwd: null,
  type: "http",
  url: "https://example.test/mcp",
  headers: { Authorization: "Bearer ${TOKEN}" },
  disabled: true,
  startupTimeoutSecs: null,
  toolTimeoutSecs: null,
};

describe("an MCP server in the form", () => {
  it("survives a round trip unchanged", () => {
    expect(settingsOf(draftOf("files", stdio))).toEqual(stdio);
    expect(settingsOf(draftOf("tracker", http))).toEqual(http);
  });

  it("keeps an argument with a space whole", () => {
    expect(draftOf("files", stdio).args.split("\n")).toHaveLength(3);
  });

  it("splits a pair at the first separator only", () => {
    const draft = { ...emptyDraft(), name: "s", command: "x", env: "MODE=a=b" };
    expect(settingsOf(draft).env).toEqual({ MODE: "a=b" });

    const remote = { ...draftOf("t", http), headers: "X-Time: 12:30" };
    expect(settingsOf(remote).headers).toEqual({ "X-Time": "12:30" });
  });

  it("writes only the half its kind uses", () => {
    const switched = { ...draftOf("files", stdio), kind: "http" as const, url: "https://x.test" };
    const written = settingsOf(switched);
    expect(written.command).toBeNull();
    expect(written.args).toEqual([]);
    expect(written.url).toBe("https://x.test");
    // The timeout the form does not show is still there.
    expect(written.startupTimeoutSecs).toBe(90);
  });

  it("is recognised by its command or its address", () => {
    expect(targetOf(stdio)).toBe("npx -y server-files C:/My Projects");
    expect(targetOf(http)).toBe("https://example.test/mcp");
  });
});

describe("what stops a save", () => {
  const good = { ...emptyDraft(), name: "files", command: "server-files" };

  it("is nothing for a complete draft", () => {
    expect(problemsOf(good, [], true)).toEqual([]);
    expect(problemsOf(draftOf("tracker", http), ["tracker"], false)).toEqual([]);
  });

  it("is a name that cannot be part of a tool name, or is taken", () => {
    expect(problemsOf({ ...good, name: "" }, [], true)).toEqual(["mcp.problem_name_empty"]);
    expect(problemsOf({ ...good, name: "my files" }, [], true)).toEqual(["mcp.problem_name_chars"]);
    expect(problemsOf(good, ["files"], true)).toEqual(["mcp.problem_name_taken"]);
  });

  it("is a missing command, or a pair without its separator", () => {
    expect(problemsOf({ ...good, command: " " }, [], true)).toEqual(["mcp.problem_command_empty"]);
    expect(problemsOf({ ...good, env: "TOKEN" }, [], true)).toEqual(["mcp.problem_env"]);
  });

  it("is an address that is not one", () => {
    const remote = { ...emptyDraft(), name: "t", kind: "http" as const };
    expect(problemsOf({ ...remote, url: "example.test" }, [], true)).toEqual(["mcp.problem_url"]);
    expect(
      problemsOf({ ...remote, url: "https://x.test", headers: "Authorization" }, [], true),
    ).toEqual(["mcp.problem_headers"]);
  });
});
