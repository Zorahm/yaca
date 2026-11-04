/** Compact agent overview; expansion and filters belong to the reader. */
import { CaretDown, CaretRight, Check, X } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";

import { agentPrompt, agentsOf, agentTokens, orderedAgents, tally } from "../../core/agents";
import { inProcessesPanel } from "../../core/backgroundCommands";
import type { ActionRow, Feed } from "../../core/feed";
import { t, tp } from "../../i18n";
import { agentIssue, agentTitle } from "../agentState";
import { caption } from "../captions";
import { durationOf, tokens } from "../format";
import { Spinner } from "../RowSign";
import { AgentDetails, RetryAgentButton } from "./AgentDetails";
import { Panel } from "./Panel";

type Filter = "all" | "running" | "failed";

export function AgentsPanel({ feed, focus, onOpenFile, onRetry }: {
  feed: Feed;
  focus: string[];
  onOpenFile: (path: string, line?: number) => void;
  onRetry: (row: ActionRow) => Promise<void>;
}) {
  const agents = agentsOf(feed);
  const sum = tally(agents);
  const [open, setOpen] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const list = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setOpen(focus[0] ?? null);
    if (!focus[0]) return;
    setFilter("all");
    setQuery("");
    const timer = requestAnimationFrame(() => list.current?.querySelector(`[data-agent="${CSS.escape(focus[0])}"]`)?.scrollIntoView({ block: "nearest" }));
    return () => cancelAnimationFrame(timer);
  }, [focus]);

  const needle = query.trim().toLocaleLowerCase();
  const shown = orderedAgents(agents).filter((row) => {
    if (filter === "running" && row.state !== "running") return false;
    if (filter === "failed" && !agentIssue(row)) return false;
    return !needle || `${agentTitle(row)}\n${agentPrompt(row)}`.toLocaleLowerCase().includes(needle);
  });

  return <Panel>
    {agents.length === 0 ? <div className="panel-empty">{t("agents.empty")}</div> : <>
      <div className="agents-overview">
        <div className="agents-summary">
          <span>{tp("agents.count", sum.count)}</span>
          {sum.running > 0 && <span>{tp("agents.running", sum.running)}</span>}
          {sum.failed > 0 && <span className="agents-summary-failed">{tp("agents.failed", sum.failed)}</span>}
          {sum.tokens !== null && <span>{tokens(sum.tokens)}</span>}
        </div>
        <div className="agents-filters" role="group">
          {(["all", "running", "failed"] as const).map((value) => <button key={value} aria-pressed={filter === value} onClick={() => setFilter(value)}>
            {t(value === "all" ? "agents.filter_all" : value === "running" ? "agents.filter_running" : "agents.filter_failed")}
            <span>{value === "all" ? sum.count : value === "running" ? sum.running : sum.failed}</span>
          </button>)}
        </div>
        {agents.length >= 10 && <input className="agents-search" type="search" aria-label={t("agents.search")} placeholder={t("agents.search")} value={query} onChange={(event) => setQuery(event.target.value)} />}
      </div>
      <div className="agents-list" ref={list}>
        {shown.length === 0 && <div className="agents-no-matches">{t("agents.no_matches")}</div>}
        {shown.map((row) => <Subagent key={row.callId} row={row} open={open === row.callId} onToggle={() => setOpen(open === row.callId ? null : row.callId)} onOpenFile={onOpenFile} onRetry={onRetry} />)}
      </div>
    </>}
  </Panel>;
}

function Subagent({ row, open, onToggle, onOpenFile, onRetry }: {
  row: ActionRow;
  open: boolean;
  onToggle: () => void;
  onOpenFile: (path: string, line?: number) => void;
  onRetry: (row: ActionRow) => Promise<void>;
}) {
  const issue = agentIssue(row);
  const running = row.state === "running";
  const spent = agentTokens(row);
  const children = row.children.filter((child) => !inProcessesPanel(child));
  const live = [...row.children].reverse().find((child) => child.state === "running");
  const [retryState, setRetryState] = useState<"idle" | "sending" | "sent" | "failed">("idle");

  async function retry() {
    setRetryState("sending");
    try { await onRetry(row); setRetryState("sent"); }
    catch { setRetryState("failed"); }
  }
  const retryControl = agentPrompt(row) ? <RetryAgentButton state={retryState} onRetry={() => void retry()} /> : null;

  return <section className="subagent" data-agent={row.callId}>
    <button className="subagent-head" aria-expanded={open} onClick={onToggle} title={agentTitle(row)}>
      {running ? <Spinner /> : <span className={`sign ${issue ? "subagent-bad" : "subagent-done"}`} aria-hidden="true">{issue ? <X size={14} /> : <Check size={14} />}</span>}
      <span className="subagent-title">{agentTitle(row)}</span>
      <span className="subagent-measures">
        {row.meta && <span>{durationOf(row.meta.durationMs)}</span>}
        {spent !== null && <span title={t("agents.tokens", { n: tokens(spent) })}>{tokens(spent)}</span>}
      </span>
      <span className="caret" aria-hidden="true">{open ? <CaretDown size={12} /> : <CaretRight size={12} />}</span>
    </button>
    <div className="subagent-statusline">
      <span className={issue ? "subagent-status-bad" : ""}>
        {running ? live?.tool === "BashWait" ? t("agents.waiting_background") : live ? caption(live) : t("agents.working") : issue ? t(issue.reason) : tp("agents.steps", children.length)}
      </span>
      {issue && !open && retryControl}
    </div>
    {retryState === "failed" && <div className="subagent-retry-error" role="status">{t("agents.retry_failed")}</div>}
    {open && <AgentDetails row={row} retryControl={retryControl} onOpenFile={onOpenFile} />}
  </section>;
}
