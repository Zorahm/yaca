/**
 * A stretch of a turn's work: one summary line, and under it the calls and
 * thoughts it is made of, in the order they happened.
 *
 * The summary counts what changed, as the action group always did. Opened, it
 * is a card with one line per step:
 *
 * - a call is the action row it has always been;
 * - a thought is a row too: its first line stands as the caption, dimmed, and
 *   the row opens into the whole text. A thought was a bare word "reasoning"
 *   before, which said that something was thought and nothing of what.
 *
 * What the agent says is not in here: text divides the work (`core/work`).
 *
 * A file group with unapplied changes starts open. Explicit toggles take precedence.
 */

import { Brain, CaretDown, CaretRight, Check } from "@phosphor-icons/react";
import { useState } from "react";

import type { WorkStep } from "../core/work";
import { flatRows } from "../core/feed";
import { workRows } from "../core/work";
import { t } from "../i18n";
import { ActionRowView } from "./ActionRow";
import { actionNotices } from "./editNotices";
import { FileActivity, FileActivitySlot } from "./FileActivityGroup";
import { actionFailed, activityPath, groupFileSteps, pathKey, uniqueFileNames } from "./fileActivity";
import { FileLspBadge, LspBadge } from "./LspBadge";
import { Checkpoint } from "./Blocks";
import { summary } from "./captions";
import type { FeedActions } from "./Feed";
import { Spinner } from "./RowSign";
import type { CheckpointChange } from "../core/client";

export function Work({
  steps,
  live,
  preview,
  actions,
}: {
  steps: WorkStep[];
  /** The running turn is still adding to this item. */
  live: boolean;
  preview: { id: string; files: CheckpointChange[] } | null;
  actions: FeedActions;
}) {
  const [expanded, setExpanded] = useState<boolean>();

  const rows = workRows(steps);
  const fileGroups = groupFileSteps(steps);
  const open = expanded ?? [...fileGroups.values()].some((group) => group?.steps.some(({ row }) => actionFailed(row)));
  const names = open ? uniqueFileNames(rows.flatMap((row) => {
    const path = activityPath(row);
    return path ? [path] : [];
  })) : new Map<string, string>();
  const head = summary(rows);
  const failed = head.failed > 0;
  const files = flatRows(rows).map(actionNotices);
  const notices = files.flat().filter((notice) => notice.kind !== "lsp-clean");
  const lspIssues = notices.length > 0;

  return (
    <div className="group">
      <button
        className="group-head"
        aria-expanded={open}
        onClick={() => setExpanded(!open)}
      >
        {live ? (
          <Spinner />
        ) : (
          !failed && !lspIssues && (
            <span className="group-state" aria-hidden="true">
              <Check size={14} weight="bold" />
            </span>
          )
        )}
        <span className="group-title">
          {/* Nothing was called: the item is thought alone, and says so. */}
          {head.parts.length === 0 && t(live ? "work.thinking" : "work.thought")}
          {head.parts.map((part, index) => (
            <span key={index}>
              {index > 0 && ", "}
              {part.text}
              {part.failedNote && (
                <span className="group-failed">{part.failedNote}</span>
              )}
            </span>
          ))}
        </span>
        {fileGroups.size ? <FileLspBadge files={files} /> : <LspBadge notices={notices} />}
        {(head.added > 0 || head.removed > 0) && (
          <span className="counts">
            <span className="count-add">+{head.added}</span>{" "}
            <span className="count-del">−{head.removed}</span>
          </span>
        )}
        <span className="caret">{open ? <CaretDown size={12} /> : <CaretRight size={12} />}</span>
      </button>

      {open && (
        // The card does not scroll: it grows and the feed scrolls. What limits
        // height is the content that can scroll itself — a diff, an output.
        <div className="group-list">
          {steps.map((step, index) => {
            switch (step.kind) {
              case "action": {
                const path = activityPath(step.row);
                if (!path) return <ActionRowView key={step.key} row={step.row} onOpenFile={actions.openFile} />;
                const group = fileGroups.get(step.key);
                const variant = group ? "group" : group === null ? "folded" : "single";
                return <FileActivitySlot key={step.key} variant={variant} live={live}>
                  {group ? <FileActivity group={group} onOpenFile={actions.openFile} /> : group === undefined
                    ? <ActionRowView row={step.row} onOpenFile={actions.openFile} displayName={names.get(pathKey(path))} />
                    : null}
                </FileActivitySlot>;
              }
              case "thought":
                return (
                  <Thought
                    key={step.key}
                    text={step.text}
                    live={live && index === steps.length - 1}
                  />
                );
              case "checkpoint":
                return (
                  <div className="row step-checkpoint" key={step.key}>
                    <Checkpoint
                      block={step.block}
                      preview={preview}
                      onPreview={actions.previewCheckpoint}
                      onRestore={actions.restoreCheckpoint}
                      onCancel={actions.cancelPreview}
                      onOpenPanel={actions.openCheckpoints}
                    />
                  </div>
                );
            }
          })}
        </div>
      )}
    </div>
  );
}

/**
 * A thought as a row of the card, folded to its first line until opened.
 *
 * No duration: the stream carries no start time for a thought, and timing it
 * by the window's clock would show, in a reopened session, the time of a
 * different run.
 */
function Thought({ text, live }: { text: string; live: boolean }) {
  const [open, setOpen] = useState(false);

  return (
    <div className={open ? "row row-open" : "row"}>
      <button
        className="row-head"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <span className={live ? "sign pulse" : "sign"} aria-hidden="true">
          <Brain size={14} />
        </span>
        <span className="row-caption thought-line">
          {open ? t(live ? "work.thinking" : "work.thought") : firstLine(text)}
        </span>
        <span className="caret">{open ? <CaretDown size={12} /> : <CaretRight size={12} />}</span>
      </button>
      {open && <div className="thought-body">{text.trim()}</div>}
    </div>
  );
}

/** The line a folded thought shows: its first words, without Markdown marks. */
function firstLine(text: string): string {
  const line = text.split("\n").find((l) => l.trim() !== "") ?? "";
  return line.replace(/^[#>*\-\s]+/, "").replace(/[*_`]/g, "");
}
