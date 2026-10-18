/**
 * The feed as a pure function of the event stream.
 *
 * The core promises that the interface is a function of `AgentEvent`, and here it is. Everything
 * drawn in the feed and the panels is derived from here: keeping state in components would mean
 * having a second version of the truth, and it diverges from the first at the first mismatch
 * with a file.
 *
 * Two properties keep the rest in place.
 *
 * *The list of blocks only grows.* A permission block is not removed after its answer; it is
 * marked closed and not drawn: this way block indices do not shift, and the feed
 * behaves like a journal from which it is itself rebuilt.
 *
 * *A frame is one recomputation.* The bridge sends events in batches per frame, so
 * `reduceAll` creates one new snapshot per batch, not per delta.
 */

import type { AgentEvent } from "./bindings/AgentEvent";
import type { CheckpointChange } from "./bindings/CheckpointChange";
import type { CheckpointSummary } from "./bindings/CheckpointSummary";
import type { FileDiff } from "./bindings/FileDiff";
import type { FileFact } from "./bindings/FileFact";
import type { FileView } from "./bindings/FileView";
import type { LspServerSummary } from "./bindings/LspServerSummary";
import type { McpServerSummary } from "./bindings/McpServerSummary";
import type { PermissionRequest } from "./bindings/PermissionRequest";
import type { ProcessId } from "./bindings/ProcessId";
import type { QueuedMessage } from "./bindings/QueuedMessage";
import type { QuestionReply } from "./bindings/QuestionReply";
import type { QuestionRequest } from "./bindings/QuestionRequest";
import type { ElicitationReply } from "./bindings/ElicitationReply";
import type { ElicitationRequest } from "./bindings/ElicitationRequest";
import type { ProcessSummary } from "./bindings/ProcessSummary";
import type { SessionState } from "./bindings/SessionState";
import type { TodoItem } from "./bindings/TodoItem";
import type { TokenCounts } from "./bindings/TokenCounts";
import type { ToolMeta } from "./bindings/ToolMeta";
import type { UsageSnapshot } from "./bindings/UsageSnapshot";

/** How an action ended. `running` is the `●` dot and the present tense. */
export type ActionState = "running" | "done" | "failed" | "rejected" | "denied" | "stopped";

/** One row in an action group. */
/**
 * Tools whose one call changes many files (`Rename`). Each file arrives as a
 * `fileChanged` of its own under the call, and becomes a row nested under the
 * call's row instead of taking its place: the row is the rename, the rows
 * under it are the files.
 */
export const MULTI_FILE_TOOLS: ReadonlySet<string> = new Set(["Rename"]);

export type ActionRow = {
  callId: string;
  /** Tool name. Not shown in the feed; needed to pick the layout. */
  tool: string;
  /** The model's caption, or `null` for tools the feed captions itself. */
  description: string | null;
  /** First line of the expanded row: the full path or the full command. */
  target: string | null;
  input: unknown;
  state: ActionState;
  /** The reason for a failure, a broken invariant or a refusal by the human. */
  message: string | null;
  meta: ToolMeta | null;
  output: string;
  /** Set on file actions: the caption and the counters come from here. */
  fact: FileFact | null;
  diff: FileDiff | null;
  /**
   * Subagent calls, on the `Task` row that sent them. Rows as rows: a subagent's
   * edit is an edit to the human's files, and it gets the same diff. Nesting is
   * one level deep: a subagent does not start a subagent.
   */
  children: ActionRow[];
  /**
   * What a subagent has spent so far, on its `Task` row: the newest
   * `subagentUsage`, which is a running total. `null` on every other row,
   * and on a `Task` row no figure has arrived for.
   */
  tokens: TokenCounts | null;
};

/**
 * Where an `AskUser` question stands (a block that looks like a permission request).
 *
 * `open` waits for the human; `answered` and `dismissed` are what they did; `unanswered`
 * means the turn was stopped before an answer. The block stays in the feed in all four:
 * a question that was answered is a line of the conversation, not a request to be dismissed.
 */
