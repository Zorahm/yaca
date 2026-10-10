/**
 * Золотые тесты ленты — зеркало тех, что в ядре.
 *
 * Ядро проверяет, что поток событий той самой формы; здесь проверяется, что
 * лента есть функция от него. Вместе это и есть обещание §15, и ломается оно
 * тут, а не глазами.
 */

import { describe, expect, it } from "vitest";

import type { AgentEvent } from "./bindings/AgentEvent";
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

  it("реплика между действиями закрывает группу", () => {
    // §5.4: лента — разговор со вставленными строчками отчёта, а не одна
    // бесконечная лестница.
    const feed = run(started("c1"), wrote("b1", "теперь стили"), started("c2"));

    expect(feed.blocks.map((b) => b.kind)).toEqual(["actions", "agent", "actions"]);
  });

  it("рассуждение между действиями группу не закрывает", () => {
    // Модель, думающая перед каждым вызовом, иначе даёт лестницу групп по
    // одной строке. Ответом рассуждение не является (§5.3).
    const feed = run(
      started("c1"),
      thought("t1", "теперь тесты"),
      started("c2"),
      thought("t2", "и ещё раз"),
      started("c3"),
    );

    expect(feed.blocks.map((b) => b.kind)).toEqual([
      "actions",
      "reasoning",
      "reasoning",
    ]);
    if (feed.blocks[0].kind !== "actions") throw new Error("не группа");
    expect(feed.blocks[0].rows).toHaveLength(3);
  });

  it("рассуждение не склеивает группы через ответ", () => {
    // Ответ закрывает группу, и рассуждение после него открытую группу не
    // возвращает: иначе действия второго хода уехали бы в шапку первого.
    const feed = run(
      started("c1"),
      wrote("b1", "готово"),
      thought("t1", "дальше"),
      started("c2"),
    );

    expect(feed.blocks.map((b) => b.kind)).toEqual([
      "actions",
      "agent",
      "reasoning",
      "actions",
    ]);
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
      { type: "turnCancelled", turnId: "t1" },
    );

    // Живая точка на вызове, который уже никто не выполняет, — враньё (§0).
    expect(flatRows(top(feed)).map((r) => r.state)).toEqual(["failed", "failed"]);
  });

  it("вызов без своего Task в ленте не теряется", () => {
    const feed = run(started("r1", "Read", "t-нет"));
    expect(top(feed).map((r) => r.callId)).toEqual(["r1"]);
  });
});
