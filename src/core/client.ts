/**
 * Типизированный клиент ядра.
 *
 * Единственное место, где фронтенд знает про Tauri. Типы приходят из Rust
 * через ts-rs, поэтому разойтись с ядром они не могут: несовпадение ловит
 * компилятор, а не человек, читающий ленту.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";

import { demoModels, demoScript, demoState, inTauri } from "./demo";
import { OPENAI_PROVIDER_ID, isPreset, needsKey } from "./provider";

import type { AgentEvent } from "./bindings/AgentEvent";
import type { CheckpointChange } from "./bindings/CheckpointChange";
import type { CheckpointId } from "./bindings/CheckpointId";
import type { CheckpointSummary } from "./bindings/CheckpointSummary";
import type { Command } from "./bindings/Command";
import type { DiffLine } from "./bindings/DiffLine";
import type { EventFrame } from "./bindings/EventFrame";
import type { FileDiff } from "./bindings/FileDiff";
import type { FileFact } from "./bindings/FileFact";
import type { Effort } from "./bindings/Effort";
import type { Mode } from "./bindings/Mode";
import type { ModelInfo } from "./bindings/ModelInfo";
import type { OpenedChat } from "./bindings/OpenedChat";
import type { PermissionAnswer } from "./bindings/PermissionAnswer";
import type { PermissionKind } from "./bindings/PermissionKind";
import type { PermissionRequest } from "./bindings/PermissionRequest";
import type { ProcessId } from "./bindings/ProcessId";
import type { ProcessState } from "./bindings/ProcessState";
import type { ProcessSummary } from "./bindings/ProcessSummary";
import type { ScopeKey } from "./bindings/ScopeKey";
import type { SearchHit } from "./bindings/SearchHit";
import type { SessionId } from "./bindings/SessionId";
import type { SessionMeta } from "./bindings/SessionMeta";
import type { SessionState } from "./bindings/SessionState";
import type { SlashCommand } from "./bindings/SlashCommand";
import type { Span } from "./bindings/Span";
import type { Protocol } from "./bindings/Protocol";
import type { ProviderSpec } from "./bindings/ProviderSpec";
import type { Providers } from "./bindings/Providers";
import type { Startup } from "./bindings/Startup";
import type { TodoItem } from "./bindings/TodoItem";
import type { ToolMeta } from "./bindings/ToolMeta";
import type { UsageSnapshot } from "./bindings/UsageSnapshot";

export type {
  AgentEvent,
  CheckpointChange,
  CheckpointId,
  CheckpointSummary,
  Command,
  DiffLine,
  Effort,
  EventFrame,
  FileDiff,
  FileFact,
  Mode,
  ModelInfo,
  OpenedChat,
  PermissionAnswer,
  PermissionKind,
  PermissionRequest,
  ProcessId,
  ProcessState,
  ProcessSummary,
  Protocol,
  ProviderSpec,
  Providers,
  ScopeKey,
  SearchHit,
  SessionId,
  SessionMeta,
  SessionState,
  SlashCommand,
  Span,
  Startup,
  TodoItem,
  ToolMeta,
  UsageSnapshot,
};

/** Мост шлёт события пачками по кадру, а не по одному (§12). */
const EVENT_CHANNEL = "yaca://events";

/**
 * Окно, открытое без оболочки, проигрывает записанный поток (`core/demo`).
 *
 * Это стенд для глаз и ничего больше: в настоящем окне `inTauri` истинно, и ни
 * одна строчка ниже не выполняется. Подменять живые события записанными там,
 * где они живые, значило бы врать про состояние (§0).
 */
const DEMO_FRAME_MS = 90;

/** Куда стенд шлёт кадры. Ставится подпиской, запускается открытием проекта. */
let demoHandler: ((frame: EventFrame) => void) | null = null;

/**
 * Проигрыватели открытых чатов стенда: по одному на разговор.
 *
 * Запись одна, а открыть её можно сколько угодно раз — каждый чат играет свою
 * копию под своим именем. Один проигрыватель на всех означал бы, что окно с
 * двумя разговорами в браузере не посмотреть, а именно это стенд и проверяет.
 */
const demoTimers = new Map<SessionId, number>();
let demoNext = 1;

