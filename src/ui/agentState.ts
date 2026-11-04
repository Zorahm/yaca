import type { ActionRow } from "../core/feed";
import { agentPrompt } from "../core/agents";
import { t, type Key } from "../i18n";

export function agentTitle(row: ActionRow): string {
  return row.description?.trim() || agentPrompt(row).split("\n").find((line) => line.trim())?.trim() || t("agents.untitled");
}

export function agentIssue(row: ActionRow): { reason: Key; help: Key } | null {
  if (row.state === "running" || row.state === "done") return null;
  if (row.state === "stopped" || (row.state === "failed" && row.message === null) || /\bcancelled\b|\bcanceled\b|\binterrupted\b/i.test(row.message ?? "")) {
    return { reason: "agents.cancelled", help: "agents.cancelled_help" };
  }
  if (row.state === "denied" || row.state === "rejected") return { reason: "agents.denied", help: "agents.denied_help" };
  if (/subagent returned (?:nothing|empty response)|no report|wrote no report/i.test(row.message ?? "")) {
    return { reason: "agents.empty_result", help: "agents.empty_help" };
  }
  if (/timed?\s*out|timeout|deadline/i.test(row.message ?? "")) return { reason: "agents.timeout", help: "agents.timeout_help" };
  return { reason: "agents.execution_failed", help: "agents.failed_help" };
}

/** A retry is a normal user request to the lead agent, keeping the permission path intact. */
export function retryAgentMessage(row: ActionRow): string {
  const input = typeof row.input === "object" && row.input !== null ? row.input as Record<string, unknown> : {};
  const parts = [t("agents.retry_message", { title: agentTitle(row), prompt: agentPrompt(row) })];
  if (typeof input.agent === "string" && input.agent) parts.push(t("agents.retry_agent", { agent: input.agent }));
  if (typeof input.model === "string" && input.model) parts.push(t("agents.retry_model", { model: input.model }));
  if (input.background === true) parts.push(t("agents.retry_background"));
  return parts.join("\n\n");
}
