/**
 * Записанный поток событий для окна без оболочки.
 *
 * Существует по той же причине, что `MockProvider` в ядре: интерфейс обещан
 * чистой функцией от потока `AgentEvent`, и проверить это должно быть можно без
 * Tauri, без ключа и без денег. `npm run dev` открывает окно в браузере и
 * проигрывает вот это.
 *
 * Включается только когда Tauri рядом нет. В настоящем окне не участвует
 * ничем: подмена живых событий записанными — это ровно тот интерфейс, который
 * врёт про состояние.
 */

import type { AgentEvent } from "./bindings/AgentEvent";
import type { McpServerSummary } from "./bindings/McpServerSummary";
import type { ModelInfo } from "./bindings/ModelInfo";
import type { SessionState } from "./bindings/SessionState";

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const demoState: SessionState = {
  projectRoot: "D:/Kod",
  provider: "anthropic",
  model: "claude-opus-5",
  mode: "manual",
  effort: "extra",
  busy: false,
  bypass: false,
};

/**
 * Серверы MCP стенда: по одному на каждое состояние, которое панель `/mcp`
 * умеет рисовать с кнопкой, — готовый, ждущий одобрения и упавший.
 */
export const demoMcp: McpServerSummary[] = [
  {
    name: "docs",
    source: "global",
    transport: "http",
    target: "https://docs.example.test/mcp",
    status: "ready",
    reason: null,
    label: "docs-server 1.4.0",
    tools: [
      { name: "search", description: "Search the documentation.", readOnly: true },
      { name: "add_note", description: "Attach a note to a page.", readOnly: false },
    ],
    resources: true,
  },
  {
    name: "files",
    source: "projectFile",
    transport: "stdio",
    target: "npx -y @example/server-files .",
    status: "needsApproval",
    reason: null,
    label: null,
    tools: [],
    resources: false,
  },
  {
    name: "tracker",
    source: "project",
    transport: "stdio",
    target: "tracker-mcp --workspace yaca",
    status: "failed",
    reason: "процесс сервера завершился с кодом 1:\nerror: TRACKER_TOKEN is not set",
    label: null,
    tools: [],
    resources: false,
  },
];

export const demoModels: ModelInfo[] = [
  {
    id: "claude-opus-5",
    providerId: "anthropic",
    displayName: "Opus 5",
    contextMax: 1_000_000,
    maxOutput: 128_000,
    pricing: { input: 5, output: 25 },
    supportsEffort: true,
    supportsThinking: true,
    contextSource: "provider_reported",
    supportsImages: true,
  },
  {
    id: "claude-sonnet-5",
    providerId: "anthropic",
    displayName: "Sonnet 5",
    contextMax: 1_000_000,
    maxOutput: 128_000,
    pricing: { input: 2, output: 10 },
    supportsEffort: true,
    supportsThinking: true,
    contextSource: "provider_reported",
    supportsImages: true,
  },
];

const NOW = 1_735_732_320_000;

function state(patch: Partial<SessionState>): AgentEvent {
  return { type: "stateChanged", state: { ...demoState, ...patch } };
}

function call(
  callId: string,
  tool: string,
  target: string,
  description: string | null,
): AgentEvent {
  return {
    type: "toolCallStarted",
    turnId: "t1",
    callId,
    tool,
    description,
    target,
    input: {},
    parent: null,
  };
}

function diff(added: string[], removed: string[]) {
  return {
    hunks: [
      {
        oldStart: 12,
        oldLines: removed.length + 2,
        newStart: 12,
        newLines: added.length + 2,
        lines: [
          {
            kind: "context" as const,
            oldLine: 12,
            newLine: 12,
            text: "export function parseSegment(raw: string) {",
            spans: [],
          },
          ...removed.map((text, i) => ({
            kind: "del" as const,
            oldLine: 13 + i,
            newLine: null,
            text,
            spans: [{ start: 9, end: 21 }],
          })),
          ...added.map((text, i) => ({
            kind: "add" as const,
            oldLine: null,
            newLine: 13 + i,
            text,
            spans: [{ start: 9, end: 25 }],
          })),
          {
            kind: "context" as const,
            oldLine: 15,
            newLine: 16,
            text: "}",
            spans: [],
          },
        ],
      },
    ],
    added: added.length,
    removed: removed.length,
    truncated: false,
  };
}