/**
 * Проекты, которые знает окно (§9.1), свежий первым.
 *
 * Список лежит в глобальных настройках приложения: в самом проекте его
 * хранить негде, он нужен раньше, чем известен хоть один проект.
 *
 * В стенде список живёт в памяти процесса и отвечает на правки так же, как
 * ответила бы оболочка, — иначе добавленный проект выглядел бы добавленным,
 * а список оставался прежним (§0).
 */
let demoProjects: string[] = [demoState.projectRoot];

export function projects(): Promise<string[]> {
  return inTauri ? invoke<string[]>("projects") : Promise.resolve([...demoProjects]);
}

/** Добавляет проект в список, не начиная чата (§9.1). Отдаёт список целиком. */
export function addProject(path: string): Promise<string[]> {
  if (!inTauri) {
    demoProjects = [path, ...demoProjects.filter((known) => known !== path)];
    return Promise.resolve([...demoProjects]);
  }
  return invoke<string[]>("add_project", { path });
}

/** Убирает проект из списка. Папку и журналы §9 не трогает. */
export function forgetProject(path: string): Promise<string[]> {
  if (!inTauri) {
    demoProjects = demoProjects.filter((known) => known !== path);
    return Promise.resolve([...demoProjects]);
  }
  return invoke<string[]>("forget_project", { path });
}

/**
 * Есть ли кому открыть диалог файлов — выбора папки, файлов или сохранения.
 *
 * В стенде некому: диалоги принадлежат оболочке. Чип проекта и `＋` вложений
 * спрашивают об этом и гасятся, а не притворяются кнопками, за которыми
 * ничего нет (§0).
 */
export const hasFileDialogs = inTauri;

/** Диалог выбора папки для чипа проекта (§9.1). `null` — человек передумал. */
export function chooseFolder(title: string): Promise<string | null> {
  if (!hasFileDialogs) return Promise.resolve(null);
  return open({ directory: true, multiple: false, title }).then((picked) =>
    typeof picked === "string" ? picked : null,
  );
}

/**
 * Диалог сохранения файла (§9). `null` — человек передумал.
 *
 * Путь возвращается наружу, а не пишется здесь: писать умеет ядро, у которого
 * и лежит то, что сохраняют. Вебвью выбирает место, а не носит содержимое
 * туда и обратно.
 */
export function chooseSavePath(
  defaultName: string,
  title: string,
  extension: string,
): Promise<string | null> {
  if (!hasFileDialogs) return Promise.resolve(null);
  return save({
    defaultPath: defaultName,
    title,
    filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
  });
}

/**
 * Диалог выбора файлов для `＋` (§7.2). Пустой список — человек передумал.
 *
 * Начинает с корня проекта: прикладывают почти всегда его файлы, и диалог,
 * открывающийся в домашнем каталоге, заставляет идти туда самому.
 */
export function chooseFiles(root: string, title: string): Promise<string[]> {
  if (!hasFileDialogs) return Promise.resolve([]);
  return open({ directory: false, multiple: true, defaultPath: root, title }).then(
    (picked) => (Array.isArray(picked) ? picked : picked === null ? [] : [picked]),
  );
}

/** Имя для приветствия §9.1. Взято у ОС; не нашлось — обращения не будет. */
export function userName(): Promise<string | null> {
  return inTauri ? invoke<string | null>("user_name") : Promise.resolve(null);
}

/**
 * С чем сессию начал бы сам проект (§10).
 *
 * Композер домашнего экрана показывает это до первого слова, иначе он молча
 * затирал бы `.yaca/settings.json` встроенными умолчаниями.
 */
export function projectStartup(path: string): Promise<Startup> {
  if (!inTauri) {
    const { mode, provider, model, effort } = demoState;
    return Promise.resolve({ mode, provider, model, effort });
  }
  return invoke<Startup>("project_startup", { path });
}

/**
 * `resume` продолжает записанную сессию (§9), а не начинает новую.
 *
 * `startup` — то, что композер домашнего экрана выбрал до первого слова.
 * Без него сессия берёт настройки проекта (§10).
 */
export function openProject(
  path: string,
  resume?: SessionId,
  startup?: Startup,
): Promise<OpenedChat> {
  if (!inTauri) {
    // Поток начинается вместе с чатом, а не с подпиской: окно чистит ленту,
    // когда чат открывают, и записанное до этого было бы стёрто.
    return Promise.resolve({
      id: startDemo(path),
      state: { ...demoState, projectRoot: path },
    });
  }
  return invoke<OpenedChat>("open_project", { path, resume, startup });
}

