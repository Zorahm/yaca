/**
 * Лента §4: вертикальный поток блоков §5.
 *
 * Автоскролл вниз, пока человек не прокрутил вверх; после этого появляется
 * кнопка возврата со счётчиком новых блоков. Скролл не отбирается: агент,
 * который утаскивает экран вниз, пока ты читаешь, — худшее, что бывает в такой
 * ленте.
 *
 * Виртуализации здесь нет. §4 требует её примерно от сотни сообщений, и она
 * приедет вместе с панелями: окно списка ломает и якорь автоскролла, и
 * раскрытые строки, и это отдельная работа, а не строчка в этом файле.
 */

import { useEffect, useRef, useState } from "react";

import type { PermissionAnswer } from "../core/client";
import type { Feed as FeedModel, FeedBlock } from "../core/feed";
import { visible } from "../core/feed";
import { t } from "../i18n";
import { ActionGroup } from "./ActionGroup";
import {
  AgentText,
  Checkpoint,
  Compaction,
  ErrorBlock,
  HumanMessage,
  Reasoning,
  Simple,
  Todos,
} from "./Blocks";
import { CodeBlock } from "./CodeBlock";
import { Permission } from "./Permission";
import { timeOf } from "./format";
import { tp } from "../i18n";

/** Насколько близко к низу считается «человек внизу». */
const STICK_PX = 80;

export type FeedActions = {
  answer: (answer: PermissionAnswer) => void;
  previewCheckpoint: (id: string) => void;
  restoreCheckpoint: (id: string) => void;
  cancelPreview: () => void;
  /** §1.1: клик по строке чекпоинта открывает панель со всеми точками. */
  openCheckpoints: () => void;
  editSummary: (text: string) => void;
  openFile: (path: string) => void;
  retry: () => void;
};

export function Feed({ feed, actions }: { feed: FeedModel; actions: FeedActions }) {
  const scroller = useRef<HTMLDivElement>(null);
  const [stuck, setStuck] = useState(true);
  const [seen, setSeen] = useState(0);

  const blocks = visible(feed);
  const busy = feed.state?.busy ?? false;

  useEffect(() => {
    const node = scroller.current;
    if (!node) return;
    if (stuck) {
      node.scrollTop = node.scrollHeight;
      setSeen(blocks.length);
    }
  }, [blocks, stuck]);

  function onScroll() {
    const node = scroller.current;
    if (!node) return;
    const bottom = node.scrollHeight - node.scrollTop - node.clientHeight;
    const atBottom = bottom <= STICK_PX;
    setStuck(atBottom);
    if (atBottom) setSeen(blocks.length);
  }

  const unread = Math.max(0, blocks.length - seen);

  return (
    <div className="feed" ref={scroller} onScroll={onScroll} role="log" aria-live="polite">
      <div className="feed-inner">
        {blocks.map((block, index) => (
          <div className="block" id={block.key} key={block.key}>
            {startsAgent(blocks, index) && (
              <div className="label label-agent">{t("feed.agent")}</div>
            )}
            <Block
              block={block}
              feed={feed}
              actions={actions}
              last={index === blocks.length - 1}
              busy={busy}
            />
          </div>
        ))}
      </div>

      {!stuck && unread > 0 && (
        <button
          className="btn btn-small feed-jump"
          onClick={() => {
            setStuck(true);
            const node = scroller.current;
            if (node) node.scrollTop = node.scrollHeight;
          }}
        >
          {t("feed.jump")} · {unread}
        </button>
      )}
    </div>
  );
}

/** Блоки, которые говорит агент, а не лента и не человек. */
const AGENT_SIDE = new Set(["agent", "reasoning", "actions", "todos", "plan"]);

/**
 * Лейбл `Yaca` ставится один раз на реплику, а не над каждым блоком: агент,
 * который представляется перед каждой строчкой отчёта, читается как заика.
 *
 * Первый блок реплики — не обязательно текст: чаще всего это рассуждение
 * (§5.3), и лейбл принадлежит ему.
 */
function startsAgent(blocks: FeedBlock[], index: number): boolean {
  if (!AGENT_SIDE.has(blocks[index].kind)) return false;
  const before = blocks[index - 1];
  return before === undefined || !AGENT_SIDE.has(before.kind);
}

function Block({
  block,
  feed,
  actions,
  last,
  busy,
}: {
  block: FeedBlock;
  feed: FeedModel;
  actions: FeedActions;
  last: boolean;
  busy: boolean;
}) {
  switch (block.kind) {
    case "human":
      return <HumanMessage text={block.text} attachments={block.attachments} />;

    case "agent":
      return <AgentText text={block.text} />;

    case "reasoning":
      return <Reasoning text={block.text} live={last && busy} />;

    case "actions":
      return (
        <ActionGroup
          rows={block.rows}
          onOpenFile={actions.openFile}
        />
      );

    case "todos":
      return <Todos items={block.items} />;

    case "permission":
      return <Permission request={block.request} onAnswer={actions.answer} />;

    case "checkpoint":
      return (
        <Checkpoint
          block={block}
          preview={feed.preview}
          onPreview={actions.previewCheckpoint}
          onRestore={actions.restoreCheckpoint}
          onCancel={actions.cancelPreview}
          onOpenPanel={actions.openCheckpoints}
        />
      );

    case "restored":
      return (
        <div className="divider">
          {tp("ckpt.restored", block.files, { time: timeOf(block.at) })}
        </div>
      );

    case "compacted":
      return (
        <Compaction
          summary={block.summary}
          freed={block.freed}
          onEdit={actions.editSummary}
        />
      );

    case "plan":
      return (
        <div className="plan">
          <CodeBlock text={block.plan} />
        </div>
      );

    case "error":
      return (
        <ErrorBlock
          code={block.code}
          message={block.message}
          retryable={block.retryable}
          onRetry={actions.retry}
        />
      );

    case "retry":
      return (
        <div className="divider">
          {t("err.retry_in", {
            seconds: Math.round(block.delayMs / 1000),
            attempt: block.attempt,
            of: block.of,
          })}
        </div>
      );

    case "stopped":
      return <Simple textKey="err.stopped" />;
  }
}
