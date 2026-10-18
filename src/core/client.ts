/**
 * Типизированный клиент ядра.
 *
 * Единственное место, где фронтенд знает про Tauri. Типы приходят из Rust
 * через ts-rs, поэтому разойтись с ядром они не могут: несовпадение ловит
 * компилятор, а не человек, читающий ленту.
 */

import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";

import { demoFinish, demoLsp, demoMcp, demoModels, demoScript, demoState, inTauri } from "./demo";
import { DEMO_FILE_PATH, demoFileView } from "./demoFile";
import { demoSessions } from "./demoSessions";
import { BUILTIN_PROVIDER_ID, PRESETS, isPreset, needsKey } from "./provider";

import type { AgentEvent } from "./bindings/AgentEvent";
import type { AttachedChat } from "./bindings/AttachedChat";
import type { AttachmentLook } from "./bindings/AttachmentLook";
import type { CheckpointChange } from "./bindings/CheckpointChange";
import type { CheckpointId } from "./bindings/CheckpointId";
import type { CheckpointSummary } from "./bindings/CheckpointSummary";
import type { Command } from "./bindings/Command";
import type { DiffLine } from "./bindings/DiffLine";
import type { EventFrame } from "./bindings/EventFrame";
import type { FileDiff } from "./bindings/FileDiff";
import type { Hunk } from "./bindings/Hunk";
import type { ViewSegment } from "./bindings/ViewSegment";
import type { FileFact } from "./bindings/FileFact";
import type { FileView } from "./bindings/FileView";
import type { Effort } from "./bindings/Effort";
import type { Mode } from "./bindings/Mode";
import type { McpServerSettings } from "./bindings/McpServerSettings";
import type { McpServerSummary } from "./bindings/McpServerSummary";
import type { McpSource } from "./bindings/McpSource";
import type { McpStatus } from "./bindings/McpStatus";
import type { LspServerSummary } from "./bindings/LspServerSummary";
import type { ElicitField } from "./bindings/ElicitField";
import type { ElicitKind } from "./bindings/ElicitKind";
import type { ElicitationReply } from "./bindings/ElicitationReply";
import type { ElicitationRequest } from "./bindings/ElicitationRequest";
import type { LspStatus } from "./bindings/LspStatus";
import type { ModelInfo } from "./bindings/ModelInfo";
import type { OpenedChat } from "./bindings/OpenedChat";
import type { PermissionAnswer } from "./bindings/PermissionAnswer";
import type { PermissionKind } from "./bindings/PermissionKind";
import type { PermissionRequest } from "./bindings/PermissionRequest";
import type { Invariant } from "./bindings/Invariant";
import type { Unsaved } from "./bindings/Unsaved";
import type { QuestionOption } from "./bindings/QuestionOption";
import type { QueuedId } from "./bindings/QueuedId";
import type { QuestionReply } from "./bindings/QuestionReply";
import type { QuestionRequest } from "./bindings/QuestionRequest";
import type { ProcessId } from "./bindings/ProcessId";
import type { ProcessState } from "./bindings/ProcessState";
import type { ProcessSummary } from "./bindings/ProcessSummary";
import type { ScopeKey } from "./bindings/ScopeKey";
import type { SearchHit } from "./bindings/SearchHit";
import type { SessionId } from "./bindings/SessionId";
import type { BrowserState } from "./bindings/BrowserState";
import type { Rect } from "./bindings/Rect";
import type { ViewportSource } from "./bindings/ViewportSource";
import type { SessionMeta } from "./bindings/SessionMeta";
import type { SessionState } from "./bindings/SessionState";
import type { ShellChoice } from "./bindings/ShellChoice";
import type { CatalogEntry } from "./bindings/CatalogEntry";
import type { MarketplaceSummary } from "./bindings/MarketplaceSummary";
import type { PluginReport } from "./bindings/PluginReport";
import type { PluginSummary } from "./bindings/PluginSummary";
import type { UpdateOutcome } from "./bindings/UpdateOutcome";
import type { SkillDiagnostic } from "./bindings/SkillDiagnostic";
import type { SkillInfo } from "./bindings/SkillInfo";
import type { SkillReport } from "./bindings/SkillReport";
import type { SkillScope } from "./bindings/SkillScope";
import type { SlashCommand } from "./bindings/SlashCommand";
import type { Span } from "./bindings/Span";
import type { Protocol } from "./bindings/Protocol";
import type { ProviderSpec } from "./bindings/ProviderSpec";
import type { Providers } from "./bindings/Providers";
import type { Language } from "./bindings/Language";
import type { Preferences } from "./bindings/Preferences";
import type { Theme } from "./bindings/Theme";
import type { Refusal } from "./bindings/Refusal";
import type { Settings } from "./bindings/Settings";
import type { Startup } from "./bindings/Startup";
import type { TodoItem } from "./bindings/TodoItem";
import type { ToolMeta } from "./bindings/ToolMeta";
import type { UsageSnapshot } from "./bindings/UsageSnapshot";