/**
 * Закрывает чат: ход останавливается, фоновые команды умирают вместе с
 * сессией (§6.5), журнал §9 остаётся. Закрытый чат открывается заново со
 * своей историей — закрыть не значит удалить.
 */
export function closeChat(session: SessionId): Promise<void> {
  if (!inTauri) {
    stopDemo(session);
    return Promise.resolve();
  }
  return invoke<void>("close_chat", { session });
}

/**
 * Закрывает всё, что осталось в оболочке от прошлой жизни окна.
 *
 * Вебвью перезагружается, а сессии переживают перезагрузку: ленты у них уже
 * нет, а агент продолжает работать. Окно зовёт это на старте — невидимый
 * агент хуже прерванного.
 */
export function closeAllChats(): Promise<void> {
  if (!inTauri) {
    stopAllDemos();
    return Promise.resolve();
  }
  return invoke<void>("close_all_chats");
}

/**
 * Список сессий §9. Работает с проектом, а не с открытым чатом: журналы лежат
 * и у проекта, в котором сейчас никто не разговаривает, а чатов в одном
 * проекте может быть открыто несколько.
 *
 * В стенде записанная сессия нигде не лежит — пустой список и есть его правда,
 * как `sessionMeta() → null` ниже.
 */
export const sessions = {
  list: (project: string): Promise<SessionMeta[]> =>
    inTauri ? invoke<SessionMeta[]>("list_sessions", { project }) : Promise.resolve([]),
  search: (project: string, query: string, limit?: number): Promise<SearchHit[]> =>
    inTauri
      ? invoke<SearchHit[]>("search_sessions", { project, query, limit })
      : Promise.resolve([]),
  rename: (project: string, id: SessionId, title: string): Promise<void> =>
    inTauri
      ? invoke<void>("rename_session", { project, id, title })
      : Promise.resolve(),
  remove: (project: string, id: SessionId): Promise<void> =>
    inTauri ? invoke<void>("delete_session", { project, id }) : Promise.resolve(),
  duplicate: (project: string, id: SessionId, through?: string): Promise<SessionMeta> =>
    inTauri
      ? invoke<SessionMeta>("duplicate_session", { project, id, through })
      : Promise.reject("в стенде сессий нет"),
  /** Экспорт §9 пишется ядром по выбранному окном пути. */
  exportTo: (project: string, id: SessionId, path: string): Promise<void> =>
    inTauri
      ? invoke<void>("export_session", { project, id, path })
      : Promise.reject("в стенде сессий нет"),
  /**
   * Журнал ленты §9 — им перерисовывается лента открытой заново сессии.
   *
   * Ядро отдаёт его уже подчищенным: процессы прошлого прогона, его состояние
   * и чекпоинты, к которым новый прогон вернуться не может, в поток не
   * попадают (`yaca_core::store::replay`). Интерфейсу остаётся то же, что и
   * всегда, — сложить поток в ленту.
   */
  feed: (project: string, id: SessionId): Promise<AgentEvent[]> =>
    inTauri ? invoke<AgentEvent[]>("session_feed", { project, id }) : Promise.resolve([]),
};

/**
 * Слэш-команды проекта §7.3 — то, чем меню композера дополняет ввод.
 *
 * По проекту, а не по чату: файлы лежат в `.yaca/commands`, а чатов в одном
 * проекте открыто может быть несколько. Разворачивает их ядро при отправке —
 * окну список нужен только чтобы нарисовать меню.
 *
 * В стенде их нет: команды живут файлами проекта, а проекта у стенда нет.
 */
export function projectCommands(project: string): Promise<SlashCommand[]> {
  return inTauri
    ? invoke<SlashCommand[]>("project_commands", { project })
    : Promise.resolve([]);
}

/**
 * Провайдеры §10: список и правка.
 *
 * В стенде реестр живёт в памяти процесса и повторяет семантику ядра
 * (`Providers::put`/`remove`): замена по id, отказ на удалении
 * предустановленного. Иначе правка выглядела бы сработавшей, а список
 * оставался прежним — интерфейс, врущий про состояние (§0). Кейчейна у стенда
 * нет, и ключи он не хранит.
 */
