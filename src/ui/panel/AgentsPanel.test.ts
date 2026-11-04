import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ActionRow, Feed } from "../../core/feed";
import { emptyFeed } from "../../core/feed";
import { orderedAgents } from "../../core/agents";
import { demoAgentFileView, demoAgentRetry } from "../../core/demoAgents";
import { t } from "../../i18n";
import { agentIssue, retryAgentMessage } from "../agentState";
import { fileReference } from "../FileReferences";
import { Markdown } from "../Markdown";
import { durationOf } from "../format";
import { AgentDetails } from "./AgentDetails";
import { AgentsPanel } from "./AgentsPanel";
import { Panel, PanelFrameContext, type PanelFrame } from "./Panel";

const row = (patch: Partial<ActionRow> = {}): ActionRow => ({
  callId: "a1", tool: "Task", target: null, description: "Find date parsers", input: { prompt: "Find the parsers without changing files." },
  state: "done", message: null, output: "Report from the model", meta: { lines: 1, items: null, truncated: false, durationMs: 14000 },
  fact: null, diff: null, children: [], tokens: { input: 24000, output: 900, cacheRead: 0, cacheWrite: 0 }, ...patch,
});
const frame: PanelFrame = { current: "agents", filePath: null, hasAgents: true, width: 380, initialWidth: 380, bounds: null, onSwitch: () => {}, onClose: () => {}, onResize: () => {}, agentsStatus: { running: 1, failed: 1 } };
const sample = [row(), row({ callId: "a2", state: "running", meta: null, output: "", children: [row({ callId: "read", tool: "Read", target: "src/segments.test.ts", state: "running", description: null, input: {}, meta: null, output: "" })] }), row({ callId: "a3", state: "failed", meta: null, output: "", message: "the subagent returned nothing" })];
const feed: Feed = { ...emptyFeed, blocks: [{ kind: "actions", key: "agents", rows: sample }] };
const panel = (data: Feed = feed) => renderToStaticMarkup(createElement(PanelFrameContext.Provider, { value: frame }, createElement(AgentsPanel, { feed: data, focus: [], onOpenFile: () => {}, onRetry: async () => {} })));
const details = (agent: ActionRow) => renderToStaticMarkup(createElement(AgentDetails, { row: agent, retryControl: null, onOpenFile: () => {} }));

describe("agent overview", () => {
  it("keeps all agents collapsed, with live work and localized failure reasons visible", () => {
    const html = panel();
    expect(html.match(/class="subagent-head" aria-expanded="false"/g)).toHaveLength(3);
    expect(html).toContain("segments.test.ts");
    expect(html).toContain(t("agents.empty_result"));
    expect(html).toContain(durationOf(14000));
    expect(html).not.toContain("subagent-detail");
    expect(html).not.toContain("Report from the model");
    expect(html).not.toContain("the subagent returned nothing");
    expect(html).not.toContain("Find the parsers without changing files.");
  });

  it("orders active, failed, and completed agents without mutating the event rows", () => {
    expect(orderedAgents(sample).map(agent => agent.callId)).toEqual(["a2", "a3", "a1"]);
    expect(sample.map(agent => agent.callId)).toEqual(["a1", "a2", "a3"]);
    const html = panel();
    expect(html.indexOf('data-agent="a2"')).toBeLessThan(html.indexOf('data-agent="a3"'));
    expect(html.indexOf('data-agent="a3"')).toBeLessThan(html.indexOf('data-agent="a1"'));
  });

  it("offers search when ten or more agents are present", () => {
    expect(panel()).not.toContain('type="search"');
    const many: Feed = { ...feed, blocks: [{ kind: "actions", key: "many", rows: Array.from({ length: 10 }, (_, index) => row({ callId: `a${index}` })) }] };
    expect(panel(many)).toContain('type="search"');
  });

  it("marks the agents tab with both a running signal and a failure signal", () => {
    const html = renderToStaticMarkup(createElement(PanelFrameContext.Provider, { value: frame }, createElement(Panel, null, "content")));
    expect(html).toContain("panel-agent-spinner");
    expect(html).toContain("panel-agent-error");
  });
});

