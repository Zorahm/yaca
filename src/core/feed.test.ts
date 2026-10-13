/**
 * Золотые тесты ленты — зеркало тех, что в ядре.
 *
 * Ядро проверяет, что поток событий той самой формы; здесь проверяется, что
 * лента есть функция от него. Вместе это и есть обещание §15, и ломается оно
 * тут, а не глазами.
 */

import { describe, expect, it } from "vitest";

import type { AgentEvent } from "./bindings/AgentEvent";
import type { QueuedMessage } from "./bindings/QueuedMessage";
import type { QuestionReply } from "./bindings/QuestionReply";
import {
  diffOf,
  emptyFeed,
  factOf,
  flatRows,
  reduceAll,
  touchedFiles,
  visible,
  type Feed,
} from "./feed";

function run(...events: AgentEvent[]): Feed {
  return reduceAll(emptyFeed, events);
}

const said = (text: string): AgentEvent => ({
  type: "userMessage",
  turnId: "t1",
  text,
  attachments: [],
});

const wrote = (blockId: string, text: string): AgentEvent => ({
  type: "textDelta",
  turnId: "t1",
  blockId,
  text,
});

const thought = (blockId: string, text: string): AgentEvent => ({
  type: "thinkingDelta",
  turnId: "t1",
  blockId,
  text,
});

const started = (callId: string, tool = "Read", parent: string | null = null): AgentEvent => ({
  type: "toolCallStarted",
  turnId: "t1",
  callId,
  tool,
  description: "Прочитал файл",
  target: "src/a.rs",
  input: {},
  parent,
});

const finished = (callId: string): AgentEvent => ({
  type: "toolCallFinished",
  callId,
  meta: { lines: 3, items: null, truncated: false, durationMs: 12 },
});