export type QuestionStatus = "open" | "answered" | "dismissed" | "unanswered";

/**
 * Where an MCP server's form stands: waiting, or what became of it.
 * `expired` — the server stopped waiting before anyone answered.
 */
export type ElicitationStatus = "open" | "accepted" | "declined" | "cancelled" | "expired";

export type FeedBlock =
  | { kind: "human"; key: string; text: string; attachments: string[] }
  | { kind: "agent"; key: string; blockId: string; text: string }
  | { kind: "reasoning"; key: string; blockId: string; text: string }
  | { kind: "actions"; key: string; rows: ActionRow[] }
  | { kind: "todos"; key: string; items: TodoItem[] }
  | { kind: "permission"; key: string; request: PermissionRequest; open: boolean }
  | {
      kind: "question";
      key: string;
      request: QuestionRequest;
      status: QuestionStatus;
      /** What was picked, when there is an answer; `null` while open or unanswered. */
      reply: QuestionReply | null;
    }
  | {
      kind: "elicitation";
      key: string;
      request: ElicitationRequest;
      status: ElicitationStatus;
    }
  | { kind: "checkpoint"; key: string; checkpoint: CheckpointSummary }
  | { kind: "restored"; key: string; at: number; files: number }
  | { kind: "compacted"; key: string; summary: string; freed: number }
  | { kind: "plan"; key: string; plan: string }
  /**
   * A file's text the agent put in front of the human (`SendText`). The text
   * is the one the event carried: the file may have changed since.
   */
  | {
      kind: "shown";
      key: string;
      path: string;
      /** The command line, when this is a background command's output. */
      command: string | null;
      /** 1-indexed. */
      firstLine: number;
      lines: number;
      /** Lines in the file: `lines === total` is the whole of it. */
      total: number;
      text: string;
    }
  | { kind: "error"; key: string; code: string; message: string; retryable: boolean }
  | { kind: "retry"; key: string; attempt: number; of: number; delayMs: number }
  | { kind: "route"; key: string; route: string | null; provider: string; model: string }
  | { kind: "stopped"; key: string }
  /** What a plugin hook said where no call row stands (`hookReported`). */
  | { kind: "hook"; key: string; plugin: string; event: string; text: string; blocked: boolean }
  /**
   * How long a turn took and when it ended, under what it said. Made of
   * `TurnStarted.at` and the end event's `at`: the core's clock, not the
   * window's, so a reopened session shows the same figures it did live.
   */
  | { kind: "turnTime"; key: string; startedAt: number; endedAt: number };