describe("expanded agent", () => {
  it("shows the report first and leaves its task behind a closed disclosure", () => {
    const html = details(row({ children: [row({ tool: "Read", target: "src/a.ts" })] }));
    expect(html.indexOf("Report from the model")).toBeLessThan(html.indexOf(t("agents.actions")));
    expect(html.indexOf(t("agents.actions"))).toBeLessThan(html.indexOf(t("agents.task")));
    expect(html).not.toContain("Find the parsers without changing files.");
    expect(html).not.toContain("subagent-prompt");
  });

  it("shows the three most recent actions for active work and offers the rest", () => {
    const children = Array.from({ length: 5 }, (_, index) => row({ tool: "Read", callId: `read${index}`, target: `src/file${index}.ts`, input: {}, state: "running", description: null, output: "", meta: null }));
    const html = details(row({ state: "running", meta: null, output: "", children }));
    expect(html).toContain("file4.ts");
    expect(html).toContain("file2.ts");
    expect(html).not.toContain("file1.ts");
    expect(html.indexOf("file4.ts")).toBeLessThan(html.indexOf("file3.ts"));
    expect(html).toContain(t("agents.more_actions", { n: 2 }));
  });

  it("explains spent tokens without tools and keeps raw system messages in the log", () => {
    const html = details(sample[2]);
    expect(html).toContain(t("agents.error_title"));
    expect(html).toContain(t("agents.no_actions_help"));
    expect(html).toContain(t("agents.open_log"));
    expect(html).not.toContain("the subagent returned nothing");
    expect(html).not.toContain("Report from the model");
  });
});

describe("agent failures and retry", () => {
  it("distinguishes empty responses, timeout, cancellation, permission refusal, and other failures", () => {
    const failed = (message: string | null) => row({ state: "failed", message });
    expect(agentIssue(failed("subagent returned empty response"))?.reason).toBe("agents.empty_result");
    expect(agentIssue(failed("request timed out"))?.reason).toBe("agents.timeout");
    expect(agentIssue(failed(null))?.reason).toBe("agents.cancelled");
    expect(agentIssue(failed("cancelled"))?.reason).toBe("agents.cancelled");
    expect(agentIssue(row({ state: "denied" }))?.reason).toBe("agents.denied");
    expect(agentIssue(failed("HTTP 503"))?.reason).toBe("agents.execution_failed");
  });

  it("retries the specific task with its selected agent, model and background option", () => {
    const message = retryAgentMessage(row({ input: { prompt: "A precise independent task", agent: "researcher", model: "chosen-model", background: true } }));
    expect(message).toContain("A precise independent task");
    expect(message).toContain("researcher");
    expect(message).toContain("chosen-model");
    expect(message).toContain("\n\n");
    expect(message).not.toContain("\\n");
  });

  it("lets the stand replay a retry and read every file referenced by its reports", () => {
    const retry = demoAgentRetry({ type: "sendMessage", text: "Task: Find the description of the segment date format in docs/.", attachments: [] });
    expect(retry?.started.some(event => event.type === "toolCallStarted" && event.tool === "Task")).toBe(true);
    expect(retry?.finished[retry.finished.length - 1]?.type).toBe("toolCallFinished");
    expect(demoAgentFileView("src/zone.ts")).not.toBeNull();
    expect(demoAgentFileView("docs/segments.md")).not.toBeNull();
  });
});

describe("file references in reports", () => {
  it("opens local paths with line numbers and leaves URLs and clock times alone", () => {
    expect(fileReference("src/segments.ts:13")).toEqual({ path: "src/segments.ts", line: 13 });
    expect(fileReference("C:\\work dir\\a.ts:14:22")).toEqual({ path: "C:\\work dir\\a.ts", line: 14 });
    expect(fileReference("https://example.com/a.ts:13")).toBeNull();
    expect(fileReference("14:22")).toBeNull();
    const html = renderToStaticMarkup(createElement(Markdown, { text: "`src/segments.ts:13` and src/zone.ts:1", onOpenFile: () => {} }));
    expect(html.match(/class="file-reference code-inline"/g)).toHaveLength(2);
    const plain = renderToStaticMarkup(createElement(Markdown, { text: "`src/segments.ts:13`" }));
    expect(plain).not.toContain("<button");
  });
});