describe("лента", () => {
  it("склеивает дельты одного блока в одну реплику", () => {
    const feed = run(wrote("b1", "Смотрю "), wrote("b1", "на парсер."));

    expect(feed.blocks).toHaveLength(1);
    expect(feed.blocks[0]).toMatchObject({
      kind: "agent",
      text: "Смотрю на парсер.",
    });
  });

  it("новый блок модели — новая реплика", () => {
    const feed = run(wrote("b1", "раз"), wrote("b2", "два"));
    expect(feed.blocks).toHaveLength(2);
  });

  it("собирает подряд идущие действия в одну группу", () => {
    const feed = run(started("c1"), started("c2"), started("c3"));

    expect(feed.blocks).toHaveLength(1);
    expect(feed.blocks[0]).toMatchObject({ kind: "actions" });
    if (feed.blocks[0].kind !== "actions") throw new Error("не группа");
    expect(feed.blocks[0].rows).toHaveLength(3);
  });

  it("реплика и рассуждение между действиями встают между ними", () => {
    // Редьюсер держит порядок; в блоки работу хода собирает `core/work`.
    const feed = run(
      started("c1"),
      started("c2"),
      thought("t1", "теперь тесты"),
      started("c3"),
      wrote("b1", "теперь стили"),
      started("c4"),
    );

    expect(feed.blocks.map((b) => b.kind)).toEqual([
      "actions",
      "reasoning",
      "actions",
      "agent",
      "actions",
    ]);
    if (feed.blocks[0].kind !== "actions") throw new Error("не группа");
    expect(feed.blocks[0].rows).toHaveLength(2);
  });

  it("исход действия доезжает до своей строки", () => {
    const feed = run(
      started("c1"),
      started("c2"),
      finished("c1"),
      { type: "toolCallFailed", callId: "c2", message: "не найдено" },
    );

    if (feed.blocks[0].kind !== "actions") throw new Error("не группа");
    const [one, two] = feed.blocks[0].rows;
    expect(one.state).toBe("done");
    expect(one.meta?.durationMs).toBe(12);
    expect(two.state).toBe("failed");
    expect(two.message).toBe("не найдено");
  });

  it("вывод команды копится в своей строке", () => {
    const feed = run(
      started("c1", "Bash"),
      { type: "toolCallProgress", callId: "c1", chunk: "раз\n" },
      { type: "toolCallProgress", callId: "c1", chunk: "два\n" },
    );

    if (feed.blocks[0].kind !== "actions") throw new Error("не группа");
    expect(feed.blocks[0].rows[0].output).toBe("раз\nдва\n");
  });

  it("правка приезжает в строку диффом и счётчиками", () => {
    const feed = run(started("c1", "Edit"), {
      type: "fileChanged",
      callId: "c1",
      fact: {
        path: "src/a.rs",
        kind: "modified",
        movedFrom: null,
        added: 4,
        removed: 2,
      },
      diff: { hunks: [], added: 4, removed: 2, truncated: false },
    });

    if (feed.blocks[0].kind !== "actions") throw new Error("не группа");
    expect(feed.blocks[0].rows[0].fact?.added).toBe(4);
    expect(feed.blocks[0].rows[0].diff).not.toBeNull();
  });

  it("список задач обновляется на месте", () => {
    // §5.6: новый блок не создаётся.
    const feed = run(
      { type: "todosUpdated", items: [{ content: "раз", status: "pending" }] },
      wrote("b1", "работаю"),
      {
        type: "todosUpdated",
        items: [{ content: "раз", status: "completed" }],
      },
    );

    const todos = feed.blocks.filter((b) => b.kind === "todos");
    expect(todos).toHaveLength(1);
    expect(feed.blocks[0].kind).toBe("todos");
    if (feed.blocks[0].kind !== "todos") throw new Error("не задачи");
    expect(feed.blocks[0].items[0].status).toBe("completed");
  });

  it("отвеченный запрос разрешения уходит из ленты, но не из истории", () => {
    const request = {
      id: "req_1",
      callId: "c1",
      kind: "runCommand" as const,
      target: "npm",
      detail: "npm ci",
      note: "/p",
      sizeBytes: null,
      files: null,
      unsaved: null,
      invariant: null,
    server: null,
      scopes: [],
      position: 1,
      total: 1,
    };
    const feed = run(
      { type: "permissionRequested", request },
      { type: "permissionResolved", requestId: "req_1" },
    );

    expect(feed.request).toBeNull();
    expect(visible(feed)).toHaveLength(0);
    // Блоки только добавляются: на этом держатся индексы и повтор из журнала.
    expect(feed.blocks).toHaveLength(1);
  });

  it("остановка хода помечает незавершённые действия", () => {
    const feed = run(started("c1"), started("c2"), finished("c1"), {
      type: "turnCancelled",
      turnId: "t1",
      at: 0,
    });

    if (feed.blocks[0].kind !== "actions") throw new Error("не группа");
    expect(feed.blocks[0].rows.map((r) => r.state)).toEqual(["done", "failed"]);
    expect(feed.blocks[1].kind).toBe("stopped");
  });

  it("правка сводки не заводит второй разделитель", () => {
    // §5.11: сводку правят перед продолжением, а не пишут заново.
    const feed = run(
      { type: "contextCompacted", summary: "первая", freed: 84000 },
      { type: "summaryEdited", summary: "поправленная" },
    );

    expect(feed.blocks).toHaveLength(1);
    expect(feed.blocks[0]).toMatchObject({
      kind: "compacted",
      summary: "поправленная",
      freed: 84000,
    });
  });

  it("сообщение человека приходит из потока, а не из отправки", () => {
    // Иначе открытая заново сессия показывала бы ответы без вопросов (§9).
    const feed = run(said("почини разбор дат"), wrote("b1", "смотрю"));
    expect(feed.blocks[0]).toMatchObject({
      kind: "human",
      text: "почини разбор дат",
    });
  });

  it("пустая пачка не создаёт нового снимка", () => {
    // §12: перерисовка на кадр, а не на событие.
    const feed = run(wrote("b1", "раз"));
    expect(reduceAll(feed, [])).toBe(feed);
  });
});