let demoRegistry: Providers = {
  list: [
    {
      id: "anthropic",
      displayName: "Anthropic",
      protocol: "anthropic",
      baseUrl: "https://api.anthropic.com",
      headers: {},
      models: demoModels,
    },
    {
      // Второй предустановленный (`registry::ProviderSpec::openai`). Без
      // моделей, как и в ядре: их отдаёт `/v1/models`, а в стенде спросить
      // некого — панель честно скажет, что моделей нет.
      id: OPENAI_PROVIDER_ID,
      displayName: "OpenAI",
      protocol: "openai",
      baseUrl: "https://api.openai.com",
      headers: {},
      models: [],
    },
  ],
};

/** Копия наружу: панель получает состояние на момент вызова, а не живую ссылку внутрь стенда. */
function demoProviders(): Providers {
  return JSON.parse(JSON.stringify(demoRegistry)) as Providers;
}

function demoHeaderPairs(spec: ProviderSpec): Array<[string, string]> {
  return Object.entries(spec.headers).filter(
    (entry): entry is [string, string] => typeof entry[1] === "string",
  );
}

export const providers = {
  list: (): Promise<Providers> =>
    inTauri ? invoke<Providers>("providers") : Promise.resolve(demoProviders()),
  save: (spec: ProviderSpec): Promise<Providers> => {
    if (!inTauri) {
      const at = demoRegistry.list.findIndex((p) => p.id === spec.id);
      if (at === -1) demoRegistry.list.push(spec);
      else demoRegistry.list[at] = spec;
      return Promise.resolve(demoProviders());
    }
    return invoke<Providers>("save_provider", { spec });
  },
  remove: (id: string): Promise<Providers> => {
    if (!inTauri) {
      if (isPreset(id)) {
        return Promise.reject("предустановленный провайдер не удаляется");
      }
      demoRegistry.list = demoRegistry.list.filter((p) => p.id !== id);
      return Promise.resolve(demoProviders());
    }
    return invoke<Providers>("delete_provider", { id });
  },
};

/** Команда адресована чату: их несколько, и безымянная не знает, кому она. */
/**
 * Спросить у провайдера, какие модели он обслуживает (§10).
 *
 * Описание уезжает из формы, а не берётся из реестра: адрес и заголовки могли
 * только что поправить, и спрашивать надо у того, кого настраивают. Ключ
 * оболочка берёт в кейчейне сама — сюда он не попадает ни в каком виде.
 *
 * В стенде спрашивать некого: реестр живёт в памяти страницы, и провайдера за
 * ним нет. Пустой список был бы ответом «моделей нет», а это неправда, —
 * поэтому отказ.
 */
export function discoverModels(spec: ProviderSpec): Promise<ModelInfo[]> {
  if (!inTauri) return Promise.reject("в стенде провайдера за реестром нет");
  return invoke<ModelInfo[]>("discover_models", { spec });
}

export function sendCommand(session: SessionId, command: Command): Promise<void> {
  if (!inTauri) {
    console.info("[стенд] команда", command);
    return Promise.resolve();
  }
  return invoke<void>("send_command", { session, command });
}

export function sessionState(session: SessionId): Promise<SessionState | null> {
  if (!inTauri) return Promise.resolve(null);
  return invoke<SessionState | null>("session_state", { session });
}

/** Строка чата в списке §9: заголовок, время, стоимость. */
export function sessionMeta(session: SessionId): Promise<SessionMeta | null> {
  if (!inTauri) return Promise.resolve(null);
  return invoke<SessionMeta | null>("session_meta", { session });
}

/**
 * §10: ключ уезжает в системный кейчейн. Обратно он не возвращается.
 *
 * Лежит под именем провайдера, и провайдер называется всегда: угаданный
 * записал бы ключ не под тем именем.
 */
export function setApiKey(key: string, provider: string): Promise<void> {
  if (!inTauri) return Promise.reject("в стенде кейчейна нет");
  return invoke<void>("set_api_key", { key, provider });
}

export function forgetApiKey(provider: string): Promise<void> {
  if (!inTauri) return Promise.reject("в стенде кейчейна нет");
  return invoke<void>("forget_api_key", { provider });
}

/**
 * Откуда взят ключ — но не сам ключ.
 *
 * `notNeeded` — провайдер, который ключа не просит: локальный шлюз. «Ключа
 * нет» было бы предупреждением ни о чём.
 */