export type Feed = {
  blocks: FeedBlock[];
  /** The composer's bottom row: modes, model, busy state. */
  state: SessionState | null;
  /** The context ring. */
  usage: UsageSnapshot | null;
  /** The processes panel. */
  processes: ProcessSummary[];
  /**
   * This chat's MCP servers, whole, as the core last sent them in
   * `mcpUpdated`. `null` means the list has not arrived yet.
   */
  mcp: McpServerSummary[] | null;
  /**
   * Language servers of this chat, whole, as the core last sent them in
   * `lspUpdated`. `null` until the list has arrived.
   */
  lsp: LspServerSummary[] | null;
  /**
   * Output of the selected process. It arrives only for the row being looked at:
   * `processOutputReset` puts the whole buffer, live chunks are appended.
   * One slot, not a map: a human can only look at one.
   */
  processOutput: { id: ProcessId; text: string; dropped: number } | null;
  /**
   * The latest diff for each path: the file panel's index.
   *
   * Stagings arrive without an action row too: when applying conflicts, the
   * `callId` is empty, yet the diff has to be shown. The last one per path wins:
   * the file panel shows the file as it now stands in the set, not its first edit.
   */
  diffs: Record<string, FileDiff>;
  /**
   * How many file writes arrived in the session. The file panel rereads its view
   * when the number grows: a new write changes what it shows.
   */
  writes: number;
  /**
   * The full diff of one file for the panel, the last one to arrive. One slot:
   * there is one file panel. `view: null` means nothing in the file awaits a decision.
   */
  fileView: { path: string; view: FileView | null } | null;
  /** The checkpoints panel. */
  checkpoints: CheckpointSummary[];
  /** The active permission request, if any. */
  request: PermissionRequest | null;
  /** The open `AskUser` question, if any: the bottom row and the hotkeys wait for it. */
  question: QuestionRequest | null;
  /**
   * What a rollback would touch, if confirmed.
   *
   * It lives in the feed, not in a button: the list arrives as an event, and the
   * checkpoint row that asked for it is the one that must show it.
   */
  preview: { id: string; files: CheckpointChange[] } | null;
  /**
   * Messages waiting for their turn in the run, in order. The list arrives whole
   * from the core: cancelling and merging change it there, and here only the
   * latest arrival is shown.
   */
  queued: QueuedMessage[];
  /**
   * The latest answer to an `@` search. It carries the query it answers:
   * the composer shows it only while the query is still the one typed.
   */
  found: { query: string; paths: string[] } | null;
  /**
   * The turn that is running, from the core's `turnStarted`. The live timer
   * counts from it; `null` between turns.
   */
  turn: { startedAt: number } | null;
};

export const emptyFeed: Feed = {
  blocks: [],
  state: null,
  usage: null,
  processes: [],
  mcp: null,
  lsp: null,
  processOutput: null,
  diffs: {},
  writes: 0,
  fileView: null,
  checkpoints: [],
  request: null,
  question: null,
  preview: null,
  queued: [],
  found: null,
  turn: null,
};

/** A batch of events — one snapshot of the feed. */
export function reduceAll(feed: Feed, events: AgentEvent[]): Feed {
  if (events.length === 0) return feed;
  const draft: Feed = { ...feed, blocks: feed.blocks.slice() };
  for (const event of events) apply(draft, event);
  return draft;
}

let sequence = 0;
/** A block's key. Its own, not an index: blocks outlive a redraw. */
function key(prefix: string): string {
  sequence += 1;
  return `${prefix}_${sequence}`;
}