/** Сценарий: всё, что лента умеет рисовать, по одному разу. */
export const demoScript: AgentEvent[] = [
  state({}),
  {
    type: "checkpointCreated",
    checkpoint: {
      id: "ckpt_1",
      at: NOW,
      reason: { type: "message" },
      tracked: 148,
    },
  },
  {
    type: "checkpointsUpdated",
    checkpoints: [
      { id: "ckpt_1", at: NOW, reason: { type: "message" }, tracked: 148 },
    ],
  },
  {
    type: "userMessage",
    turnId: "t1",
    text: "почини разбор дат в сегментах — падает на границе часового пояса",
    attachments: ["src/segments.ts"],
  },
  // The stand has no core behind it, so the turn starts when the window does:
  // the live timer counts from a real moment, not from the fixed NOW.
  { type: "turnStarted", turnId: "t1", at: Date.now() },
  state({ busy: true }),
  {
    type: "thinkingDelta",
    turnId: "t1",
    blockId: "b0",
    text: "Похоже, дата парсится в UTC, а сравнивается в локальной зоне.",
  },
  {
    type: "textDelta",
    turnId: "t1",
    blockId: "b1",
    text: "Смотрю на `parseSegment` — скорее всего дело в том, что дата\nразбирается в UTC, а сравнивается уже локальной.\n\n",
  },
  call("c1", "Read", "src/segments.ts", "Прочитал разбор сегментов"),
  {
    type: "toolCallProgress",
    callId: "c1",
    // The listing as `Read` writes it: a right-aligned number, a tab, the line.
    chunk:
      "     1\texport function parseSegment(raw: string) {\n     2\t  return new Date(raw);\n     3\t}\n",
  },
  {
    type: "toolCallFinished",
    callId: "c1",
    meta: { lines: 3, items: null, truncated: false, durationMs: 8 },
  },
  call("c2", "Grep", "rg parseSegment src/", "Нашёл три места, где это зовут"),
  {
    type: "toolCallFinished",
    callId: "c2",
    meta: { lines: 3, items: 3, truncated: false, durationMs: 41 },
  },
  // Three subagents sent together, one in each state the plate and the
  // agents panel draw: reported, still at work, and ended without a report.
  {
    type: "toolCallStarted",
    turnId: "t1",
    callId: "a1",
    tool: "Task",
    description: "Выяснил, где ещё разбираются даты",
    target: null,
    input: {
      prompt:
        "Найди все места в src/, где строка превращается в дату: Date.parse, new Date(строка), свои разборщики.\n\nПо каждому месту скажи, в какой зоне получается результат — UTC или локальной. Ничего не меняй.\n\nВ отчёте: список path:line и одна строка о каждом.",
    },
    parent: null,
  },
  {
    type: "toolCallStarted",
    turnId: "t1",
    callId: "a2",
    tool: "Task",
    description: "Проверяет, как зону считают тесты",
    target: null,
    input: { prompt: "Посмотри тесты сегментов: в какой зоне они запускаются и где она задаётся." },
    parent: null,
  },
  {
    type: "toolCallStarted",
    turnId: "t1",
    callId: "a3",
    tool: "Task",
    description: "Искал описание формата в документации",
    target: null,
    input: { prompt: "Найди в docs/ описание формата даты сегмента." },
    parent: null,
  },
  {
    type: "toolCallStarted",
    turnId: "t1",
    callId: "a1c1",
    tool: "Grep",
    description: "Нашёл разборы дат",
    target: "rg \"Date.parse|new Date\\(\" src/",
    input: {},
    parent: "a1",
  },
  {
    type: "toolCallFinished",
    callId: "a1c1",
    meta: { lines: 4, items: 4, truncated: false, durationMs: 37 },
  },
  {
    type: "toolCallStarted",
    turnId: "t1",
    callId: "a1c2",
    tool: "Read",
    description: null,
    target: "src/zone.ts",
    input: {},
    parent: "a1",
  },
  {
    type: "toolCallProgress",
    callId: "a1c2",
    chunk: "     1\texport function offset(raw: string): number {\n     2\t  return 0;\n     3\t}\n",
  },
  {
    type: "toolCallFinished",
    callId: "a1c2",
    meta: { lines: 3, items: null, truncated: false, durationMs: 6 },
  },
  {
    type: "subagentUsage",
    callId: "a1",
    tokens: { input: 18400, output: 1260, cacheWrite: 0, cacheRead: 5200 },
  },
  {
    type: "toolCallProgress",
    callId: "a1",
    chunk:
      "Даты разбираются в двух местах, оба дают UTC:\n\n- `src/segments.ts:13` — `Date.parse(raw)`\n- `src/zone.ts:1` — `offset` всегда возвращает 0\n\nСвоих разборщиков нет.",
  },
  {
    type: "toolCallFinished",
    callId: "a1",
    meta: { lines: 6, items: null, truncated: false, durationMs: 9400 },
  },
  {
    type: "toolCallStarted",
    turnId: "t1",
    callId: "a2c1",
    tool: "Read",
    description: null,
    target: "src/segments.test.ts",
    input: {},
    parent: "a2",
  },
  {
    type: "subagentUsage",
    callId: "a2",
    tokens: { input: 9100, output: 420, cacheWrite: 0, cacheRead: 0 },
  },
  {
    type: "subagentUsage",
    callId: "a3",
    tokens: { input: 3100, output: 90, cacheWrite: 0, cacheRead: 0 },
  },
  { type: "toolCallFailed", callId: "a3", message: "подагент ничего не ответил" },
  call("c3", "Edit", "src/segments.ts", null),
  {
    type: "fileChanged",
    callId: "c3",
    fact: {
      path: "src/segments.ts",
      kind: "modified",
      movedFrom: null,
      added: 2,
      removed: 1,
    },
    diff: diff(
      ["  const at = Date.parse(raw);", "  return new Date(at + offset(raw));"],
      ["  return new Date(raw);"],
    ),
  },
  {
    type: "toolCallFinished",
    callId: "c3",
    meta: { lines: 0, items: null, truncated: false, durationMs: 12 },
  },
  call("c3d", "DeleteDir", "src/legacy", null),
  {
    type: "toolCallProgress",
    callId: "c3d",
    chunk: "Удалён каталог src/legacy, файлов: 14\n",
  },
  {
    type: "toolCallFinished",
    callId: "c3d",
    meta: { lines: 1, items: 14, truncated: false, durationMs: 23 },
  },
  call("c4", "Edit", "src/app.rs", null),
  { type: "toolCallFailed", callId: "c4", message: "якорь встречается дважды" },
  call("c5", "Write", "src/segments.test.ts", null),
  {
    type: "toolCallRejected",
    callId: "c5",
    reason: "src/segments.test.ts не прочитан в этой сессии",
  },
  {
    type: "todosUpdated",
    items: [
      { content: "Найти, где дата теряет зону", status: "completed" },
      { content: "Починить parseSegment", status: "inProgress" },
      { content: "Прогнать тесты", status: "pending" },
    ],
  },
  {
    type: "textDelta",
    turnId: "t1",
    blockId: "b2",
    text:
      "## Что изменилось\n\nПоправил `parseSegment`: смещение зоны теперь **прибавляется** после разбора.\n\n" +
      "```ts\nconst at = Date.parse(raw);\nreturn new Date(at + offset(raw));\n```\n\n" +
      '```python\ndef count_lines(path: str) -> int:\n    with open(path, encoding="utf-8") as f:\n        return sum(1 for _ in f)\n```\n\n' +
      "- `segments.ts` — сам разбор\n- `app.rs` — правка не прошла\n\nПрогоню тесты.\n",
  },
  {
    type: "textShown",
    path: "src/segments.ts",
    firstLine: 12,
    lines: 4,
    total: 86,
    text:
      "export function parseSegment(raw: string): Date {\n" +
      "  const at = Date.parse(raw);\n" +
      "  return new Date(at + offset(raw));\n" +
      "}",
  },
  call("c6", "Bash", "npm test -- segments", "Прогнал тесты сегментов"),
  {
    type: "permissionRequested",
    request: {
      id: "req_1",
      callId: "c6",
      kind: "runCommand",
      target: "npm",
      detail: "npm test -- segments",
      note: "D:/Kod",
      sizeBytes: null,
      files: null,
      unsaved: null,
      invariant: null,
    server: null,
      scopes: [
        { type: "commandPrefix", prefix: "npm" },
        { type: "tool", name: "Bash" },
      ],
      position: 1,
      total: 1,
    },
  },
  {
    type: "usageUpdated",
    usage: {
      last: { input: 24_000, output: 800, cacheWrite: 0, cacheRead: 0 },
      total: { input: 24_000, output: 800, cacheWrite: 0, cacheRead: 0 },
      contextUsed: 24_800,
      contextMax: 1_000_000,
      breakdown: {
        systemPrompt: 2_100,
        instructions: 1_400,
        memory: 300,
        skills: 0,
        tools: 6_200,
        mcpTools: 0,
        mcpNotes: 0,
        messages: 14_800,
        reserved: 157_600,
      },
      costUsd: 0.31,
    },
  },
];