export type KeySource = "keychain" | "environment" | "none" | "not_needed";

export function apiKeySource(provider: string): Promise<KeySource> {
  if (!inTauri) {
    const spec = demoRegistry.list.find((p) => p.id === provider);
    if (!spec) return Promise.reject(`провайдера «${provider}» нет в списке`);
    // У стенда нет ни кейчейна, ни переменных окружения, и врать про
    // «environment» нечестно: честные ответы два — «нет» и «не нужен».
    return Promise.resolve(
      needsKey(spec.protocol, demoHeaderPairs(spec)) ? "none" : "not_needed",
    );
  }
  return invoke<KeySource>("api_key_source", { provider });
}

export function models(): Promise<ModelInfo[]> {
  if (!inTauri) return Promise.resolve(demoModels);
  return invoke<ModelInfo[]>("models");
}

/**
 * Which of a provider's models are in memory right now (§7.1), for a server
 * that says. The stand has no server behind its providers, and an empty map
 * claims nothing.
 */
export function modelResidency(provider: string): Promise<Record<string, boolean>> {
  if (!inTauri) return Promise.resolve({});
  return invoke<Record<string, boolean>>("model_residency", { provider });
}

/**
 * Подписка на поток событий. Обработчик получает кадр целиком: применять его
 * надо одной перерисовкой, иначе экономия на IPC уходит в рендер.
 *
 * Канал один на все чаты, и кадр приезжает с именем сессии: подписка на чат
 * означала бы отписку на каждое закрытие — лишний повод потерять события
 * того, кто работает в фоне.
 */
export function onEvents(handler: (frame: EventFrame) => void): Promise<UnlistenFn> {
  if (!inTauri) {
    demoHandler = handler;
    return Promise.resolve(() => {
      // Только своего: React в разработке подписывается дважды, и отписка
      // первой подписки приходит уже после второй.
      if (demoHandler !== handler) return;
      demoHandler = null;
      stopAllDemos();
    });
  }
  return listen<EventFrame>(EVENT_CHANNEL, (e) => handler(e.payload));
}

/** Заводит проигрыватель записи и отдаёт имя чата, которым тот назвался. */
function startDemo(path: string): SessionId {
  const session: SessionId = `ses_demo${demoNext}`;
  demoNext += 1;
  let at = 0;
  const timer = window.setInterval(() => {
    if (at >= demoScript.length || demoHandler === null) {
      stopDemo(session);
      return;
    }
    const event = demoScript[at];
    demoHandler({
      session,
      events: [
        event.type === "stateChanged"
          ? { ...event, state: { ...event.state, projectRoot: path } }
          : event,
      ],
    });
    at += 1;
  }, DEMO_FRAME_MS);
  demoTimers.set(session, timer);
  return session;
}

function stopDemo(session: SessionId): void {
  const timer = demoTimers.get(session);
  if (timer === undefined) return;
  window.clearInterval(timer);
  demoTimers.delete(session);
}

function stopAllDemos(): void {
  for (const timer of demoTimers.values()) window.clearInterval(timer);
  demoTimers.clear();
}

/** Удобные обёртки над командами §15. */
export const commands = {
  send: (text: string, attachments: string[] = []): Command => ({
    type: "sendMessage",
    text,
    attachments,
  }),
  cancel: (): Command => ({ type: "cancel" }),
  setMode: (mode: Mode): Command => ({ type: "setMode", mode }),
  setModel: (model: string): Command => ({ type: "setModel", model }),
  setEffort: (effort: Effort): Command => ({ type: "setEffort", effort }),
  answerPermission: (requestId: string, answer: PermissionAnswer): Command => ({
    type: "answerPermission",
    requestId,
    answer,
  }),
  setBypass: (on: boolean): Command => ({ type: "setBypass", on }),
  watchProcess: (id: ProcessId | null): Command => ({
    type: "watchProcess",
    id,
  }),
  stopProcess: (id: ProcessId): Command => ({ type: "stopProcess", id }),
  previewCheckpoint: (id: CheckpointId): Command => ({
    type: "previewCheckpoint",
    id,
  }),
  restoreCheckpoint: (id: CheckpointId): Command => ({
    type: "restoreCheckpoint",
    id,
  }),
  compact: (): Command => ({ type: "compact" }),
  editSummary: (text: string): Command => ({ type: "editSummary", text }),
};
