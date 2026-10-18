/**
 * Лента: вертикальный поток блоков, отрисованный окном.
 *
 * Виртуализация: `virtua` (VList) рендерит только видимую область и меряет
 * высоты блоков сам, ResizeObserver'ом внутри. Блоки разной высоты — норма,
 * ключи-строки дают библиотеке свои размеры на блок (индексы не используются).
 *
 * Автоскролл вниз, пока человек не прокрутил вверх; после этого появляется
 * кнопка возврата со счётчиком новых блоков. Скролл не отбирается: агент,
 * который утаскивает экран вниз, пока ты читаешь, — худшее, что бывает в такой
 * ленте. Решения «человек внизу», «он уехал?» и «куда встать» — чистая
 * арифметика в `ui/feedScroll`, проверяемая тестами; компонент только меряет
 * и применяет. Отпускание держится не порогом от измеренного низа (он отстаёт
 * на кадр, рывок роста перекрыл бы запас посреди стрима), а движением человека
 * вверх относительно последней нашей команды прокрутки.
 *
 * Дорастающий последний блок при стриминге сдвигает хвост ленты на один кадр
 * измерений: библиотека меряет рост после кадра, лента догоняет следующим
 * дельта-кадром, ошибка ограничена высотой одной пачки и глазом не видна.
 * Скачки по ленте (стрелки шапки, кнопка возврата) идут через окно по индексам:
 * блок вне видимой области не размечен, `getElementById` по нему мёртв.
 */

import { useEffect, useImperativeHandle, useRef, useState, type Ref } from "react";

import { VList, type VListHandle } from "virtua";

import type { ElicitationReply, PermissionAnswer, QuestionReply } from "../core/client";
import type { Feed as FeedModel } from "../core/feed";
import { visible } from "../core/feed";
import { closesReply, fold, isLiveWork, replyText, type FeedItem } from "../core/work";
import { AgentsPlate } from "./AgentsPlate";
import { t } from "../i18n";
import {
  AgentText,
  Checkpoint,
  Compaction,
  ErrorBlock,
  HumanMessage,
  ShownText,
  Simple,
  Todos,
  HookNote,
  RouteNote,
} from "./Blocks";
import { CodeBlock } from "./CodeBlock";
import { Permission } from "./Permission";
import { Elicitation } from "./Elicitation";
import { Question } from "./Question";
import { timeOf } from "./format";
import { tp } from "../i18n";
import { shouldHold, shouldStick, type ScrollState } from "./feedScroll";
import { Work } from "./Work";
import { TurnLive, TurnTime } from "./TurnTime";

/** Точка конца ленты, с расчётом на отстающее измерение (см. удержание низа). */
const END_PAD = 240;

/** Ручной скачок по ключу блока идёт целиком через окно, а не через DOM:
   блок вне видимой области не размечен и `getElementById` вернёт null. */
export type FeedJump = {
  /** Прокрутка ленты к блоку с этим ключом; верх блока встаёт под шапку окна. */
  jumpTo: (key: string) => void;
};

export type FeedActions = {
  answer: (answer: PermissionAnswer) => void;
  /** Вопрос `AskUser`: ответ или отказ от ответа. */
  answerQuestion: (reply: QuestionReply) => void;
  /** A form from an MCP server, answered by its id: several may be open. */
  answerElicitation: (requestId: string, reply: ElicitationReply) => void;
  previewCheckpoint: (id: string) => void;
  restoreCheckpoint: (id: string) => void;
  cancelPreview: () => void;
  /** Клик по строке чекпоинта открывает панель со всеми точками. */
  openCheckpoints: () => void;
  editSummary: (text: string) => void;
  openFile: (path: string) => void;
  /** Opens the agents panel on these subagents, by their `Task` call ids. */
  openAgents: (ids: string[]) => void;
  retry: () => void;
};