/**
 * The end of the recorded turn: what the core sends once the person has
 * answered the question the script stops on. Played by the stand on that
 * answer, so the finished state of a turn — the line with how long it took and
 * when it ended — can be looked at as well as the running one.
 */
export function demoFinish(): AgentEvent[] {
  return [
    { type: "permissionResolved", requestId: "req_1" },
    // The subagent left at work reports before the turn ends.
    {
      type: "toolCallFinished",
      callId: "a2c1",
      meta: { lines: 40, items: null, truncated: false, durationMs: 7 },
    },
    {
      type: "subagentUsage",
      callId: "a2",
      tokens: { input: 14800, output: 760, cacheWrite: 0, cacheRead: 9100 },
    },
    {
      type: "toolCallProgress",
      callId: "a2",
      chunk: "Тесты идут в UTC: зона задана в `vitest.config.ts:4` через `TZ`.",
    },
    {
      type: "toolCallFinished",
      callId: "a2",
      meta: { lines: 1, items: null, truncated: false, durationMs: 21000 },
    },
    { type: "toolCallProgress", callId: "c6", chunk: "Тесты: 12 из 12 прошли.\n" },
    {
      type: "toolCallFinished",
      callId: "c6",
      meta: { lines: 1, items: null, truncated: false, durationMs: 3_200 },
    },
    { type: "textDelta", turnId: "t1", blockId: "b3", text: "Тесты проходят: 12 из 12." },
    state({ busy: false }),
    { type: "turnFinished", turnId: "t1", stopReason: "endTurn", at: Date.now() },
  ];
}
