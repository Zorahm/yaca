/**
 * Лента как чистая функция от потока событий.
 *
 * Ядро обещает, что интерфейс — функция от `AgentEvent`, и вот она. Всё, что
 * рисуется в ленте и панелях, выводится отсюда: копить состояние в компонентах значило бы
 * иметь вторую версию правды, а она расходится с первой на первом же отклонении
 * файла.
 *
 * Два свойства держат остальное.
 *
 * *Список блоков только растёт.* Блок разрешения не удаляется после ответа, а
 * помечается закрытым и не рисуется: так индексы блоков не съезжают, и лента
 * ведёт себя как журнал, из которого её же и восстанавливают.
 *
 * *Кадр — одна перерасчёт.* Мост шлёт события пачками по кадру, поэтому
 * `reduceAll` создаёт один новый снимок на пачку, а не на дельту.
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
import type { ProcessSummary } from "./bindings/ProcessSummary";
import type { SessionState } from "./bindings/SessionState";
import type { TodoItem } from "./bindings/TodoItem";
import type { TokenCounts } from "./bindings/TokenCounts";
import type { ToolMeta } from "./bindings/ToolMeta";
import type { UsageSnapshot } from "./bindings/UsageSnapshot";

/** Чем кончилось действие. `running` — точка `●` и настоящее время. */
export type ActionState = "running" | "done" | "failed" | "rejected" | "denied" | "stopped";

/** Одна строка в группе действий. */
export type ActionRow = {
  callId: string;
  /** Имя тула. В ленте не показывается — нужно, чтобы выбрать форму. */
  tool: string;
  /** Подпись модели либо `null` у тулов, которые подписывает лента. */
  description: string | null;
  /** Первая строка раскрытой строки: полный путь или полная команда. */
  target: string | null;
  input: unknown;
  state: ActionState;
  /** Причина неудачи, отказа инварианта или отказа человека. */
  message: string | null;
  meta: ToolMeta | null;
  output: string;
  /** Заполнено у файловых действий: отсюда подпись и счётчики. */
  fact: FileFact | null;
  diff: FileDiff | null;
  /**
   * Вызовы подагента — у строки `Task`, которая его отправила. Строки как
   * строки: правка подагента — правка в файлах человека, и дифф ей положен тот
   * же. Вложенность одна: подагент подагента не запускает.
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
 * Где стоит вопрос `AskUser` (блок, похожий на запрос разрешения).
 *
 * `open` — ждёт человека; `answered` и `dismissed` — что он сделал; `unanswered` —
 * турн остановили раньше ответа. Блок остаётся в ленте во всех четырёх: вопрос,
 * на который ответили, — это строка разговора, а не запрос, который можно убрать.
 */
export type QuestionStatus = "open" | "answered" | "dismissed" | "unanswered";

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
      /** Что выбрано, когда ответ есть; `null`, пока открыто или без ответа. */
      reply: QuestionReply | null;
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
      /** 1-indexed. */
      firstLine: number;
      lines: number;
      /** Lines in the file: `lines === total` is the whole of it. */
      total: number;
      text: string;
    }
  | { kind: "error"; key: string; code: string; message: string; retryable: boolean }
  | { kind: "retry"; key: string; attempt: number; of: number; delayMs: number }
  | { kind: "stopped"; key: string }
  /**
   * How long a turn took and when it ended, under what it said. Made of
   * `TurnStarted.at` and the end event's `at`: the core's clock, not the
   * window's, so a reopened session shows the same figures it did live.
   */
  | { kind: "turnTime"; key: string; startedAt: number; endedAt: number };