export type {
  BrowserState,
  Rect,
  ViewportSource,
  AttachedChat,
  AttachmentLook,
  ShellChoice,
  AgentEvent,
  Language,
  Preferences,
  Theme,
  Refusal,
  Settings,
  CheckpointChange,
  CheckpointId,
  CheckpointSummary,
  Command,
  DiffLine,
  Effort,
  EventFrame,
  FileDiff,
  FileFact,
  FileView,
  Hunk,
  ViewSegment,
  Mode,
  McpServerSettings,
  McpServerSummary,
  McpSource,
  McpStatus,
  LspServerSummary,
  LspStatus,
  ElicitField,
  ElicitKind,
  ElicitationReply,
  ElicitationRequest,
  ModelInfo,
  OpenedChat,
  PermissionAnswer,
  PermissionKind,
  PermissionRequest,
  Invariant,
  Unsaved,
  QuestionOption,
  QuestionReply,
  QuestionRequest,
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
  CatalogEntry,
  MarketplaceSummary,
  PluginReport,
  PluginSummary,
  UpdateOutcome,
  SkillDiagnostic,
  SkillInfo,
  SkillReport,
  SkillScope,
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

/**
 * Что лежит по пути вложения — и миниатюра, если это картинка.
 *
 * Путь тот, что записан в ленте: от корня проекта либо абсолютный, если файл
 * лежит снаружи. Смотрит ядро — вебвью диска не видит. В стенде смотреть
 * некому, и всё, кроме пути с косой чертой на конце, считается файлом.
 */
export function attachmentLook(project: string, path: string): Promise<AttachmentLook> {
  if (!inTauri) {
    return Promise.resolve({ kind: /[\\/]$/.test(path) ? "folder" : "file", preview: null });
  }
  return invoke<AttachmentLook>("attachment_look", { project, path });
}

/**
 * Картинка целиком, `data:`-адресом, — для просмотрщика. Миниатюры хватает
 * узнать картинку, но не рассмотреть её.
 */
export function imageFull(project: string, path: string): Promise<string> {
  if (!inTauri) return Promise.reject("в стенде картинку прочитать некому");
  return invoke<string>("image_full", { project, path });
}

/**
 * Сохранить вставленную картинку (base64 без префикса) и вернуть её путь от
 * корня проекта. У вставленного файла пути нет, а всё дальше — чип, журнал,
 * карточка открытой заново сессии — говорит путями.
 */
export function stashImage(project: string, data: string): Promise<string> {
  if (!inTauri) return Promise.reject("в стенде вставленное сохранить некуда");
  return invoke<string>("stash_image", { project, data });
}

/**
 * Файлы, брошенные в окно. Возвращает отписку.
 *
 * Оболочка перехватывает перетаскивание сама и отдаёт пути, а не содержимое:
 * событие `drop` до страницы не доходит, зато и читать файл странице незачем.
 */
export function onFilesDropped(listener: (paths: string[]) => void): () => void {
  if (!inTauri) return () => {};
  let off: (() => void) | null = null;
  let gone = false;
  void getCurrentWebview()
    .onDragDropEvent((event) => {
      if (event.payload.type === "drop") listener(event.payload.paths);
    })
    .then((unlisten) => {
      if (gone) unlisten();
      else off = unlisten;
    });
  return () => {
    gone = true;
    off?.();
  };
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
    // Записанная сессия стенда открывается под своим именем и молчит: строка
    // списка должна стать открытой, а не завести рядом новый разговор.
    const recorded = resume !== undefined && demoSessions.meta(resume) !== null;
    return Promise.resolve({
      id: recorded ? resume : startDemo(path),
      state: { ...demoState, projectRoot: path },
      mcp: structuredClone(demoMcp),
      lsp: structuredClone(demoLsp),
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
 * Picks up the chats that kept running while the window was gone.
 *
 * The webview reloads (Ctrl+R, a hot reload) and the sessions in the shell do
 * not: the turn goes on, the commands go on, a question waits for its answer.
 * Each chat comes back as the events its feed is the fold of, plus a new bridge
 * whose frames carry the returned `epoch`; subscribe to `onEvents` first, or the
 * frames between this call and the subscription are lost.
 *
 * The stand has no shell and nothing outlives its page.
 */
export function attachChats(): Promise<AttachedChat[]> {
  if (!inTauri) return Promise.resolve([]);
  return invoke<AttachedChat[]>("attach_chats");
}

/**
 * Список сессий §9. Работает с проектом, а не с открытым чатом: журналы лежат
 * и у проекта, в котором сейчас никто не разговаривает, а чатов в одном
 * проекте может быть открыто несколько.
 *
 * В стенде список живёт в памяти процесса (`core/demoSessions`) и отвечает на
 * правки так же, как ответило бы хранилище. Экспорта там нет: писать файл
 * некому.
 */
export const sessions = {
  list: (project: string): Promise<SessionMeta[]> =>
    inTauri
      ? invoke<SessionMeta[]>("list_sessions", { project })
      : Promise.resolve(demoSessions.list(project)),
  search: (project: string, query: string, limit?: number): Promise<SearchHit[]> =>
    inTauri
      ? invoke<SearchHit[]>("search_sessions", { project, query, limit })
      : Promise.resolve(demoSessions.search(project, query)),
  rename: (project: string, id: SessionId, title: string): Promise<void> =>
    inTauri
      ? invoke<void>("rename_session", { project, id, title })
      : Promise.resolve(demoSessions.rename(project, id, title)),
  remove: (project: string, id: SessionId): Promise<void> =>
    inTauri
      ? invoke<void>("delete_session", { project, id })
      : Promise.resolve(demoSessions.remove(project, id)),
  duplicate: (project: string, id: SessionId, through?: string): Promise<SessionMeta> =>
    inTauri
      ? invoke<SessionMeta>("duplicate_session", { project, id, through })
      : Promise.resolve().then(() => demoSessions.duplicate(project, id)),
  /** Экспорт §9 пишется ядром по выбранному окном пути. */
  exportTo: (project: string, id: SessionId, path: string): Promise<void> =>
    inTauri
      ? invoke<void>("export_session", { project, id, path })
      : Promise.reject("в стенде файл записать некому"),
  /**
   * Журнал ленты §9 — им перерисовывается лента открытой заново сессии.
   *
   * Ядро отдаёт его уже подчищенным: процессы прошлого прогона, его состояние
   * и чекпоинты, к которым новый прогон вернуться не может, в поток не
   * попадают (`yaca_core::store::replay`). Интерфейсу остаётся то же, что и
   * всегда, — сложить поток в ленту.
   */
  feed: (project: string, id: SessionId): Promise<AgentEvent[]> =>
    inTauri
      ? invoke<AgentEvent[]>("session_feed", { project, id })
      : Promise.resolve(demoSessions.feed(project, id)),
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
 * Skills a chat in this project sees: the project's and the human's own, with
 * what is wrong with the files that did not load. For the slash menu and the
 * settings window; the core expands `/name` itself when the message is sent.
 *
 * The stand has none, for the same reason it has no commands.
 */
export function projectSkills(project: string): Promise<SkillReport> {
  return inTauri
    ? invoke<SkillReport>("project_skills", { project })
    : Promise.resolve({ skills: [], diagnostics: [] });
}

/**
 * Plugins (Yaca's own and Claude Code's) with their switches. The stand has
 * none: plugins are folders on a machine, and the stand has no machine.
 */
export function listPlugins(): Promise<PluginReport> {
  return inTauri
    ? invoke<PluginReport>("list_plugins")
    : Promise.resolve({ plugins: [], diagnostics: [] });
}

/** The marketplaces plugins are installed from. None in the stand. */
export function listMarketplaces(): Promise<MarketplaceSummary[]> {
  return inTauri ? invoke<MarketplaceSummary[]>("list_marketplaces") : Promise.resolve([]);
}

const NO_STAND = () => Promise.reject(new Error("no plugins in the stand"));

/** Adds a marketplace by address and fetches it. */
export function addMarketplace(source: string): Promise<MarketplaceSummary> {
  return inTauri ? invoke<MarketplaceSummary>("add_marketplace", { source }) : NO_STAND();
}

export function removeMarketplace(name: string): Promise<void> {
  return inTauri ? invoke<void>("remove_marketplace", { name }) : NO_STAND();
}

export function refreshMarketplace(name: string): Promise<MarketplaceSummary> {
  return inTauri ? invoke<MarketplaceSummary>("refresh_marketplace", { name }) : NO_STAND();
}

/** What a marketplace offers; the first look fetches it. */
export function marketplaceCatalog(name: string): Promise<CatalogEntry[]> {
  return inTauri ? invoke<CatalogEntry[]>("marketplace_catalog", { name }) : NO_STAND();
}

/** Installs `plugin@marketplace`; it is on afterwards. */
export function installPlugin(id: string): Promise<PluginSummary> {
  return inTauri ? invoke<PluginSummary>("install_plugin", { id }) : NO_STAND();
}

export function updatePlugin(id: string): Promise<UpdateOutcome> {
  return inTauri ? invoke<UpdateOutcome>("update_plugin", { id }) : NO_STAND();
}

export function uninstallPlugin(id: string): Promise<void> {
  return inTauri ? invoke<void>("uninstall_plugin", { id }) : NO_STAND();
}

/** Switches a plugin for the chats opened after this; returns the new list. */
export function setPluginEnabled(id: string, enabled: boolean): Promise<PluginReport> {
  return inTauri
    ? invoke<PluginReport>("set_plugin_enabled", { id, enabled })
    : Promise.reject(new Error("no plugins in the stand"));
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
  // Предустановленные те же, что в ядре, и тоже без моделей: их отдаёт сам
  // провайдер, а в стенде спросить некого — панель честно скажет, что моделей
  // нет. Кроме Anthropic: стенду нужно, на чём показать разговор.
  list: PRESETS.map(({ keyless, ...preset }) => ({
    ...preset,
    // A server that never asks for a key drops the default `authorization`.
    headers: keyless ? { authorization: "" } : {},
    models: preset.id === BUILTIN_PROVIDER_ID ? demoModels : [],
  })),
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

/**
 * The list after a command that also asked a provider for its models
 * (`providers::Asked` in the shell).
 *
 * `pollError` is separate from the command failing: the key is in the keychain
 * and the provider is in the file whether or not its address then answered.
 */
export type Asked = { providers: Providers; pollError: string | null };

export const providers = {
  list: (): Promise<Providers> =>
    inTauri ? invoke<Providers>("providers") : Promise.resolve(demoProviders()),
  /** Сохранённый без моделей провайдер оболочка тут же опрашивает сама. */
  save: (spec: ProviderSpec): Promise<Asked> => {
    if (!inTauri) {
      const at = demoRegistry.list.findIndex((p) => p.id === spec.id);
      if (at === -1) demoRegistry.list.push(spec);
      else demoRegistry.list[at] = spec;
      demoProvidersHandler?.();
      // Опроса в стенде нет, и ошибки опроса тоже: строка останется
      // «не опрошен», что и есть правда.
      return Promise.resolve({ providers: demoProviders(), pollError: null });
    }
    return invoke<Asked>("save_provider", { spec });
  },
  remove: (id: string): Promise<Providers> => {
    if (!inTauri) {
      if (isPreset(id)) {
        return Promise.reject("предустановленный провайдер не удаляется");
      }
      demoRegistry.list = demoRegistry.list.filter((p) => p.id !== id);
      demoProvidersHandler?.();
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
    answerInDemo(session, command);
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
  if (!inTauri) return Promise.resolve(demoSessions.meta(session));
  return invoke<SessionMeta | null>("session_meta", { session });
}

/**
 * §10: ключ уезжает в системный кейчейн. Обратно он не возвращается.
 *
 * Лежит под именем провайдера, и провайдер называется всегда: угаданный
 * записал бы ключ не под тем именем.
 *
 * Провайдера из списка оболочка после этого опрашивает сама: ключ — то, чего
 * не хватало, чтобы спросить. Ответ приезжает сюда же вместе со списком.
 */
export function setApiKey(key: string, provider: string): Promise<Asked> {
  if (!inTauri) return Promise.reject("в стенде кейчейна нет");
  return invoke<Asked>("set_api_key", { key, provider });
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

/** The provider id the model router is listed under; it is not in `providers.json`. */
export const ROUTER_PROVIDER_ID = "router";

/**
 * What `routing.json` amounts to. `broken` is a state of its own and not
 * `absent`: a router that quietly leaves the model list because of a typo is a
 * setting the window lies about.
 */
export type RoutingStatus =
  | { state: "absent" }
  | { state: "ready"; model: ModelInfo }
  | { state: "broken"; error: string };

/** The stand has no `routing.json`: nothing is claimed, and nothing is offered. */
export function routingStatus(): Promise<RoutingStatus> {
  if (!inTauri) return Promise.resolve({ state: "absent" });
  return invoke<RoutingStatus>("routing_status");
}

/** The file as it is on disk, or `null` when there is none. */
export function routingText(): Promise<string | null> {
  if (!inTauri) return Promise.resolve(null);
  return invoke<string | null>("routing_text");
}

/**
 * Checks the text with the core's own rules and writes it. A refusal rejects
 * with the words the file would be refused with, and nothing is written.
 */
export function saveRouting(text: string): Promise<RoutingStatus> {
  if (!inTauri) return Promise.reject(new Error("routing.json is written by the app only"));
  return invoke<RoutingStatus>("save_routing", { text });
}

export function removeRouting(): Promise<void> {
  if (!inTauri) return Promise.resolve();
  return invoke<void>("remove_routing");
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

/**
 * Ответы стенда на команды, на которые у ядра есть ответ событием.
 *
 * Панель файла (§6.4) висела бы на «Читаю файл…», и `@` висел бы на «ищем…»,
 * если бы стенд молчал, — а это интерфейс, врущий о состоянии. Поэтому стенд
 * отвечает, но только тем, что знает: единственным файлом демо. Кадр уходит
 * следующим тиком, как приходит от ядра: ответ в том же стеке вызова прошёл
 * бы раньше, чем интерфейс успел запомнить запрос.
 */
function answerInDemo(session: SessionId, command: Command): void {
  if (command.type === "answerPermission") {
    // The recorded turn stops on this question; answering it lets the turn
    // end, as it would with a core behind it.
    const events = demoFinish();
    window.setTimeout(() => demoHandler?.({ session, epoch: 0, events }), 0);
    return;
  }
  if (command.type === "readFileView") {
    const events: AgentEvent[] = [
      { type: "fileViewLoaded", path: command.path, view: demoFileView(command.path) },
    ];
    window.setTimeout(() => demoHandler?.({ session, epoch: 0, events }), 0);
    return;
  }
  if (command.type === "searchFiles") {
    // The stand has one file. It is offered when the query could name it, and
    // an empty answer otherwise: the project it stands for has no others.
    const query = command.query.toLowerCase();
    const paths = DEMO_FILE_PATH.toLowerCase().includes(query) ? [DEMO_FILE_PATH] : [];
    const events: AgentEvent[] = [{ type: "filesFound", query: command.query, paths }];
    window.setTimeout(() => demoHandler?.({ session, epoch: 0, events }), 0);
    return;
  }
  if (command.type === "approveMcpServer" || command.type === "reconnectMcpServer") {
    // Одобренный сервер стенда «запускается»; переподключённый остаётся тем,
    // чем был, — упавший упадёт снова, как и настоящий с той же командой.
    const approved = command.type === "approveMcpServer" ? command.server : null;
    const servers = (demoMcpNow.get(session) ?? structuredClone(demoMcp)).map(
      (server): McpServerSummary =>
        server.name === approved && server.status === "needsApproval"
          ? {
              ...server,
              status: "ready",
              label: "server-files 0.6.2",
              tools: [
                { name: "read_file", description: "Read a file.", readOnly: true },
                { name: "write_file", description: "Write a file.", readOnly: false },
              ],
            }
          : server,
    );
    demoMcpNow.set(session, servers);
    const events: AgentEvent[] = [{ type: "mcpUpdated", servers }];
    window.setTimeout(() => demoHandler?.({ session, epoch: 0, events }), 0);
  }
}

/** Серверы MCP каждого чата стенда — после того, что с ними сделали. */
const demoMcpNow = new Map<SessionId, McpServerSummary[]>();

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
      epoch: 0,
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
  answerQuestion: (requestId: string, answer: QuestionReply): Command => ({
    type: "answerQuestion",
    requestId,
    answer,
  }),
  setBypass: (on: boolean): Command => ({ type: "setBypass", on }),
  watchProcess: (id: ProcessId | null): Command => ({
    type: "watchProcess",
    id,
  }),
  stopProcess: (id: ProcessId): Command => ({ type: "stopProcess", id }),
  readFileView: (path: string): Command => ({ type: "readFileView", path }),
  searchFiles: (query: string): Command => ({ type: "searchFiles", query }),
  reconnectMcpServer: (server: string): Command => ({ type: "reconnectMcpServer", server }),
  approveMcpServer: (server: string): Command => ({ type: "approveMcpServer", server }),
  restartLspServer: (server: string): Command => ({ type: "restartLspServer", server }),
  loginMcpServer: (server: string): Command => ({ type: "loginMcpServer", server }),
  logoutMcpServer: (server: string): Command => ({ type: "logoutMcpServer", server }),
  answerElicitation: (requestId: string, reply: ElicitationReply): Command => ({
    type: "answerElicitation",
    requestId,
    reply,
  }),
  cancelQueued: (id: QueuedId): Command => ({ type: "cancelQueued", id }),
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

// --- настройки §10 ----------------------------------------------------------

/** Канал настроек приложения. Одно имя на оба окна: см. `prefs.rs`. */
const PREFS_CHANNEL = "yaca://preferences";

/** Канал «список провайдеров изменился» — см. `providers.rs`. */
const PROVIDERS_CHANNEL = "yaca://providers";

/**
 * Подписка на правку провайдеров в соседнем окне.
 *
 * Полезной нагрузки нет: список перечитывается целиком. Слать его событием
 * значило бы завести второй путь, которым он попадает в окно, и первое же
 * расхождение между путями стало бы разницей в переключателе моделей.
 */
export function onProviders(handler: () => void): Promise<UnlistenFn> {
  if (!inTauri) {
    demoProvidersHandler = handler;
    return Promise.resolve(() => {
      if (demoProvidersHandler === handler) demoProvidersHandler = null;
    });
  }
  return listen<null>(PROVIDERS_CHANNEL, () => handler());
}

let demoProvidersHandler: (() => void) | null = null;

/**
 * Умолчания настроек приложения — те же, что в `prefs.rs`.
 *
 * Нужны стенду и первому кадру: окно рисуется до того, как оболочка ответит,
 * и рисовать его нечем было бы. Значения повторены здесь не от хорошей жизни —
 * ts-rs переносит типы, но не `impl Default`, — поэтому они обязаны совпадать
 * с Rust, и тест `prefs.test.ts` за этим следит.
 */
export const DEFAULT_PREFS: Preferences = {
  theme: "paper",
  lineNumbers: true,
  compact: false,
  language: "system",
  notifications: true,
  respectRobots: true,
  shell: "auto",
};

/**
 * Шеллы, из которых на этой машине есть что выбирать (§10), кроме «авто».
 *
 * Пусто везде, кроме Windows: шелл там один, и строки с выбором, которого
 * нет, окно настроек не рисует (§0). В стенде оболочки нет — и выбирать нечего.
 */
export function installedShells(): Promise<ShellChoice[]> {
  return inTauri ? invoke<ShellChoice[]>("installed_shells") : Promise.resolve([]);
}

/** В стенде оболочки нет, и настройки живут в памяти страницы. */
let demoPrefs: Preferences = { ...DEFAULT_PREFS };
let demoPrefsHandler: ((prefs: Preferences) => void) | null = null;

export function preferences(): Promise<Preferences> {
  return inTauri ? invoke<Preferences>("preferences") : Promise.resolve({ ...demoPrefs });
}

/** Пишет настройки и рассылает их по окнам — обоим (§0). */
export function setPreferences(prefs: Preferences): Promise<void> {
  if (!inTauri) {
    demoPrefs = { ...prefs };
    demoPrefsHandler?.({ ...prefs });
    return Promise.resolve();
  }
  return invoke<void>("set_preferences", { prefs });
}

/** Подписка на чужую правку настроек: второе окно меняет вид этого. */
export function onPreferences(
  handler: (prefs: Preferences) => void,
): Promise<UnlistenFn> {
  if (!inTauri) {
    demoPrefsHandler = handler;
    return Promise.resolve(() => {
      if (demoPrefsHandler === handler) demoPrefsHandler = null;
    });
  }
  return listen<Preferences>(PREFS_CHANNEL, (e) => handler(e.payload));
}

/** Пустые настройки проекта — форма, с которой начинается несохранённый файл. */
export function emptySettings(): Settings {
  return {
    mode: null,
    provider: null,
    model: null,
    compactModel: null,
    subagentModel: null,
    permissions: { rules: [], denies: [] },
    shadow: { exclude: [] },
    verify: null,
    mcpServers: {},
  };
}

/** В стенде настройки проекта живут в памяти страницы, как и список проектов. */
let demoSettings: Settings = emptySettings();

/** Настройки проекта (§10). Битый файл приезжает отказом, а не пустотой. */
export function projectSettings(project: string): Promise<Settings> {
  if (!inTauri) return Promise.resolve(structuredClone(demoSettings));
  return invoke<Settings>("project_settings", { project });
}

/** Пишет `.yaca/settings.json` целиком. */
export function saveProjectSettings(project: string, settings: Settings): Promise<void> {
  if (!inTauri) {
    demoSettings = structuredClone(settings);
    return Promise.resolve();
  }
  return invoke<void>("save_project_settings", { project, settings });
}

/**
 * Каталоги, в которые снимок §5.8 не заходит никогда.
 *
 * Спрашиваются у ядра, а не записаны здесь: список его, и копия во фронтенде
 * разошлась бы с ним на первом же добавлении.
 */
export function shadowExcludedDirs(): Promise<string[]> {
  if (!inTauri) return Promise.resolve([".yaca", ".git", "node_modules", "target"]);
  return invoke<string[]>("shadow_excluded_dirs");
}

/** Какой файл инструкций у проекта в силе (§10), или `null`. */
export function projectInstructions(project: string): Promise<string | null> {
  if (!inTauri) return Promise.resolve("CLAUDE.md");
  return invoke<string | null>("project_instructions", { project });
}

/** Ключ веб-поиска лежит в кейчейне под этим именем — см. `secrets.rs`. */
export const SEARCH_KEY_ITEM = "tavily";

/**
 * Откуда взялся ключ веб-поиска (§10).
 *
 * Своя команда, не `apiKeySource`: Tavily не провайдер, и общая ответила бы
 * «нет в списке» про ключ, который лежит в кейчейне.
 */
export function searchKeySource(): Promise<KeySource> {
  if (!inTauri) return Promise.resolve("none");
  return invoke<KeySource>("search_key_source");
}

// --- MCP ---------------------------------------------------------------------

type McpServers = { [key in string]?: McpServerSettings };

/** В стенде общий список серверов живёт в памяти страницы. */
let demoOwnMcp: McpServers = {};

/**
 * Свои серверы MCP — `mcp.json` в каталоге настроек, общие для всех проектов.
 * Битый файл приезжает отказом: окно не должно предлагать перезаписать
 * серверы, которые не сумело прочитать (§0).
 */
export function globalMcpServers(): Promise<McpServers> {
  if (!inTauri) return Promise.resolve(structuredClone(demoOwnMcp));
  return invoke<McpServers>("global_mcp_servers");
}

/** Пишет общий список целиком. До открытого чата он не доходит: тот работает
    с серверами, с которыми начал. */
export function saveGlobalMcpServers(servers: McpServers): Promise<void> {
  if (!inTauri) {
    demoOwnMcp = structuredClone(servers);
    return Promise.resolve();
  }
  return invoke<void>("save_global_mcp_servers", { servers });
}

/** Что объявляет `.mcp.json` проекта. Только для показа: файл не наш. */
export function projectMcpFile(project: string): Promise<McpServers> {
  if (!inTauri) return Promise.resolve({});
  return invoke<McpServers>("project_mcp_file", { project });
}

/**
 * Серверы открытого чата как они есть сейчас. Обычно их приносит событие
 * `mcpUpdated`; это — для окна, которое в тот момент не слушало.
 */
export function sessionMcp(session: SessionId): Promise<McpServerSummary[]> {
  if (!inTauri) return Promise.resolve(demoMcpNow.get(session) ?? structuredClone(demoMcp));
  return invoke<McpServerSummary[]>("session_mcp", { session });
}

/** Отказ без причины: `null`, а не пустая строка — так пишет ядро. */
export function refusal(scope: ScopeKey, reason: string): Refusal {
  const trimmed = reason.trim();
  return { scope, reason: trimmed === "" ? null : trimmed };
}

// --- the browser column -------------------------------------------------------

const BROWSER_CHANNEL = "yaca://browser";

/** A step of the page the column's arrows and reload button take. */
export type BrowserStep = "back" | "forward" | "reload" | "stop";

const NO_BROWSER = () => Promise.reject(new Error("no browser in the stand"));

/**
 * The chats' browsers (`src-tauri/src/browser`). The stand has none: its
 * window says so by not offering the column at all.
 */
export const browser = {
  supported: (): Promise<boolean> =>
    inTauri ? invoke<boolean>("browser_supported") : Promise.resolve(false),
  states: (): Promise<BrowserState[]> =>
    inTauri ? invoke<BrowserState[]>("browser_states") : Promise.resolve([]),
  /** Shows `session`'s page at `rect` and hides every other; both `null` hide all. */
  place: (session: SessionId | null, rect: Rect | null): Promise<void> =>
    inTauri ? invoke<void>("browser_place", { session, rect }) : Promise.resolve(),
  open: (session: SessionId, url: string): Promise<void> =>
    inTauri ? invoke<void>("browser_open", { session, url }) : NO_BROWSER(),
  go: (session: SessionId, step: BrowserStep): Promise<void> =>
    inTauri ? invoke<void>("browser_go", { session, step }) : NO_BROWSER(),
  external: (session: SessionId): Promise<void> =>
    inTauri ? invoke<void>("browser_external", { session }) : NO_BROWSER(),
  devtools: (session: SessionId): Promise<void> =>
    inTauri ? invoke<void>("browser_devtools", { session }) : NO_BROWSER(),
  resetViewport: (session: SessionId): Promise<void> =>
    inTauri ? invoke<void>("browser_reset_viewport", { session }) : NO_BROWSER(),
  onState: (handler: (state: BrowserState) => void): Promise<UnlistenFn> =>
    inTauri
      ? listen<BrowserState>(BROWSER_CHANNEL, (event) => handler(event.payload))
      : Promise.resolve(() => {}),
};
