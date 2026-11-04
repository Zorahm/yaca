import { CaretDown, CaretRight } from "@phosphor-icons/react";
import { useRef, useState, type ReactNode } from "react";
import { agentPrompt, agentTokens } from "../../core/agents";
import { inProcessesPanel } from "../../core/backgroundCommands";
import type { ActionRow } from "../../core/feed";
import { t } from "../../i18n";
import { agentIssue } from "../agentState";
import { ActionRowView } from "../ActionRow";
import { Markdown } from "../Markdown";

type OpenFile = (path: string, line?: number) => void;

export function RetryAgentButton({ state, onRetry }: {
  state: "idle" | "sending" | "sent" | "failed";
  onRetry: () => void;
}) {
  return <button className="agent-retry" disabled={state === "sending" || state === "sent"} title={t("agents.retry_help")} onClick={onRetry}>
    {t(state === "sent" ? "agents.retry_requested" : state === "sending" ? "agents.retry_sending" : "err.retry")}
  </button>;
}

export function AgentDetails({ row, retryControl, onOpenFile }: {
  row: ActionRow;
  retryControl: ReactNode;
  onOpenFile: OpenFile;
}) {
  const running = row.state === "running";
  const issue = agentIssue(row);
  const prompt = agentPrompt(row);
  const children = row.children.filter((child) => !inProcessesPanel(child));
  const ordered = running ? [...children].reverse() : children;
  const [actionsOpen, setActionsOpen] = useState(running);
  const [taskOpen, setTaskOpen] = useState(false);
  const [wholePrompt, setWholePrompt] = useState(false);
  const [allActions, setAllActions] = useState(false);
  const [logOpen, setLogOpen] = useState(false);
  const log = useRef<HTMLDivElement>(null);
  const recent = allActions ? ordered : running ? ordered.slice(0, 3) : ordered.slice(-3);
  const longPrompt = prompt.length > 220 || prompt.split("\n").length > 3;

  const actions = children.length > 0 && <section className="subagent-section subagent-actions">
    <button className="subagent-section-toggle" aria-expanded={actionsOpen} onClick={() => setActionsOpen(!actionsOpen)}>
      <span>{t("agents.actions")} · {children.length}</span>
      {actionsOpen ? <CaretDown size={12} /> : <CaretRight size={12} />}
    </button>
    {actionsOpen && <div className="subagent-action-list">
      {recent.map((child) => <ActionRowView key={child.callId} row={child} onOpenFile={onOpenFile} />)}
      {!allActions && children.length > 3 && <button className="subagent-more" onClick={() => setAllActions(true)}>{t("agents.more_actions", { n: children.length - 3 })}</button>}
    </div>}
  </section>;

  function openLog() {
    setLogOpen(true);
    setActionsOpen(true);
    setAllActions(true);
    requestAnimationFrame(() => log.current?.scrollIntoView({ block: "nearest" }));
  }

  return <div className="subagent-detail">
    {running && actions}
    {row.state === "done" && <section className="subagent-section subagent-result">
      <h3>{t("agents.result")}</h3>
      <Markdown text={row.output} onOpenFile={onOpenFile} />
    </section>}
    {issue && <div className="subagent-error">
      <div className="subagent-error-title">{t("agents.error_title")}</div>
      <div>{t(issue.reason)}</div>
      <div className="subagent-error-help">{t(issue.help)}</div>
      {row.children.length === 0 && (agentTokens(row) ?? 0) > 0 && <div className="subagent-error-help">{t("agents.no_actions_help")}</div>}
      <div className="subagent-error-actions">
        {retryControl}
        <button className="agent-retry" onClick={openLog}>{t("agents.open_log")}</button>
      </div>
    </div>}
    {!running && actions}
    {prompt && <section className="subagent-section subagent-task">
      <button className="subagent-section-toggle" aria-expanded={taskOpen} onClick={() => setTaskOpen(!taskOpen)}>
        <span>{t("agents.task")}</span>
        {taskOpen ? <CaretDown size={12} /> : <CaretRight size={12} />}
      </button>
      {taskOpen && <div>
        <div className={`subagent-task-text${longPrompt && !wholePrompt ? " subagent-task-folded" : ""}`}>{prompt}</div>
        {longPrompt && <button className="subagent-more" onClick={() => setWholePrompt(!wholePrompt)}>{t(wholePrompt ? "agents.prompt_less" : "agents.prompt_more")}</button>}
      </div>}
    </section>}
    {logOpen && <div className="subagent-log" ref={log}>
      <h3>{t("agents.log_title")}</h3>
      <pre>{[row.message, ...row.children.map((child) => `${child.tool}${child.target ? ` ${child.target}` : ""} [${child.state}]\n${child.message ?? child.output}`)].filter(Boolean).join("\n\n") || t("agents.log_empty")}</pre>
    </div>}
  </div>;
}