export type Feed = {
  blocks: FeedBlock[];
  /** Нижний ряд композера: режимы, модель, занятость. */
  state: SessionState | null;
  /** Кольцо контекста. */
  usage: UsageSnapshot | null;
  /** Панель процессов. */
  processes: ProcessSummary[];
  /**
   * Серверы MCP этого чата, целиком, как их отдало ядро последним
   * `mcpUpdated`. `null` — список ещё не приезжал.
   */
  mcp: McpServerSummary[] | null;
  /**
   * Language servers of this chat, whole, as the core last sent them in
   * `lspUpdated`. `null` until the list has arrived.
   */
  lsp: LspServerSummary[] | null;
  /**
   * Вывод выбранного процесса. Приходит только для той строки, на
   * которую смотрят: `processOutputReset` кладёт буфер целиком, живые куски
   * дописываются. Один слот, а не карта: смотреть человек может только один.
   */
  processOutput: { id: ProcessId; text: string; dropped: number } | null;
  /**
   * Последний дифф по каждому пути — индекс панели файла.
   *
   * Стейджинги приходят и без строки действия: при конфликте применения
   * `callId` пуст, а дифф показывать надо. Последний по пути выигрывает —
   * панель файла показывает файл, каким он сейчас в наборе, а не его первую
   * правку.
   */
  diffs: Record<string, FileDiff>;
  /**
   * Сколько записей файлов пришло за сессию. Панель файла перечитывает
   * вид, когда число растёт: новая запись меняет то, что она показывает.
   */
  writes: number;
  /**
   * Полный дифф одного файла для панели, последний пришедший. Один слот:
   * панель файла одна. `view: null` — в файле ничего не ждёт решения.
   */
  fileView: { path: string; view: FileView | null } | null;
  /** Панель чекпоинтов. */
  checkpoints: CheckpointSummary[];
  /** Активный запрос разрешения, если он есть. */
  request: PermissionRequest | null;
  /** Открытый вопрос `AskUser`, если он есть: нижний ряд и горячие клавиши ждут его. */
  question: QuestionRequest | null;
  /**
   * Что тронет откат, если его подтвердить.
   *
   * Живёт в ленте, а не в кнопке: список приходит событием, и показать его
   * должна та же строка чекпоинта, у которой спросили.
   */
  preview: { id: string; files: CheckpointChange[] } | null;
  /**
   * Сообщения, которые ждут своей очереди в ходе, по порядку. Список
   * приходит целиком от ядра: отмена и влитие меняют его там, а здесь только
   * показывается то, что пришло последним.
   */
  queued: QueuedMessage[];
  /**
   * Последний ответ на `@`-поиск. Несёт запрос, на который отвечает:
   * композер показывает его, только когда запрос всё ещё тот, что набран.
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

/** Пачка событий — один снимок ленты. */
export function reduceAll(feed: Feed, events: AgentEvent[]): Feed {
  if (events.length === 0) return feed;
  const draft: Feed = { ...feed, blocks: feed.blocks.slice() };
  for (const event of events) apply(draft, event);
  return draft;
}

let sequence = 0;
/** Ключ блока. Свой, а не индекс: блоки живут дольше, чем перерисовка. */
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
      // Вызов подагента встаёт под свою строку `Task`, где бы та ни была:
      // между ними мог пройти запрос разрешения, а он группу закрывает.
      // Родитель, которого в ленте нет, — не повод потерять действие.
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
      // Отказ в разрешении и остановка работающего вызова — один род исхода
      // (решение человека), но не одно предложение.
      patchRow(feed, event.callId, (row) => ({
        ...row,
        state: event.stopped ? "stopped" : "denied",
        message: event.reason,
      }));
      return;

    case "fileChanged":
      // Индекс панели файла держится здесь же: панель показывает
      // последнее, что ядро прислало по этому пути.
      feed.diffs = { ...feed.diffs, [event.fact.path]: event.diff };
      feed.writes += 1;
      patchRow(feed, event.callId, (row) => ({
        ...row,
        fact: event.fact,
        diff: event.diff,
      }));
      return;

    case "fileViewLoaded":
      feed.fileView = { path: event.path, view: event.view };
      return;

    case "todosUpdated": {
      // Обновляется на месте, новый блок не создаётся.
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

    case "planProposed":
      feed.blocks.push({ kind: "plan", key: key("plan"), plan: event.plan });
      return;

    case "textShown":
      feed.blocks.push({
        kind: "shown",
        key: key("shown"),
        path: event.path,
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
      // Сводку правят, а не пишут заново — разделитель тот же.
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

    case "turnCancelled":
      // Незавершённые тулы получают «отменено», в ленте строка.
      cancelRunning(feed);
      feed.blocks.push({ kind: "stopped", key: key("stop") });
      endTurn(feed, event.at);
      return;

    case "processOutputReset":
      // Человек выбрал строку — буфер целиком, сбрасывая то, что было до него.
      feed.processOutput = {
        id: event.id,
        text: event.text,
        dropped: event.dropped,
      };
      return;

    case "processOutput":
      // Живой хвост приходит только для выбранной строки, и выбор всегда
      // объявляется сбросом. Кусок чужого id не дописываем: он подменил бы
      // буфер того, на кого смотрят.
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

    // Кольцо и кнопка отправки читают `state.busy`, который приходит своим
    // событием; время хода рисуется отдельно — живым таймером и строкой под
    // ответом (`endTurn`).
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
  // Без длительности: строка рассуждения просит «рассуждение · 12 с», но времени начала в
  // потоке нет, а замерить его часами вебвью — значит показать в открытой
  // заново сессии время не того прогона.
  feed.blocks.push({ kind: "reasoning", key: key("think"), blockId, text });
}

/**
 * Действия подряд — одна запись ленты, и только подряд: рассуждение или
 * реплика между вызовами встаёт между ними отдельным блоком.
 *
 * Редьюсер держит хронологию и ничего не сворачивает. В один блок вызовы и
 * рассуждения между репликами собирает `core/work` — уже для показа: ему нужен порядок, в котором всё происходило, а группа, тянущая в
 * себя вызовы через рассуждение, этот порядок теряла.
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

/** Меняет строку по идентификатору; говорит, нашлась ли она. */
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

/** Список с изменённой строкой — своей или вложенной; `null`, если её тут нет. */
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
 * Строки вместе с вложенными, в порядке появления.
 *
 * Для всего, что считает по действиям, а не рисует их: что тронуто, сколько
 * создано, какой дифф последний. Файл, изменённый подагентом, изменён — и в
 * шапке группы, и в панели файла он стоит наравне с остальными.
 */
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
 * Закрывает вопрос ответом, отказом или ничем.
 *
 * Пишет в тот же блок, а не добавляет новый: вопрос один, и его строка в ленте
 * должна стать итогом, а не второй записью про то же.
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

function lastIndex(feed: Feed, match: (b: FeedBlock) => boolean): number {
  for (let i = feed.blocks.length - 1; i >= 0; i -= 1) {
    if (match(feed.blocks[i])) return i;
  }
  return -1;
}

/** Что видно в ленте: закрытые запросы разрешения из неё уходят. */
export function visible(feed: Feed): FeedBlock[] {
  return feed.blocks.filter((b) => b.kind !== "permission" || b.open);
}

/**
 * Дифф файла для панели файла.
 *
 * Сперва индекс стейджингов — там лежит последнее, что ядро прислало по пути.
 * Панель, открытая по старой строке действия после применения, падает к диффу
 * самой строки: набор уже пуст, но прятать то, что в ленте есть, причин нет.
 */
/**
 * Файлы, которых сессия касалась, в порядке появления.
 *
 * По этому списку `⌥←` и `⌥→` листают панель файла. Раньше его давал набор
 * правок; теперь он выводится из ленты, которая и так всё это знает.
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
 * Последнее, что ядро сказало про файл: вид изменения и счётчики.
 *
 * Ищется там же, где дифф, и по той же причине — набора правок, который знал
 * бы это отдельно, больше нет.
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
