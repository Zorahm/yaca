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
 * врёт про состояние (§0).
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

/** Сценарий: всё, что §5 умеет рисовать, по одному разу. */
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
  { type: "turnStarted", turnId: "t1", at: NOW },
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
    chunk: "export function parseSegment(raw: string) {\n  return new Date(raw);\n}\n",
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
    text: "Поправил. Прогоню тесты.\n",
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
      costUsd: 0.31,
    },
  },
];