function apply(feed: Feed, event: AgentEvent): void {
  switch (event.type) {
    case "userMessage":
      feed.blocks.push({
        kind: "human",
        key: key("human"),
        text: event.text,
        attachments: event.attachments,
      });
      return;

    case "textDelta":
      appendText(feed, event.blockId, event.text);
      return;

    case "thinkingDelta":
      appendThinking(feed, event.blockId, event.text);
      return;

    case "toolCallStarted": {
      const row: ActionRow = {
        callId: event.callId,
        tool: event.tool,
        description: event.description,
        target: event.target,
        input: event.input,
        state: "running",
        message: null,
        meta: null,
        output: "",
        fact: null,
        diff: null,
        children: [],
        tokens: null,
      };
      // A subagent call goes under its own `Task` row, wherever that row is:
      // a permission request may have come between them, and it closes the group.
      // A parent that is not in the feed is no reason to lose the action.
      const parent = event.parent;
      const nested =
        parent != null &&
        patchRow(feed, parent, (task) => ({
          ...task,
          children: [...task.children, row],
        }));
      if (!nested) startCall(feed, row);
      return;
    }

    case "subagentUsage":
      patchRow(feed, event.callId, (row) => ({ ...row, tokens: event.tokens }));
      return;

    case "toolCallProgress":
      patchRow(feed, event.callId, (row) => ({
        ...row,
        output: row.output + event.chunk,
      }));
      return;

    case "toolCallFinished":
      patchRow(feed, event.callId, (row) => ({
        ...row,
        state: "done",
        meta: event.meta,
      }));
      return;

    case "toolCallFailed":
      patchRow(feed, event.callId, (row) => ({
        ...row,
        state: "failed",
        message: event.message,
      }));
      return;

    case "toolCallRejected":
      patchRow(feed, event.callId, (row) => ({
        ...row,
        state: "rejected",
        message: event.reason,
      }));
      return;

    case "toolCallDenied":
      // A refused permission and a stopped running call are one kind of outcome
      // (the human's decision), but not one phrase.
      patchRow(feed, event.callId, (row) => ({
        ...row,
        state: event.stopped ? "stopped" : "denied",
        message: event.reason,
      }));
      return;

    case "fileChanged":
      // The file panel's index is kept here too: the panel shows
      // the latest thing the core sent for this path.
      feed.diffs = { ...feed.diffs, [event.fact.path]: event.diff };
      feed.writes += 1;
      patchRow(feed, event.callId, (row) =>
        MULTI_FILE_TOOLS.has(row.tool)
          ? { ...row, children: withFile(row, event.fact, event.diff) }
          : { ...row, fact: event.fact, diff: event.diff },
      );
      return;

    case "fileViewLoaded":
      feed.fileView = { path: event.path, view: event.view };
      return;

    case "todosUpdated": {
      // Updated in place; no new block is created.
      const at = feed.blocks.findIndex((b) => b.kind === "todos");
      const block: FeedBlock = {
        kind: "todos",
        key: at === -1 ? key("todos") : feed.blocks[at].key,
        items: event.items,
      };
      if (at === -1) feed.blocks.push(block);
      else feed.blocks[at] = block;
      return;
    }

    case "permissionRequested":
      feed.request = event.request;
      feed.blocks.push({
        kind: "permission",
        key: key("ask"),
        request: event.request,
        open: true,
      });
      return;

    case "permissionResolved":
      feed.request = null;
      closeRequest(feed, event.requestId);
      return;

    case "questionAsked":
      feed.question = event.request;
      feed.blocks.push({
        kind: "question",
        key: key("ques"),
        request: event.request,
        status: "open",
        reply: null,
      });
      return;

    case "questionResolved":
      if (feed.question?.id === event.requestId) feed.question = null;
      settleQuestion(feed, event.requestId, event.reply);
      return;

    case "elicitationRequested":
      // A window that reconnects is sent the open forms again: one block per
      // form, whichever way it arrived.
      if (feed.blocks.some((b) => b.kind === "elicitation" && b.request.id === event.request.id)) {
        return;
      }
      feed.blocks.push({
        kind: "elicitation",
        key: key("elicit"),
        request: event.request,
        status: "open",
      });
      return;

    case "elicitationResolved":
      settleElicitation(feed, event.requestId, event.reply);
      return;

    case "planProposed":
      feed.blocks.push({ kind: "plan", key: key("plan"), plan: event.plan });
      return;

    case "textShown":
      feed.blocks.push({
        kind: "shown",
        key: key("shown"),
        path: event.path,
        command: event.command,
        firstLine: event.firstLine,
        lines: event.lines,
        total: event.total,
        text: event.text,
      });
      return;

    case "checkpointCreated":
      feed.blocks.push({
        kind: "checkpoint",
        key: key("ckpt"),
        checkpoint: event.checkpoint,
      });
      return;

    case "checkpointPreviewed":
      feed.preview = { id: event.id, files: event.files };
      return;

    case "checkpointRestored":
      feed.preview = null;
      feed.blocks.push({
        kind: "restored",
        key: key("back"),
        at: event.at,
        files: event.files,
      });
      return;

    case "contextCompacted":
      feed.blocks.push({
        kind: "compacted",
        key: key("compact"),
        summary: event.summary,
        freed: event.freed,
      });
      return;

    case "summaryEdited": {
      // The summary is edited, not written again: the divider stays the same.
      const at = lastIndex(feed, (b) => b.kind === "compacted");
      if (at !== -1) {
        const block = feed.blocks[at];
        if (block.kind === "compacted") {
          feed.blocks[at] = { ...block, summary: event.summary };
        }
      }
      return;
    }

    case "error":
      feed.blocks.push({
        kind: "error",
        key: key("err"),
        code: event.code,
        message: event.message,
        retryable: event.retryable,
      });
      return;

    case "retryScheduled":
      feed.blocks.push({
        kind: "retry",
        key: key("retry"),
        attempt: event.attempt,
        of: event.of,
        delayMs: event.delayMs,
      });
      return;

    // Which model answers this turn: a line of its own before the text, so the
    // reader knows who wrote it. The core sends it once per user message, not
    // once per request of the tool loop.
    case "routeDecided":
      feed.blocks.push({
        kind: "route",
        key: key("route"),
        route: event.route,
        provider: event.provider,
        model: event.model,
      });
      return;

    case "hookReported":
      feed.blocks.push({
        kind: "hook",
        key: key("hook"),
        plugin: event.plugin,
        event: event.event,
        text: event.text,
        blocked: event.blocked,
      });
      return;

    case "turnCancelled":
      // Unfinished tools get "cancelled"; the feed gets a line.
      cancelRunning(feed);
      feed.blocks.push({ kind: "stopped", key: key("stop") });
      endTurn(feed, event.at);
      return;

    case "processOutputReset":
      // The human picked a row: the whole buffer, dropping whatever was there before.
      feed.processOutput = {
        id: event.id,
        text: event.text,
        dropped: event.dropped,
      };
      return;

    case "processOutput":
      // The live tail arrives only for the selected row, and a selection is always
      // announced by a reset. A chunk with someone else's id is not appended: it would
      // replace the buffer of the one being looked at.
      if (feed.processOutput?.id === event.id) {
        feed.processOutput = {
          ...feed.processOutput,
          text: feed.processOutput.text + event.chunk,
        };
      }
      return;

    case "processesUpdated":
      feed.processes = event.processes;
      return;

    case "mcpUpdated":
      feed.mcp = event.servers;
      return;

    case "lspUpdated":
      feed.lsp = event.servers;
      return;

    case "checkpointsUpdated":
      feed.checkpoints = event.checkpoints;
      return;

    case "usageUpdated":
      feed.usage = event.usage;
      return;

    case "stateChanged":
      feed.state = event.state;
      return;

    case "queueUpdated":
      feed.queued = event.queued;
      return;

    case "filesFound":
      feed.found = { query: event.query, paths: event.paths };
      return;

    // The ring and the send button read `state.busy`, which arrives as its own
    // event; the turn's time is drawn separately, by a live timer and a line under
    // the answer (`endTurn`).
    case "turnStarted":
      feed.turn = { startedAt: event.at };
      return;

    case "turnFinished":
      endTurn(feed, event.at);
      return;
  }
}

