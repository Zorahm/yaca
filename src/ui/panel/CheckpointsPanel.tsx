/** Checkpoint cards keep inspection separate from the explicit rollback action. */
import { CaretDown, CaretRight, ChatCircle, ClockCounterClockwise, Terminal } from "@phosphor-icons/react";
import { useMemo } from "react";

import type { CheckpointChange, CheckpointSummary } from "../../core/client";
import type { Feed } from "../../core/feed";
import { capitalize, locale, t, tp } from "../../i18n";
import { checkpointMessages, reasonText, willDo } from "../checkpoint";
import { dateTimeOf } from "../format";
import { Panel } from "./Panel";

export function CheckpointsPanel({ feed, onPreview, onRestore, onCancel }: {
  feed: Feed;
  onPreview: (id: string) => void;
  onRestore: (id: string) => void;
  onCancel: () => void;
}) {
  const points = [...feed.checkpoints].reverse();
  const messages = useMemo(() => checkpointMessages(feed), [feed.blocks]);
  const preview = feed.preview;

  return (
    <Panel>
      {points.length === 0 ? (
        <div className="panel-empty">{t("ckpt.empty")}</div>
      ) : (
        <div className="ckpt-list">
          <div className="ckpt-order">{t("ckpt.order")}</div>
          {points.map((point, index) => (
            <CheckpointCard
              key={point.id}
              point={point}
              message={messages.get(point.id)}
              latest={index === 0}
              files={preview?.id === point.id ? preview.files : null}
              onPreview={() => onPreview(point.id)}
              onRestore={() => onRestore(point.id)}
              onCancel={onCancel}
            />
          ))}
        </div>
      )}
    </Panel>
  );
}

function CheckpointCard({ point, message, latest, files, onPreview, onRestore, onCancel }: {
  point: CheckpointSummary;
  message?: string;
  latest: boolean;
  files: CheckpointChange[] | null;
  onPreview: () => void;
  onRestore: () => void;
  onCancel: () => void;
}) {
  const open = files !== null;
  const command = point.reason.type === "command" ? point.reason.command : null;
  const Icon = point.reason.type === "command" ? Terminal : point.reason.type === "message" ? ChatCircle : ClockCounterClockwise;
  const date = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }).format(point.at);
  const outputId = `checkpoint-preview-${point.id}`;
  const added = files?.reduce((sum, file) => sum + file.added, 0) ?? 0;
  const removed = files?.reduce((sum, file) => sum + file.removed, 0) ?? 0;

  return (
    <article className={open ? "ckpt ckpt-open" : "ckpt"}>
      <div className="ckpt-info">
        <div className="ckpt-heading">
          <Icon size={16} aria-hidden="true" />
          <div className="ckpt-title">{command !== null ? t("ckpt.before_command") : capitalize(reasonText(point.reason))}</div>
          {latest && <span className="ckpt-latest">{t("ckpt.latest")}</span>}
        </div>
        <time className="ckpt-time" dateTime={new Date(point.at).toISOString()} title={dateTimeOf(point.at)}>{date}</time>
        {command !== null && <code className="ckpt-command">{command}</code>}
        {message?.trim() && <div className="ckpt-message" title={message}>{message}</div>}
        <div className="ckpt-meta">{t("ckpt.snapshot", { files: tp("row.files", point.tracked) })}</div>
      </div>
      <button className="ckpt-toggle" aria-expanded={open} aria-controls={outputId} onClick={open ? onCancel : onPreview}>
        {open ? <CaretDown size={12} /> : <CaretRight size={12} />}
        {t(open ? "ckpt.hide_changes" : "ckpt.view_changes")}
      </button>
      {files !== null && (
        <div className="ckpt-preview" id={outputId}>
          {files.length === 0 ? (
            <div className="ckpt-no-changes">{t("ckpt.confirm_nothing")}</div>
          ) : (
            <>
              <div className="ckpt-preview-head">
                <span>{t("ckpt.affected", { files: tp("row.files", files.length) })}</span>
                <span className="counts"><span className="count-add">+{added}</span>{" "}<span className="count-del">−{removed}</span></span>
              </div>
              <div className="ckpt-files">
                {files.map((file) => (
                  <div className="ckpt-file" key={file.path}>
                    <span className="ckpt-file-path">{file.path}</span>
                    <span className={`ckpt-file-action ckpt-file-${file.kind}`}>{willDo(file)}</span>
                  </div>
                ))}
              </div>
              <div className="ckpt-restore">
                <div className="ckpt-restore-question">{t("ckpt.confirm_title", { time: date })}</div>
                <button className="btn btn-small btn-accent" onClick={onRestore}>{t("ckpt.restore_here")}</button>
              </div>
            </>
          )}
        </div>
      )}
    </article>
  );
}