export function Feed({
  feed,
  actions,
  ref,
}: {
  feed: FeedModel;
  actions: FeedActions;
  ref?: Ref<FeedJump>;
}) {
  const handle = useRef<VListHandle>(null);
  const [stuck, setStuck] = useState(true);
  const [seen, setSeen] = useState(0);

  // Вызовы и рассуждения между репликами — один элемент ленты (`core/work`).
  const blocks = fold(visible(feed));
  const busy = feed.state?.busy ?? false;
  const count = blocks.length;

  /**
   * Пока человек внизу, лента сама отдаёт скролл; прочие движения — его.
   * `commanded` — куда мы сами положили окно в прошлый раз: прокрутка заметно
   * выше этой точки значит жест человека, а не отставшее измерение.
   */
  const stuckRef = useRef(stuck);
  const commanded = useRef<number | null>(null);

  /**
   * Вниз держит не скролл-контейнер, а реплика окна на каждый кадр ленты:
   * пока человек внизу, хвост стоит на месте, дальше — скролл его, а не наш.
   * Это и возврат кнопкой, и дорастание последнего блока при стриминге.
   *
   * Цель — конец кеша с подушкой `END_PAD`: измерение отстаёт от роста на
   * кадр, и команда «до конца» без запаса упиралась бы во вчерашний низ,
   * теряя прилипание, когда пачка перекрывает запас до старого конца. Подушка
   * больше DRIFT_PX держит честной и решётку «человек уехал?»: зажатый до
   * настоящего низа скролл читается ниже запрошенной цели и жёстко сидит.
   */
  useEffect(() => {
    if (!stuck) return;
    const h = handle.current;
    if (!h) return;
    setSeen(count);
    if (count > 0) {
      const target = h.getItemOffset(count) + END_PAD;
      commanded.current = target;
      h.scrollTo(target);
    }
  }, [blocks, stuck]);

  function onScroll(offset: number) {
    const h = handle.current;
    // Без ручки (кадр до первого измерения) хвост читается нулевым, и лента
    // считается внизу — безопасное «прилипло»: метрика 0 / 0 даёт нулевой
    // разрыв, отпускание на ровном месте не сработает, а первым же
    // измерением решение будет выправлено.
    const state: ScrollState = {
      offset,
      scrollSize: h ? h.scrollSize : offset,
      viewportSize: h ? h.viewportSize : 0,
    };
    let down = false;
    if (stuckRef.current) {
      // Прилипание не проверяется порогом от низа: измеренное дно отстаёт от
      // настоящего, и рывок роста перекрыл бы запас на середине стрима.
      // Отпускает только движение человека вверх, что и держит shouldHold.
      down = shouldHold(state, commanded.current);
    } else if (shouldStick(state)) {
      down = true;
    }
    if (!down) commanded.current = null;
    stuckRef.current = down;
    setStuck(down);
    if (down) setSeen(count);
  }

  /**
   * Ручка скачков читает блоки через ref-with-актуальность: factory заморожена
   * раз и навсегда ([] в конце), и замкнутый на первом рендере `blocks`
   * молчал бы по любому блоку, приехавшему позже — весь свежий разговор.
   */
  const blocksRef = useRef(blocks);
  blocksRef.current = blocks;

  useImperativeHandle(
    ref,
    () => ({
      jumpTo: (key: string) => {
        const at = blocksRef.current.findIndex((b) => b.key === key);
        if (at === -1) return;
        // Человек ушёл смотреть в ленту: прилипание отпускаем сразу, не по
        // событию прокрутки — цель могла встать у самого низа.
        stuckRef.current = false;
        commanded.current = null;
        handle.current?.scrollToIndex(at, { align: "start" });
        setStuck(false);
      },
    }),
    [],
  );

  const unread = Math.max(0, count - seen);

  return (
    <div className="feed-wrap">
      <VList
        className="feed"
        ref={handle}
        onScroll={onScroll}
        role="log"
        aria-live="polite"
      >
        {blocks.map((block, index) => (
          <div
            className={startsAgent(blocks, index) ? "block feed-item feed-turn" : "block feed-item"}
            data-kind={block.kind}
            id={block.key}
            key={block.key}
          >
            {startsAgent(blocks, index) && (
              <div className="label label-agent">{t("feed.agent")}</div>
            )}
            <Block
              block={block}
              feed={feed}
              actions={actions}
              live={isLiveWork(blocks, index, busy)}
              copyable={block.kind === "agent" && closesReply(blocks, index, busy)}
              replyText={block.kind === "agent" ? replyText(blocks, index) : ""}
            />
          </div>
        ))}
        {/* A running turn counts up under everything it has said so far. */}
        {busy && feed.turn && (
          <div className="block feed-item" key="turn-live">
            <TurnLive startedAt={feed.turn.startedAt} />
          </div>
        )}
        {/* Воздух между последним блоком и композером. Отдельным элементом
            ленты, а не отступом последнего блока: тот перестаёт быть
            последним с каждым новым, и его высота менялась бы под
            прилипшей к низу прокруткой. */}
        <div className="feed-end" key="feed-end" aria-hidden="true" />
      </VList>

      {!stuck && unread > 0 && (
        <button className="btn btn-small feed-jump" onClick={() => setStuck(true)}>
          {t("feed.jump")} · {unread}
        </button>
      )}
    </div>
  );
}

/** Блоки, которые говорит агент, а не лента и не человек. */
const AGENT_SIDE = new Set(["agent", "work", "agents", "todos", "plan", "shown"]);

/**
 * Лейбл `Yaca` ставится один раз на реплику, а не над каждым блоком: агент,
 * который представляется перед каждой строчкой отчёта, читается как заика.
 *
 * Первый блок реплики — не обязательно текст: чаще всего это работа хода,
 * и лейбл принадлежит ей.
 */
function startsAgent(blocks: FeedItem[], index: number): boolean {
  if (!AGENT_SIDE.has(blocks[index].kind)) return false;
  const before = blocks[index - 1];
  return before === undefined || !AGENT_SIDE.has(before.kind);
}

function Block({
  block,
  feed,
  actions,
  live,
  copyable,
  replyText,
}: {
  block: FeedItem;
  feed: FeedModel;
  actions: FeedActions;
  /** Только для работы хода: ход ещё дописывает её. */
  live: boolean;
  /** Только для текста агента: им заканчивается ответ. */
  copyable: boolean;
  replyText: string;
}) {
  switch (block.kind) {
    case "human":
      return <HumanMessage text={block.text} attachments={block.attachments} />;

    case "agent":
      return <AgentText text={block.text} copyable={copyable} replyText={replyText} />;

    case "work":
      return (
        <Work
          steps={block.steps}
          live={live}
          preview={feed.preview}
          actions={actions}
        />
      );

    case "agents":
      return <AgentsPlate rows={block.rows} onOpen={actions.openAgents} />;

    case "todos":
      return <Todos items={block.items} />;

    case "permission":
      return <Permission request={block.request} onAnswer={actions.answer} />;

    case "question":
      return (
        <Question
          request={block.request}
          status={block.status}
          reply={block.reply}
          onAnswer={actions.answerQuestion}
        />
      );

    case "elicitation":
      return (
        <Elicitation
          request={block.request}
          status={block.status}
          onAnswer={(reply) => actions.answerElicitation(block.request.id, reply)}
        />
      );

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

    case "shown":
      return <ShownText block={block} onOpen={actions.openFile} />;

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

    case "route":
      return <RouteNote block={block} />;

    case "hook":
      return <HookNote block={block} />;

    case "turnTime":
      return <TurnTime startedAt={block.startedAt} endedAt={block.endedAt} />;
  }
}