/**
 * Closes the running turn's clock. No time line when either end is unknown:
 * a journal from before the core stamped turns carries `at: 0`, and "worked
 * 492 000 hours" is worse than nothing.
 */
function endTurn(feed: Feed, endedAt: number): void {
  const started = feed.turn?.startedAt ?? 0;
  feed.turn = null;
  if (started <= 0 || endedAt < started) return;
  feed.blocks.push({
    kind: "turnTime",
    key: key("time"),
    startedAt: started,
    endedAt,
  });
}

function appendText(feed: Feed, blockId: string, text: string): void {
  const last = feed.blocks[feed.blocks.length - 1];
  if (last && last.kind === "agent" && last.blockId === blockId) {
    feed.blocks[feed.blocks.length - 1] = { ...last, text: last.text + text };
    return;
  }
  feed.blocks.push({ kind: "agent", key: key("text"), blockId, text });
}

function appendThinking(feed: Feed, blockId: string, text: string): void {
  const last = feed.blocks[feed.blocks.length - 1];
  if (last && last.kind === "reasoning" && last.blockId === blockId) {
    feed.blocks[feed.blocks.length - 1] = { ...last, text: last.text + text };
    return;
  }
  // No duration: the reasoning row asks for "reasoning · 12 s", but the stream has no
  // start time, and measuring it with the webview's clock would show, in a session
  // reopened later, the time of a different run.
  feed.blocks.push({ kind: "reasoning", key: key("think"), blockId, text });
}

