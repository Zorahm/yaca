/**
 * Feed: a vertical stream of blocks, drawn through a window.
 *
 * Virtualised: `virtua` (VList) renders only the visible area and measures the
 * block heights itself, with a ResizeObserver inside. Blocks of different heights
 * are the norm; row keys give the library its own size per block (indexes are not
 * used).
 *
 * Auto-scroll down while the person has not scrolled up; after that a return
 * button appears with a counter of new blocks. The scroll is not taken away: an
 * agent that drags the screen down while you read is the worst thing that can
 * happen in such a feed. The decisions "the person is at the bottom", "have they
 * left?" and "where to stand" are pure arithmetic in `ui/feedScroll`, checked by
 * tests; the component only measures and applies. Letting go is held not by a
 * threshold from the measured bottom (it lags by a frame, and a jerk of growth
 * would cover the margin in the middle of a stream), but by the person's movement
 * up relative to our last scroll command.
 *
 * A growing last block during streaming shifts the tail of the feed by one frame
 * of measurement: the library measures the growth after the frame, the feed
 * catches up on the next delta frame, and the error is limited to the height of
 * one batch and is invisible to the eye. Jumps along the feed (the header arrows,
 * the return button) go through the window by index: a block outside the visible
 * area is not marked up, and `getElementById` on it is dead.
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

/** The feed's end point, allowing for a lagging measurement (see the bottom hold). */
const END_PAD = 240;

/** A manual jump to a block by its key goes entirely through the window, not
   through the DOM: a block outside the visible area is not marked up, and
   `getElementById` returns null. */
export type FeedJump = {
  /** Scroll the feed to the block with this key; the top of the block stands under the window's header. */
  jumpTo: (key: string) => void;
};

export type FeedActions = {
  answer: (answer: PermissionAnswer) => void;
  /** The `AskUser` question: an answer or a refusal to answer. */
  answerQuestion: (reply: QuestionReply) => void;
  /** A form from an MCP server, answered by its id: several may be open. */
  answerElicitation: (requestId: string, reply: ElicitationReply) => void;
  previewCheckpoint: (id: string) => void;
  restoreCheckpoint: (id: string) => void;
  cancelPreview: () => void;
  /** A click on a checkpoint line opens the panel with all the points. */
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

  // Calls and reasoning between replies are one feed item (`core/work`).
  const blocks = fold(visible(feed));
  const busy = feed.state?.busy ?? false;
  const count = blocks.length;

  /**
   * While the person is at the bottom, the feed gives the scroll itself; other
   * movements are theirs. `commanded` is where we ourselves put the window last
   * time: a scroll noticeably above this point is a gesture of the person, not a
   * lagging measurement.
   */
  const stuckRef = useRef(stuck);
  const commanded = useRef<number | null>(null);

  /**
   * The bottom is held not by the scroll container, but by a replica of the window
   * on each frame of the feed: while the person is at the bottom, the tail stays in
   * place; further on, the scroll is theirs, not ours. This covers both the return
   * by button and the growth of the last block during streaming.
   *
   * The target is the end of the cache with the padding `END_PAD`: the measurement
   * lags behind growth by a frame, and a "to the end" command without a margin
   * would aim at yesterday's bottom, losing the stickiness when a batch covers the
   * margin up to the old end. A padding larger than DRIFT_PX also keeps the
   * "has the person moved away?" check honest: a scroll pinned to the real bottom
   * reads below the requested target and sits stuck there.
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
    // Without a handle (the frame before the first measurement) the tail reads as
    // zero, and the feed counts as at the bottom — a safe "stuck": with a metric of
    // 0 / 0 the gap is zero, letting go on a flat spot will not fire, and the first
    // measurement corrects the decision.
    const state: ScrollState = {
      offset,
      scrollSize: h ? h.scrollSize : offset,
      viewportSize: h ? h.viewportSize : 0,
    };
    let down = false;
    if (stuckRef.current) {
      // Sticking is not checked against a threshold from the bottom: the measured
      // bottom lags behind the real one, and a jerk of growth would cover the margin
      // in the middle of the stream. Only the person's movement up releases it, which
      // is what shouldHold keeps.
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
   * The jump handle reads blocks through a ref that is kept current: the factory is
   * frozen once and for all ([] at the end), and a `blocks` closed over on the first
   * render would stay silent about any block that arrived later — the whole fresh
   * conversation.
   */
  const blocksRef = useRef(blocks);
  blocksRef.current = blocks;

  useImperativeHandle(
    ref,
    () => ({
      jumpTo: (key: string) => {
        const at = blocksRef.current.findIndex((b) => b.key === key);
        if (at === -1) return;
        // The person has gone to look at the feed: letting go of the stickiness at
        // once, not by a scroll event — the target could have landed at the very bottom.
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
        {/* Air between the last block and the composer. A feed item of its own,
            not a margin of the last block: that stops being the last with each
            new one, and its height would change under the scroll stuck to the
            bottom. */}
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

/** Blocks that the agent says, not the feed and not the person. */
const AGENT_SIDE = new Set(["agent", "work", "agents", "todos", "plan", "shown"]);

/**
 * The `Yaca` label is put on a reply once, not above each block: an agent that
 * introduces itself before every line of a report reads as a stutterer.
 *
 * The first block of a reply is not necessarily text: most often it is the turn's
 * work, and the label belongs to it.
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
  /** Only for the turn's work: the turn is still writing it. */
  live: boolean;
  /** Only for the agent's text: it is what ends a reply. */
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