describe("панели", () => {
  const changed = (callId: string, path: string, added = 2): AgentEvent => ({
    type: "fileChanged",
    callId,
    fact: { path, kind: "modified", movedFrom: null, added, removed: 1 },
    diff: { hunks: [], added, removed: 1, truncated: false },
  });

  it("правка пополняет индекс панели файла", () => {
    const feed = run(started("c1", "Edit"), changed("c1", "a.rs"));

    expect(feed.diffs["a.rs"]).toBeTruthy();
    expect(diffOf(feed, "a.rs")).toBe(feed.diffs["a.rs"]);
  });

  it("diffOf падает к диффу строки действия", () => {
    const feed = run(started("c3", "Edit"), changed("c3", "src/a.rs"));

    // Индекс и строка держат одно событие; проверяем именно падение, выкинув индекс.
    expect(diffOf({ ...feed, diffs: {} }, "src/a.rs")).toBeTruthy();
    expect(diffOf({ ...feed, diffs: {} }, "src/чужой.rs")).toBeNull();
  });

  it("последняя правка пути выигрывает", () => {
    // Панель файла показывает файл, каким он стал, а не первую правку.
    const feed = run(
      started("c1", "Edit"),
      changed("c1", "a.rs", 2),
      started("c2", "Edit"),
      changed("c2", "a.rs", 5),
    );

    expect(feed.diffs["a.rs"].added).toBe(5);
    expect(factOf(feed, "a.rs")?.added).toBe(5);
    expect(touchedFiles(feed)).toEqual(["a.rs"]);
  });

  it("вывод процесса: сброс кладёт буфер, куски дописываются к своему id", () => {
    const feed = run(
      { type: "processOutputReset", id: 3, text: "ready\n", dropped: 0 },
      { type: "processOutput", id: 3, chunk: "compiled\n" },
    );

    expect(feed.processOutput).toEqual({ id: 3, text: "ready\ncompiled\n", dropped: 0 });
  });

  it("кусок чужого процесса не подменяет буфер выбранного", () => {
    // Выбор объявляется сбросом; кусок без сброса — не тот, на кого смотрят.
    const feed = run(
      { type: "processOutputReset", id: 3, text: "ready\n", dropped: 0 },
      { type: "processOutput", id: 7, chunk: "чужой\n" },
    );

    expect(feed.processOutput).toEqual({ id: 3, text: "ready\n", dropped: 0 });
  });

  it("новый выбор сбрасывает буфер целиком", () => {
    const feed = run(
      { type: "processOutputReset", id: 3, text: "ready\n", dropped: 0 },
      { type: "processOutput", id: 3, chunk: "compiled\n" },
      { type: "processOutputReset", id: 4, text: "listening\n", dropped: 12 },
    );

    expect(feed.processOutput).toEqual({ id: 4, text: "listening\n", dropped: 12 });
  });
});

describe("подагент", () => {
  const changed = (callId: string, path: string): AgentEvent => ({
    type: "fileChanged",
    callId,
    fact: { path, kind: "created", movedFrom: null, added: 3, removed: 0 },
    diff: { hunks: [], added: 3, removed: 0, truncated: false },
  });

  const asked = (callId: string): AgentEvent => ({
    type: "permissionRequested",
    request: {
      id: "req_1",
      callId,
      kind: "editFile",
      target: "notes.txt",
      detail: "",
      note: null,
      sizeBytes: null,
      files: null,
      unsaved: null,
      invariant: null,
    server: null,
      scopes: [],
      position: 1,
      total: 1,
    },
  });

  /** Строки верхнего уровня: то, что группа рисует сама. */
  function top(feed: Feed) {
    return feed.blocks.flatMap((b) => (b.kind === "actions" ? b.rows : []));
  }

  it("его вызовы встают под строку Task, а не рядом с ней", () => {
    const feed = run(
      started("t1", "Task"),
      started("r1", "Read", "t1"),
      finished("r1"),
      started("r2", "Read", "t1"),
    );

    expect(top(feed).map((r) => r.callId)).toEqual(["t1"]);
    expect(top(feed)[0].children.map((r) => [r.callId, r.state])).toEqual([
      ["r1", "done"],
      ["r2", "running"],
    ]);
  });

  it("запрос разрешения не отрывает вызов от своей строки", () => {
    // Запрос закрывает группу (§5.4), а строка подагента к группе не
    // прибивается: она идёт к своему Task, где бы тот ни остался.
    const feed = run(
      started("t1", "Task"),
      started("w1", "Write", "t1"),
      asked("w1"),
      { type: "permissionResolved", requestId: "req_1" },
      started("r1", "Read", "t1"),
    );

    expect(top(feed).map((r) => r.callId)).toEqual(["t1"]);
    expect(top(feed)[0].children.map((r) => r.callId)).toEqual(["w1", "r1"]);
  });

  it("два подагента не путают строки друг друга", () => {
    const feed = run(
      started("t1", "Task"),
      started("t2", "Task"),
      started("a", "Read", "t2"),
      started("b", "Read", "t1"),
    );

    const [first, second] = top(feed);
    expect(first.children.map((r) => r.callId)).toEqual(["b"]);
    expect(second.children.map((r) => r.callId)).toEqual(["a"]);
  });

  it("файл, созданный подагентом, виден панели файла и списку тронутых", () => {
    const feed = run(
      started("t1", "Task"),
      started("w1", "Write", "t1"),
      changed("w1", "notes.txt"),
      finished("w1"),
    );

    expect(top(feed)[0].children[0].fact?.path).toBe("notes.txt");
    expect(touchedFiles(feed)).toEqual(["notes.txt"]);
    expect(factOf(feed, "notes.txt")?.kind).toBe("created");
    expect(diffOf({ ...feed, diffs: {} }, "notes.txt")).toBeTruthy();
  });

  it("остановка хода гасит и его незавершённые вызовы", () => {
    const feed = run(
      started("t1", "Task"),
      started("r1", "Read", "t1"),
      { type: "turnCancelled", turnId: "t1", at: 0 },
    );

    // Живая точка на вызове, который уже никто не выполняет, — враньё (§0).
    expect(flatRows(top(feed)).map((r) => r.state)).toEqual(["failed", "failed"]);
  });

  it("вызов без своего Task в ленте не теряется", () => {
    const feed = run(started("r1", "Read", "t-нет"));
    expect(top(feed).map((r) => r.callId)).toEqual(["r1"]);
  });
});

