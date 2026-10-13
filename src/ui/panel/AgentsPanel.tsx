/**
 * The agents panel: every subagent of the conversation, folded to one row.
 *
 * A row opens into a small conversation of its own: the task the agent wrote
 * for the subagent, what the subagent did, and the report it handed back.
 * The actions are the same rows the feed draws, with the same diff and the
 * same way into the file panel, because an edit a subagent made is an edit to
 * the person's files.
 *
 * What the subagent said between its actions is not here: the core does not
 * send it, and tells the subagent so.
 *
 * The list is read from the feed's `Task` rows (`core/agents`); the panel
 * keeps only which rows are open. The plate in the feed opens the panel on
 * its own agents through `focus`.
 */

import { CaretDown, CaretRight, Check } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";

import { agentPrompt, agentsOf, agentTokens } from "../../core/agents";
import type { ActionRow, Feed } from "../../core/feed";
import { t, tp } from "../../i18n";
import { ActionRowView } from "../ActionRow";
import { tokens } from "../format";
import { Markdown } from "../Markdown";
import { RowSign } from "../RowSign";
import { Panel } from "./Panel";

/** A prompt longer than this is folded until asked for. */
const PROMPT_FOLD_CHARS = 360;
const PROMPT_FOLD_LINES = 6;

export function AgentsPanel({
  feed,
  focus,
  onOpenFile,
}: {
  feed: Feed;
  /** The agents to open: those of the plate that was pressed. */
  focus: string[];
  onOpenFile: (path: string) => void;
}) {
  const agents = agentsOf(feed);
  const [open, setOpen] = useState<Set<string>>(() => new Set(focus));
  const list = useRef<HTMLDivElement>(null);

  // A plate pressed while the panel is up: its agents open, and the first of
  // them comes into view.
  useEffect(() => {
    setOpen(new Set(focus));
    const first = focus[0];
    if (first === undefined) return;
    list.current
      ?.querySelector(`[data-agent="${CSS.escape(first)}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [focus]);

  function toggle(id: string) {
    setOpen((now) => {
      const next = new Set(now);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }

  return (
    <Panel>
      {agents.length === 0 ? (
        <div className="panel-empty">{t("agents.empty")}</div>
      ) : (
        <div className="panel-body" ref={list}>
          {agents.map((row) => (
            <Subagent
              key={row.callId}
              row={row}
              open={open.has(row.callId)}
              onToggle={() => toggle(row.callId)}
              onOpenFile={onOpenFile}
            />
          ))}
        </div>
      )}
    </Panel>
  );
}

function Subagent({
  row,
  open,
  onToggle,
  onOpenFile,
}: {
  row: ActionRow;
  open: boolean;
  onToggle: () => void;
  onOpenFile: (path: string) => void;
}) {
  const prompt = agentPrompt(row);
  const spent = agentTokens(row);
  const running = row.state === "running";
  const done = row.state === "done";

  return (
    <section className="subagent" data-agent={row.callId}>
      <button className="subagent-head" aria-expanded={open} onClick={onToggle}>
        {done ? (
          <span className="sign subagent-done" aria-hidden="true">
            <Check size={14} weight="bold" />
          </span>
        ) : (
          <RowSign row={row} />
        )}
        <span className="subagent-name">
          <span className="subagent-title">{row.description ?? firstLine(prompt)}</span>
          <span className="subagent-meta">
            {running && <span>{t("agents.working")}</span>}
            <span>{tp("agents.steps", row.children.length)}</span>
            {spent !== null && <span>{t("agents.tokens", { n: tokens(spent) })}</span>}
          </span>
        </span>
        <span className="caret" aria-hidden="true">
          {open ? <CaretDown size={12} /> : <CaretRight size={12} />}
        </span>
      </button>

      {open && (
        <div className="subagent-chat">
          {prompt !== "" && <Prompt text={prompt} />}

          {row.children.length > 0 && (
            <div className="group-list">
              {row.children.map((child) => (
                <ActionRowView key={child.callId} row={child} onOpenFile={onOpenFile} />
              ))}
            </div>
          )}

          {done && row.output.trim() !== "" && <Markdown text={row.output} />}

          {/* Ended without a report: the reason stands where the report would. */}
          {!running && !done && (
            <div className="subagent-failed">
              {row.message ?? t(row.state === "stopped" ? "act.stopped" : "act.cancelled")}
            </div>
          )}
        </div>
      )}
    </section>
  );
}

/** The task as the agent wrote it: the other side of this small conversation. */
function Prompt({ text }: { text: string }) {
  const long =
    text.length > PROMPT_FOLD_CHARS || text.split("\n").length > PROMPT_FOLD_LINES;
  const [whole, setWhole] = useState(false);

  return (
    <div className="subagent-prompt">
      <div className={long && !whole ? "subagent-prompt-text subagent-prompt-folded" : "subagent-prompt-text"}>
        {text}
      </div>
      {long && (
        <button className="subagent-prompt-more" onClick={() => setWhole(!whole)}>
          {t(whole ? "agents.prompt_less" : "agents.prompt_more")}
        </button>
      )}
    </div>
  );
}

function firstLine(text: string): string {
  return text.split("\n").find((line) => line.trim() !== "")?.trim() ?? "";
}
