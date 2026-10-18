/**
 * Checkpoints panel: the list of this run's checkpoints and a diff from the selected one.
 *
 * The core sends the whole list in the `checkpointsUpdated` event, so the panel is a function
 * of it and keeps no list of its own. The diff works the same way: selecting a row sends
 * `previewCheckpoint`, the core answers `checkpointPreviewed`, and what is shown is exactly
 * what arrived. Recomputing it in the window would be a second version of the truth, and the
 * two diverge exactly when the tree moved between the question and the answer, which is when
 * the cost of a mistake is highest.
 *
 * Newest first: the panel is a way to go back, and one usually goes back not far. The feed runs
 * the other way round, but there a checkpoint sits in its place in the conversation, while here
 * it is a row of a list.
 *
 * The diff is the same list of files the confirmation in the feed shows: what the rollback
 * will do to each. The core does not give a line-by-line diff from a checkpoint, and there is
 * nothing here to draw one from.
 *
 * Checkpoints of earlier runs do not get here: their list lives in the session's memory and
 * does not outlive it (README). A session reopened starts with an empty panel, and that is
 * true — it cannot go back to yesterday's checkpoint.
 */

import type { CheckpointChange, CheckpointSummary } from "../../core/client";
import type { Feed } from "../../core/feed";
import { t } from "../../i18n";
import { reasonText, willDo } from "../checkpoint";
import { timeOf } from "../format";
import { Panel } from "./Panel";

export function CheckpointsPanel({
  feed,
  onPreview,
  onRestore,
  onCancel,
}: {
  feed: Feed;
  onPreview: (id: string) => void;
  onRestore: (id: string) => void;
  /** Dismiss the question without rolling anything back. */
  onCancel: () => void;
}) {
  const points = [...feed.checkpoints].reverse();
  const preview = feed.preview;

  return (
    <Panel>
      {points.length === 0 ? (
        <div className="panel-empty">{t("ckpt.empty")}</div>
      ) : (
        <div className="ckpt-list">
          {points.map((point) => (
            <CheckpointRow
              key={point.id}
              point={point}
              asking={preview?.id === point.id}
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

function CheckpointRow({
  point,
  asking,
  files,
  onPreview,
  onRestore,
  onCancel,
}: {
  point: CheckpointSummary;
  asking: boolean;
  files: CheckpointChange[] | null;
  onPreview: () => void;
  onRestore: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="ckpt">
      <button className="ckpt-row" aria-expanded={asking} onClick={onPreview}>
        <span className={asking ? "ckpt-dot ckpt-dot-on" : "ckpt-dot"} />
        <span className="ckpt-reason">
          {reasonText(point.reason)} · {timeOf(point.at)}
        </span>
      </button>

      {/* A rollback shows what it will touch and waits for confirmation, before
          anything happens. */}
      {asking && files !== null && (
        <div className="confirm">
          <div className="confirm-title">
            {t("ckpt.confirm_title", { time: timeOf(point.at) })}
          </div>
          {files.length === 0 ? (
            <div className="confirm-list">{t("ckpt.confirm_nothing")}</div>
          ) : (
            <div className="confirm-list">
              {files.map((file) => (
                <div className="confirm-file" key={file.path}>
                  <span>{file.path}</span>
                  <span className="confirm-what">{willDo(file)}</span>
                </div>
              ))}
            </div>
          )}
          <div className="error-actions">
            <button className="btn" onClick={onCancel}>
              {t("ckpt.confirm_no")}
            </button>
            <button className="btn btn-accent" onClick={onRestore}>
              {t("ckpt.confirm_yes")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