describe("вопросы AskUser", () => {
  const request = {
    id: "q_1",
    callId: "c1",
    question: "Какой путь?",
    options: [
      { label: "Быстрый", description: "без тестов" },
      { label: "Полный", description: null },
    ],
    allowOther: true,
    multiSelect: false,
  };

  const asked = { type: "questionAsked" as const, request };
  const resolved = (reply: QuestionReply | null) => ({
    type: "questionResolved" as const,
    requestId: "q_1",
    reply,
  });

  function question(feed: Feed) {
    const block = feed.blocks.find((b) => b.kind === "question");
    if (!block || block.kind !== "question") throw new Error("нет блока вопроса");
    return block;
  }

  it("вопрос встаёт в ленту открытым и занимает нижний ряд", () => {
    const feed = run(asked);

    expect(feed.question?.id).toBe("q_1");
    expect(question(feed).status).toBe("open");
    expect(question(feed).reply).toBeNull();
    expect(visible(feed)).toHaveLength(1);
  });

  it("ответ закрывает вопрос, но строка с выбором остаётся в ленте", () => {
    const feed = run(asked, resolved({ type: "answered", selected: [1], other: null }));

    expect(feed.question).toBeNull();
    expect(question(feed).status).toBe("answered");
    expect(question(feed).reply).toEqual({ type: "answered", selected: [1], other: null });
    // Блоки только добавляются: итог ответа — часть разговора.
    expect(visible(feed)).toHaveLength(1);
  });

  it("отказ от ответа помечается отдельно от ответа", () => {
    const feed = run(asked, resolved({ type: "dismissed" }));

    expect(feed.question).toBeNull();
    expect(question(feed).status).toBe("dismissed");
    expect(question(feed).reply).toEqual({ type: "dismissed" });
  });

  it("ответ на вопрос, которого нет на экране, ничего не трогает", () => {
    const feed = run(asked, {
      type: "questionResolved",
      requestId: "другой",
      reply: { type: "dismissed" },
    });

    expect(feed.question?.id).toBe("q_1");
    expect(question(feed).status).toBe("open");
  });

  it("вопрос без ответа из журнала закрыт и не висит открытым", () => {
    // Так выглядит повтор незавершённого прогона: ядро закрывает вопрос
    // без ответа, и лента не рисует кнопки, которые уже никто не слушает.
    const feed = run(asked, resolved(null));

    expect(feed.question).toBeNull();
    expect(question(feed).status).toBe("unanswered");
    expect(question(feed).reply).toBeNull();
  });

  it("выбор сохраняет свой текст рядом с отмеченными вариантами", () => {
    const feed = run(
      asked,
      resolved({ type: "answered", selected: [0], other: "оба сразу" }),
    );

    expect(question(feed).reply).toEqual({
      type: "answered",
      selected: [0],
      other: "оба сразу",
    });
  });
});

describe("панель файла (§6.4)", () => {
  const pending = (path: string, hunks: number) => ({
    fact: { path, kind: "modified" as const, movedFrom: null, added: hunks, removed: 0 },
  });

  it("каждая запись файла двигает счётчик, по которому панель перечитывает вид", () => {
    const change = (callId: string, path: string) => ({
      type: "fileChanged" as const,
      callId,
      fact: pending(path, 1).fact,
      diff: { hunks: [], added: 1, removed: 0, truncated: false },
    });
    const feed = run(change("c1", "a.txt"), change("c2", "a.txt"));
    expect(feed.writes).toBe(2);
  });

  it("вид файла держит последний ответ ядра по пути", () => {
    const view = {
      fact: pending("a.txt", 1).fact,
      segments: [],
      truncated: false,
    };
    const feed = run({ type: "fileViewLoaded", path: "a.txt", view });
    expect(feed.fileView).toEqual({ path: "a.txt", view });
  });

  it("«нечего решать» приходит как null, а не как отсутствие ответа", () => {
    const feed = run({ type: "fileViewLoaded", path: "a.txt", view: null });
    expect(feed.fileView).toEqual({ path: "a.txt", view: null });
  });

  it("новый вид перекрывает прежний, даже для другого файла", () => {
    const view = { fact: pending("b.txt", 1).fact, segments: [], truncated: false };
    const feed = run(
      { type: "fileViewLoaded", path: "a.txt", view: null },
      { type: "fileViewLoaded", path: "b.txt", view },
    );
    expect(feed.fileView?.path).toBe("b.txt");
  });
});