/**
 * Actions in a row are one feed entry, and only in a row: reasoning or a reply
 * between calls stands between them as a separate block.
 *
 * The reducer keeps the chronology and folds nothing. The calls and the reasoning
 * between replies are gathered into one block by `core/work`, for display: it needs
 * the order in which everything happened, and a group that pulled calls across
 * reasoning would have lost that order.
 */
function startCall(feed: Feed, row: ActionRow): void {
  const at = feed.blocks.length - 1;
  const last = feed.blocks[at];
  if (last && last.kind === "actions") {
    feed.blocks[at] = { ...last, rows: [...last.rows, row] };
    return;
  }
  feed.blocks.push({ kind: "actions", key: key("acts"), rows: [row] });
}

/** Changes a row by its id; says whether it was found. */
function patchRow(
  feed: Feed,
  callId: string,
  change: (row: ActionRow) => ActionRow,
): boolean {
  for (let i = feed.blocks.length - 1; i >= 0; i -= 1) {
    const block = feed.blocks[i];
    if (block.kind !== "actions") continue;
    const rows = patchIn(block.rows, callId, change);
    if (rows === null) continue;
    feed.blocks[i] = { ...block, rows };
    return true;
  }
  return false;
}

/** The list with the row changed, its own or nested; `null` if it is not here. */
function patchIn(
  rows: ActionRow[],
  callId: string,
  change: (row: ActionRow) => ActionRow,
): ActionRow[] | null {
  for (let at = rows.length - 1; at >= 0; at -= 1) {
    const row = rows[at];
    let next: ActionRow | null = null;
    if (row.callId === callId) {
      next = change(row);
    } else if (row.children.length > 0) {
      const children = patchIn(row.children, callId, change);
      if (children !== null) next = { ...row, children };
    }
    if (next === null) continue;
    const out = rows.slice();
    out[at] = next;
    return out;
  }
  return null;
}

function cancelRunning(feed: Feed): void {
  for (let i = feed.blocks.length - 1; i >= 0; i -= 1) {
    const block = feed.blocks[i];
    if (block.kind !== "actions") continue;
    if (!flatRows(block.rows).some((r) => r.state === "running")) continue;
    feed.blocks[i] = { ...block, rows: block.rows.map(cancelled) };
  }
}

function cancelled(row: ActionRow): ActionRow {
  return {
    ...row,
    ...(row.state === "running" ? { state: "failed" as const, message: null } : {}),
    children: row.children.map(cancelled),
  };
}

/**
 * Rows together with their nested ones, in order of appearance.
 *
 * For everything that counts actions rather than draws them: what was touched, how
 * many were created, which diff is the last. A file changed by a subagent is changed,
 * and both in the group header and in the file panel it stands on a par with the rest.
 */
/**
 * The rows under a many-file call, with this file's in place: an edit of the
 * file it already has replaces that row rather than adding a second.
 */
function withFile(row: ActionRow, fact: FileFact, diff: FileDiff): ActionRow[] {
  const file: ActionRow = {
    callId: `${row.callId}#${fact.path}`,
    tool: "Edit",
    description: null,
    target: fact.path,
    input: null,
    state: "done",
    message: null,
    meta: null,
    output: "",
    fact,
    diff,
    children: [],
    tokens: null,
  };
  const at = row.children.findIndex((child) => child.fact?.path === fact.path);
  if (at === -1) return [...row.children, file];
  const children = row.children.slice();
  children[at] = file;
  return children;
}

export function flatRows(rows: ActionRow[]): ActionRow[] {
  return rows.flatMap((row) => [row, ...flatRows(row.children)]);
}

