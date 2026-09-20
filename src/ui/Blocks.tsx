/**
 * Мелкие блоки ленты: §5.1, §5.2, §5.3, §5.6, §5.8, §5.10, §5.11.
 *
 * Каждый из них — несколько строк вёрстки поверх того, что уже посчитал
 * редьюсер, поэтому они лежат вместе. Группа действий и запрос разрешения
 * живут отдельно: там своё состояние.
 */

import { Brain, CaretDown, CaretRight, Check, Circle } from "@phosphor-icons/react";
import { useState } from "react";

import type { FeedBlock } from "../core/feed";
import type { CheckpointChange } from "../core/client";
import { t, type Key } from "../i18n";
import { reasonText, willDo } from "./checkpoint";
import { timeOf, tokens } from "./format";
import { Markdown } from "./Markdown";

export function HumanMessage({ text, attachments }: { text: string; attachments: string[] }) {
  return (
    <>
      <div className="label label-you">{t("feed.you")}</div>
      <div className="human">
        {attachments.length > 0 && (
          <div className="attachments">
            {attachments.map((path) => (
              <span className="attachment" key={path}>
                {path}
              </span>
            ))}
          </div>
        )}
        {text}
      </div>
    </>
  );
}

export function AgentText({ text }: { text: string }) {
  return <Markdown text={text} />;
}

/**
 * §5.3. Свёрнуто в строку; во время стриминга — «рассуждает…» с пульсирующей
 * точкой. Длительности нет: времени начала в потоке событий не приходит, а
 * замерить его часами окна значило бы показать в открытой заново сессии время
 * не того прогона.
 */
export function Reasoning({ text, live }: { text: string; live: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button className="reason-head" onClick={() => setOpen(!open)}>
        <Brain size={13} className={live ? "pulse" : undefined} />
        {live ? (
          <>
            {t("reason.live")}{" "}
            <span className="row-dot pulse">
              <Circle size={7} weight="fill" />
            </span>
          </>
        ) : (
          t("reason.title")
        )}
      </button>
      {open && <div className="reason-body">{text}</div>}
    </div>
  );
}

/** §5.6: обновляется на месте, новый блок не создаётся. */
export function Todos({ items }: { items: { content: string; status: string }[] }) {
  return (
    <div className="todos">
      {items.map((item, index) => {
        const cls =
          item.status === "completed"
            ? "todo todo-done"
            : item.status === "inProgress"
              ? "todo todo-now"
              : "todo todo-next";
        const mark =
          item.status === "completed" ? (
            <Check size={12} weight="bold" />
          ) : item.status === "inProgress" ? (
            <Circle size={11} weight="fill" />
          ) : (
            <Circle size={11} />
          );
        return (
          <div className={cls} key={index}>
            <span className={item.status === "inProgress" ? "todo-mark pulse" : "todo-mark"}>
              {mark}
            </span>
            <span className="todo-text">{item.content}</span>
          </div>
        );
      })}
    </div>
  );
}

/** §5.8: тонкая строка-разделитель с предложением откатиться. */
export function Checkpoint({
  block,
  preview,
  onPreview,
  onRestore,
  onCancel,
  onOpenPanel,
}: {
  block: Extract<FeedBlock, { kind: "checkpoint" }>;
  preview: { id: string; files: CheckpointChange[] } | null;
  onPreview: (id: string) => void;
  onRestore: (id: string) => void;
  onCancel: () => void;
  /** §1.1: сама строка ведёт в панель со всеми точками, кнопка справа —
      в откат к этой. Две цели на одной строке, как их и рисует §5.8. */
  onOpenPanel: () => void;
}) {
  const { checkpoint } = block;
  const asking = preview?.id === checkpoint.id;

  return (
    <div>
      <div className="divider">
        <button
          className="divider-open"
          onClick={onOpenPanel}
          title={t("ckpt.open_panel")}
        >
          <span className="divider-dot"><Circle size={6} weight="fill" /></span>
          <span>{timeOf(checkpoint.at)}</span>
          <span>·</span>
          <span>{reasonText(checkpoint.reason)}</span>
        </button>
        <span className="divider-line" />
        <button
          className="divider-action"
          onClick={() => onPreview(checkpoint.id)}
        >
          {t("ckpt.restore_here")}
        </button>
      </div>

      {/* §5.8: откат показывает список затрагиваемых файлов и требует
          подтверждения — до того, как что-либо произошло. */}
      {asking && (
        <div className="confirm">
          <div className="confirm-title">
            {t("ckpt.confirm_title", { time: timeOf(checkpoint.at) })}
          </div>
          {preview.files.length === 0 ? (
            <div className="confirm-list">{t("ckpt.confirm_nothing")}</div>
          ) : (
            <div className="confirm-list">
              {preview.files.map((file) => (
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
            <button
              className="btn btn-accent"
              onClick={() => onRestore(checkpoint.id)}
            >
              {t("ckpt.confirm_yes")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/** §5.11: разделитель, под ним сводка, которую можно поправить. */
export function Compaction({
  summary,
  freed,
  onEdit,
}: {
  summary: string;
  freed: number;
  onEdit: (text: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<string | null>(null);

  return (
    <div>
      <button className="divider" onClick={() => setOpen(!open)}>
        <span>{t("compact.divider", { tokens: tokens(freed) })}</span>
        <span className="caret">{open ? <CaretDown size={12} /> : <CaretRight size={12} />}</span>
      </button>

      {open && (
        <div className="summary-box">
          {draft === null ? (
            <>
              {summary}
              <div className="error-actions">
                <button className="btn btn-small" onClick={() => setDraft(summary)}>
                  {t("compact.edit")}
                </button>
              </div>
            </>
          ) : (
            <>
              <textarea value={draft} onChange={(e) => setDraft(e.target.value)} />
              <div className="error-actions">
                <button className="btn btn-small" onClick={() => setDraft(null)}>
                  {t("compact.cancel")}
                </button>
                <button
                  className="btn btn-small btn-accent"
                  onClick={() => {
                    onEdit(draft);
                    setDraft(null);
                  }}
                >
                  {t("compact.save")}
                </button>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}

/** §5.10: заголовок, техническая деталь моноширинным, действия. */
export function ErrorBlock({
  code,
  message,
  retryable,
  onRetry,
}: {
  code: string;
  message: string;
  retryable: boolean;
  onRetry: () => void;
}) {
  return (
    <div className="error">
      <div className="error-title">{code}</div>
      <div className="error-detail">{message}</div>
      {retryable && (
        <div className="error-actions">
          <button className="btn btn-small" onClick={onRetry}>
            {t("err.retry")}
          </button>
        </div>
      )}
    </div>
  );
}

export function Simple({ textKey }: { textKey: Key }) {
  return <div className="divider">{t(textKey)}</div>;
}