describe("очередь и поиск файлов (§7.4, §7.2)", () => {
  const waiting = (id: string, text: string): QueuedMessage => ({
    id,
    text,
    attachments: [],
  });

  it("shows exactly the list the core last sent, in its order", () => {
    const feed = run(
      { type: "queueUpdated", queued: [waiting("q1", "first"), waiting("q2", "second")] },
    );
    expect(feed.queued.map((m) => m.id)).toEqual(["q1", "q2"]);
  });

  it("drops a message the core no longer lists, and the empty list clears all", () => {
    const after = run(
      { type: "queueUpdated", queued: [waiting("q1", "a"), waiting("q2", "b")] },
      { type: "queueUpdated", queued: [waiting("q2", "b")] },
    );
    expect(after.queued.map((m) => m.id)).toEqual(["q2"]);

    const empty = reduceAll(after, [{ type: "queueUpdated", queued: [] }]);
    expect(empty.queued).toEqual([]);
  });

  it("takes the message into the conversation without touching the queue itself", () => {
    const feed = run(
      { type: "queueUpdated", queued: [waiting("q1", "later")] },
      { type: "queueUpdated", queued: [] },
      said("later"),
    );
    expect(feed.queued).toEqual([]);
    expect(visible(feed).some((b) => b.kind === "human" && b.text === "later")).toBe(true);
  });

  it("keeps the answer to a search with the query it answers", () => {
    const feed = run({ type: "filesFound", query: "app", paths: ["src/app.rs"] });
    expect(feed.found).toEqual({ query: "app", paths: ["src/app.rs"] });
  });

  it("the last answer wins, whichever query it is for", () => {
    const feed = run(
      { type: "filesFound", query: "a", paths: ["a.txt"] },
      { type: "filesFound", query: "ab", paths: ["ab.txt"] },
    );
    expect(feed.found?.query).toBe("ab");
  });
});

describe("время хода", () => {
  const begin: AgentEvent = { type: "turnStarted", turnId: "t1", at: 1_000 };

  it("идущий ход несёт время начала, законченный — убирает его", () => {
    expect(run(begin).turn).toEqual({ startedAt: 1_000 });
    const done = run(begin, { type: "turnFinished", turnId: "t1", stopReason: "endTurn", at: 5_000 });
    expect(done.turn).toBeNull();
  });

  it("под ответом встаёт строка с длительностью и временем конца", () => {
    const feed = run(begin, wrote("b1", "готово"), {
      type: "turnFinished",
      turnId: "t1",
      stopReason: "endTurn",
      at: 134_000,
    });
    const last = feed.blocks[feed.blocks.length - 1];
    expect(last).toMatchObject({ kind: "turnTime", startedAt: 1_000, endedAt: 134_000 });
  });

  it("остановленный ход тоже показывает, сколько он шёл", () => {
    const feed = run(begin, { type: "turnCancelled", turnId: "t1", at: 4_000 });
    expect(feed.blocks.map((b) => b.kind)).toEqual(["stopped", "turnTime"]);
  });

  it("без времени конца (журнал старой сборки) строки нет", () => {
    const feed = run(begin, { type: "turnFinished", turnId: "t1", stopReason: "endTurn", at: 0 });
    expect(feed.blocks.some((b) => b.kind === "turnTime")).toBe(false);
  });
});

describe("показанный текст файла", () => {
  const shown = (text: string): AgentEvent => ({
    type: "textShown",
    path: "notes.md",
    firstLine: 2,
    lines: 2,
    total: 5,
    text,
  });

  it("встаёт своим блоком с путём и строками", () => {
    const feed = run(shown("two\nthree"));
    expect(feed.blocks).toMatchObject([
      { kind: "shown", path: "notes.md", firstLine: 2, lines: 2, total: 5, text: "two\nthree" },
    ]);
  });

  it("не сливается с текстом агента вокруг", () => {
    const feed = run(wrote("b1", "Вот файл:"), shown("two"), wrote("b1", "Это всё."));
    expect(feed.blocks.map((b) => b.kind)).toEqual(["agent", "shown", "agent"]);
    expect(feed.blocks[2]).toMatchObject({ text: "Это всё." });
  });
});