function closeRequest(feed: Feed, requestId: string): void {
  const at = lastIndex(
    feed,
    (b) => b.kind === "permission" && b.request.id === requestId,
  );
  if (at === -1) return;
  const block = feed.blocks[at];
  if (block.kind === "permission") feed.blocks[at] = { ...block, open: false };
}

/**
 * Closes the question with an answer, a refusal, or nothing.
 *
 * Writes to the same block rather than adding a new one: there is one question, and
 * its row in the feed should become the outcome, not a second entry about the same thing.
 */
function settleQuestion(
  feed: Feed,
  requestId: string,
  reply: QuestionReply | null,
): void {
  const at = lastIndex(
    feed,
    (b) => b.kind === "question" && b.request.id === requestId,
  );
  if (at === -1) return;
  const block = feed.blocks[at];
  if (block.kind !== "question") return;
  const status: QuestionStatus =
    reply === null ? "unanswered" : reply.type === "dismissed" ? "dismissed" : "answered";
  feed.blocks[at] = { ...block, status, reply };
}

function settleElicitation(feed: Feed, requestId: string, reply: ElicitationReply | null): void {
  const at = lastIndex(feed, (b) => b.kind === "elicitation" && b.request.id === requestId);
  if (at === -1) return;
  const block = feed.blocks[at];
  if (block.kind !== "elicitation") return;
  const status: ElicitationStatus =
    reply === null
      ? "expired"
      : reply.type === "accept"
        ? "accepted"
        : reply.type === "decline"
          ? "declined"
          : "cancelled";
  feed.blocks[at] = { ...block, status };
}

function lastIndex(feed: Feed, match: (b: FeedBlock) => boolean): number {
  for (let i = feed.blocks.length - 1; i >= 0; i -= 1) {
    if (match(feed.blocks[i])) return i;
  }
  return -1;
}

/** What is visible in the feed: closed permission requests leave it. */
export function visible(feed: Feed): FeedBlock[] {
  return feed.blocks.filter((b) => b.kind !== "permission" || b.open);
}

/**
 * The diff of a file for the file panel.
 *
 * The staging index comes first: it holds the latest thing the core sent for the path.
 * A panel opened from an old action row after applying falls back to the diff of that
 * row itself: the set is already empty, but there is no reason to hide what the feed has.
 */
/**
 * Files the session has touched, in order of appearance.
 *
 * `⌥←` and `⌥→` page through the file panel by this list. It used to come from the set
 * of edits; now it is derived from the feed, which already knows all of this.
 */
export function touchedFiles(feed: Feed): string[] {
  const out: string[] = [];
  for (const block of feed.blocks) {
    if (block.kind !== "actions") continue;
    for (const row of flatRows(block.rows)) {
      const path = row.fact?.path;
      if (path && !out.includes(path)) out.push(path);
    }
  }
  return out;
}

/**
 * The latest thing the core said about a file: the kind of change and the counters.
 *
 * It is looked up where the diff is, for the same reason: the set of edits that would
 * know this separately no longer exists.
 */
export function factOf(feed: Feed, path: string): FileFact | null {
  for (let i = feed.blocks.length - 1; i >= 0; i -= 1) {
    const block = feed.blocks[i];
    if (block.kind !== "actions") continue;
    const rows = flatRows(block.rows);
    for (let j = rows.length - 1; j >= 0; j -= 1) {
      const fact = rows[j].fact;
      if (fact?.path === path) return fact;
    }
  }
  return null;
}

export function diffOf(feed: Feed, path: string): FileDiff | null {
  const staged = feed.diffs[path];
  if (staged) return staged;
  for (let i = feed.blocks.length - 1; i >= 0; i -= 1) {
    const block = feed.blocks[i];
    if (block.kind !== "actions") continue;
    const rows = flatRows(block.rows);
    for (let j = rows.length - 1; j >= 0; j -= 1) {
      const row = rows[j];
      if (row.fact?.path === path && row.diff) return row.diff;
    }
  }
  return null;
}
