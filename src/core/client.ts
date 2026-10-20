/**
 * Typed client of the core.
 *
 * The only place where the frontend knows about Tauri. Types come from Rust
 * via ts-rs, so they cannot drift from the core: a mismatch is caught by the
 * compiler, not by a person reading the feed.
 */

import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";

import { demoFinish, demoLsp, demoMcp, demoModels, demoScript, demoState, inTauri } from "./demo";
import { DEMO_FILE_PATH, demoFileView } from "./demoFile";
import { isRemote } from "./link";
import { askServerFolder, exportText, giveFile, pickImages } from "./pickers";
import { demoSessions } from "./demoSessions";
import { BUILTIN_PROVIDER_ID, PRESETS, isPreset, needsKey } from "./provider";
import { call, listenChannel } from "./transport";

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
import type { ReleaseInfo } from "./bindings/ReleaseInfo";
import type { UpdateCheck } from "./bindings/UpdateCheck";

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
  ReleaseInfo,
  UpdateCheck,
};

/** The bridge sends events in batches per frame, not one by one (§12). */
const EVENT_CHANNEL = "yaca://events";

/**
 * A window opened without the shell plays back a recorded stream (`core/demo`).
 *
 * The stand is for looking at and nothing more: in a real window `inTauri` is true,
 * and not one line below runs. Replacing live events with recorded ones where they
 * are live would mean lying about the state (§0).
 */
const DEMO_FRAME_MS = 90;

/** Where the stand sends its frames. Set by a subscription, started by opening a project. */
let demoHandler: ((frame: EventFrame) => void) | null = null;

/**
 * Players for the stand's open chats: one per conversation.
 *
 * There is one recording, but it can be opened any number of times: each chat plays
 * its own copy under its own name. A single player for all of them would mean that a
 * window with two conversations could not be looked at in a browser, and that is
 * exactly what the stand checks.
 */
const demoTimers = new Map<SessionId, number>();
let demoNext = 1;

/**
 * Projects the window knows (§9.1), freshest first.
 *
 * The list lives in the app's global settings: a project has nowhere to keep it,
 * since it is needed before any project is known.
 *
 * In the stand the list lives in process memory and answers edits the way the shell
 * would — otherwise an added project would look added while the list stayed the
 * same (§0).
 */
let demoProjects: string[] = [demoState.projectRoot];

export function projects(): Promise<string[]> {
  return inTauri ? call<string[]>("projects") : Promise.resolve([...demoProjects]);
}

/** Adds a project to the list without starting a chat (§9.1). Returns the whole list. */
export function addProject(path: string): Promise<string[]> {
  if (!inTauri) {
    demoProjects = [path, ...demoProjects.filter((known) => known !== path)];
    return Promise.resolve([...demoProjects]);
  }
  return call<string[]>("add_project", { path });
}

/** Removes a project from the list. Leaves the folder and the §9 journals alone. */
export function forgetProject(path: string): Promise<string[]> {
  if (!inTauri) {
    demoProjects = demoProjects.filter((known) => known !== path);
    return Promise.resolve([...demoProjects]);
  }
  return call<string[]>("forget_project", { path });
}

/**
 * Whether this window is a desktop shell with its own host. Drag-and-drop paths
 * and the browser column belong to it: against a server, a path on this machine
 * names nothing, and the server has no browser column to draw.
 */
export const localShell = (): boolean => inTauri && !isRemote();

/**
 * Whether anything can open a file dialog: for choosing a folder, files or a save path.
 *
 * In the stand nothing can: dialogs belong to the shell. The project chip and the
 * attachment `＋` ask about this and switch off, rather than pretending to be buttons
 * with nothing behind them (§0).
 */
export const hasFileDialogs = inTauri;

/** Folder picker for the project chip (§9.1). `null` means the person changed their mind. */
export function chooseFolder(title: string): Promise<string | null> {
  if (!hasFileDialogs) return Promise.resolve(null);
  // Against a server the project is on the server's disk, not under this dialog.
  if (isRemote()) return askServerFolder(title);
  return open({ directory: true, multiple: false, title }).then((picked) =>
    typeof picked === "string" ? picked : null,
  );
}

/**
 * Save-file dialog (§9). `null` means the person changed their mind.
 *
 * The path is returned rather than written here: writing belongs to the core, which
 * holds what is being saved. The webview chooses the place and does not carry the
 * content there and back.
 */
export function chooseSavePath(
  defaultName: string,
  title: string,
  extension: string,
): Promise<string | null> {
  if (!hasFileDialogs) return Promise.resolve(null);
  // There is no path on this disk to give the server; the name stands for the
  // file that `exportTo` hands over instead.
  if (isRemote()) return Promise.resolve(defaultName);
  return save({
    defaultPath: defaultName,
    title,
    filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
  });
}

/**
 * File picker for `＋` (§7.2). An empty list means the person changed their mind.
 *
 * Starts at the project root: attachments almost always come from there, and a dialog
 * that opens in the home folder makes the person walk there by hand.
 */
export function chooseFiles(root: string, title: string): Promise<string[]> {
  if (!hasFileDialogs) return Promise.resolve([]);
  // A server cannot read this device's files, so what can be attached is what
  // can be sent: pictures, kept in the project like a pasted one (§7.2).
  if (isRemote()) {
    return pickImages().then((pictures) => Promise.all(pictures.map((data) => stashImage(root, data))));
  }
  return open({ directory: false, multiple: true, defaultPath: root, title }).then(
    (picked) => (Array.isArray(picked) ? picked : picked === null ? [] : [picked]),
  );
}

/**
 * What lies at an attachment's path — and a thumbnail if it is a picture.
 *
 * The path is the one recorded in the feed: relative to the project root, or absolute
 * if the file lies outside it. The core looks, since the webview cannot see the disk.
 * In the stand nothing looks, so everything except a path with a slash at the end
 * counts as a file.
 */
export function attachmentLook(project: string, path: string): Promise<AttachmentLook> {
  if (!inTauri) {
    return Promise.resolve({ kind: /[\\/]$/.test(path) ? "folder" : "file", preview: null });
  }
  return call<AttachmentLook>("attachment_look", { project, path });
}

/**
 * The whole picture as a `data:` address, for the viewer. A thumbnail is enough to
 * recognise a picture, but not to look at it.
 */
export function imageFull(project: string, path: string): Promise<string> {
  if (!inTauri) return Promise.reject("in the stand there is no one to read the picture");
  return call<string>("image_full", { project, path });
}

/**
 * Saves a pasted picture (base64 without the prefix) and returns its path from the
 * project root. A pasted file has no path of its own, and everything after it — the
 * chip, the journal, the card of a reopened session — speaks in paths.
 */
export function stashImage(project: string, data: string): Promise<string> {
  if (!inTauri) return Promise.reject("in the stand there is nowhere to save a pasted image");
  return call<string>("stash_image", { project, data });
}

/**
 * Whether the person is dragging files over the window right now: `true` from the
 * first entry while they hover over it, `false` once they leave or drop them. The
 * window needs to say where they will land before they are released. Returns an
 * unsubscribe.
 */
export function onFilesDragging(listener: (over: boolean) => void): () => void {
  if (!localShell()) return () => {};
  let off: (() => void) | null = null;
  let gone = false;
  void getCurrentWebview()
    .onDragDropEvent((event) => {
      const kind = event.payload.type;
      if (kind === "enter" || kind === "over") listener(true);
      else listener(false);
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

/**
 * Files dropped on the window. Returns an unsubscribe.
 *
 * The shell intercepts the drag itself and hands over paths, not contents: the `drop`
 * event never reaches the page, and the page has no reason to read the file anyway.
 */
export function onFilesDropped(listener: (paths: string[]) => void): () => void {
  if (!localShell()) return () => {};
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

/** Name for the greeting (§9.1). Taken from the OS; if none is found, the greeting has no name. */
export function userName(): Promise<string | null> {
  return inTauri ? call<string | null>("user_name") : Promise.resolve(null);
}

/**
 * What the project itself would start a session with (§10).
 *
 * The home screen composer shows this before the first word; otherwise it would
 * silently overwrite `.yaca/settings.json` with the built-in defaults.
 */
export function projectStartup(path: string): Promise<Startup> {
  if (!inTauri) {
    const { mode, provider, model, effort } = demoState;
    return Promise.resolve({ mode, provider, model, effort });
  }
  return call<Startup>("project_startup", { path });
}

/**
 * `resume` continues a recorded session (§9) instead of starting a new one.
 *
 * `startup` is what the home screen composer chose before the first word.
 * Without it the session takes the project's settings (§10).
 */
export function openProject(
  path: string,
  resume?: SessionId,
  startup?: Startup,
): Promise<OpenedChat> {
  if (!inTauri) {
    // The stream starts with the chat, not with the subscription: the window clears
    // the feed when a chat is opened, and anything recorded before that would be wiped.
    // A recorded session of the stand opens under its own name and stays silent: the
    // list row must become open, not start a new conversation next to it.
    const recorded = resume !== undefined && demoSessions.meta(resume) !== null;
    return Promise.resolve({
      id: recorded ? resume : startDemo(path),
      state: { ...demoState, projectRoot: path },
      mcp: structuredClone(demoMcp),
      lsp: structuredClone(demoLsp),
    });
  }
  return call<OpenedChat>("open_project", { path, resume, startup });
}

/**
 * Closes the chat: the turn stops, and background commands die with the session
 * (§6.5); the §9 journal stays. A closed chat reopens with its own history: closing
 * is not deleting.
 */
export function closeChat(session: SessionId): Promise<void> {
  if (!inTauri) {
    stopDemo(session);
    return Promise.resolve();
  }
  return call<void>("close_chat", { session });
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
  return call<AttachedChat[]>("attach_chats");
}

/**
 * The §9 session list. It works with the project, not with an open chat: journals
 * also live with a project in which nobody is talking right now, and several chats of
 * one project may be open at once.
 *
 * In the stand the list lives in process memory (`core/demoSessions`) and answers
 * edits the way the store would. There is no export there: nothing can write a file.
 */
export const sessions = {
  list: (project: string): Promise<SessionMeta[]> =>
    inTauri
      ? call<SessionMeta[]>("list_sessions", { project })
      : Promise.resolve(demoSessions.list(project)),
  search: (project: string, query: string, limit?: number): Promise<SearchHit[]> =>
    inTauri
      ? call<SearchHit[]>("search_sessions", { project, query, limit })
      : Promise.resolve(demoSessions.search(project, query)),
  rename: (project: string, id: SessionId, title: string): Promise<void> =>
    inTauri
      ? call<void>("rename_session", { project, id, title })
      : Promise.resolve(demoSessions.rename(project, id, title)),
  remove: (project: string, id: SessionId): Promise<void> =>
    inTauri
      ? call<void>("delete_session", { project, id })
      : Promise.resolve(demoSessions.remove(project, id)),
  duplicate: (project: string, id: SessionId, through?: string): Promise<SessionMeta> =>
    inTauri
      ? call<SessionMeta>("duplicate_session", { project, id, through })
      : Promise.resolve().then(() => demoSessions.duplicate(project, id)),
  /** The §9 export is written by the core to the path the window picked. */
  exportTo: (project: string, id: SessionId, path: string): Promise<void> =>
    isRemote()
      ? // The server's disk is not the one the person picked a place on: the
        // text comes here and is given over as a file (`path` is only its name).
        exportText(project, id).then((text) => giveFile(path, text))
      : inTauri
      ? call<void>("export_session", { project, id, path })
      : Promise.reject("in the stand there is no one to write the file"),
  /**
   * The §9 feed journal — the feed of a reopened session is redrawn from it.
   *
   * The core hands it over already cleaned up: processes of the previous run, and its
   * state and checkpoints that a new run cannot return to, do not reach the stream
   * (`yaca_core::store::replay`). The interface still does what it always did — folds
   * the stream into the feed.
   */
  feed: (project: string, id: SessionId): Promise<AgentEvent[]> =>
    inTauri
      ? call<AgentEvent[]>("session_feed", { project, id })
      : Promise.resolve(demoSessions.feed(project, id)),
};

/**
 * The project's slash commands (§7.3), which the composer menu completes input with.
 *
 * They belong to the project, not to a chat: the files live in `.yaca/commands`, and
 * several chats of one project may be open. The core expands them when a message is
 * sent; the window needs the list only to draw the menu.
 *
 * The stand has none: commands live as project files, and the stand has no project.
 */
export function projectCommands(project: string): Promise<SlashCommand[]> {
  return inTauri
    ? call<SlashCommand[]>("project_commands", { project })
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
    ? call<SkillReport>("project_skills", { project })
    : Promise.resolve({ skills: [], diagnostics: [] });
}

/**
 * Plugins (Yaca's own and Claude Code's) with their switches. The stand has
 * none: plugins are folders on a machine, and the stand has no machine.
 */
export function listPlugins(): Promise<PluginReport> {
  return inTauri
    ? call<PluginReport>("list_plugins")
    : Promise.resolve({ plugins: [], diagnostics: [] });
}

/** The marketplaces plugins are installed from. None in the stand. */
export function listMarketplaces(): Promise<MarketplaceSummary[]> {
  return inTauri ? call<MarketplaceSummary[]>("list_marketplaces") : Promise.resolve([]);
}

const NO_STAND = () => Promise.reject(new Error("no plugins in the stand"));

/** Adds a marketplace by address and fetches it. */
export function addMarketplace(source: string): Promise<MarketplaceSummary> {
  return inTauri ? call<MarketplaceSummary>("add_marketplace", { source }) : NO_STAND();
}

export function removeMarketplace(name: string): Promise<void> {
  return inTauri ? call<void>("remove_marketplace", { name }) : NO_STAND();
}

export function refreshMarketplace(name: string): Promise<MarketplaceSummary> {
  return inTauri ? call<MarketplaceSummary>("refresh_marketplace", { name }) : NO_STAND();
}

/** What a marketplace offers; the first look fetches it. */
export function marketplaceCatalog(name: string): Promise<CatalogEntry[]> {
  return inTauri ? call<CatalogEntry[]>("marketplace_catalog", { name }) : NO_STAND();
}

/** Installs `plugin@marketplace`; it is on afterwards. */
export function installPlugin(id: string): Promise<PluginSummary> {
  return inTauri ? call<PluginSummary>("install_plugin", { id }) : NO_STAND();
}

export function updatePlugin(id: string): Promise<UpdateOutcome> {
  return inTauri ? call<UpdateOutcome>("update_plugin", { id }) : NO_STAND();
}

export function uninstallPlugin(id: string): Promise<void> {
  return inTauri ? call<void>("uninstall_plugin", { id }) : NO_STAND();
}

/** Switches a plugin for the chats opened after this; returns the new list. */
export function setPluginEnabled(id: string, enabled: boolean): Promise<PluginReport> {
  return inTauri
    ? call<PluginReport>("set_plugin_enabled", { id, enabled })
    : Promise.reject(new Error("no plugins in the stand"));
}

/**
 * Providers (§10): the list and edits.
 *
 * In the stand the registry lives in process memory and repeats the core's semantics
 * (`Providers::put`/`remove`): replacement by id, and a refusal to remove a preset.
 * Otherwise an edit would look successful while the list stayed the same — an interface
 * lying about the state (§0). The stand has no keychain, and it stores no keys.
 */
let demoRegistry: Providers = {
  // Presets are the same as in the core, and also have no models: the provider itself
  // supplies them, and in the stand there is no one to ask — the panel will honestly say
  // there are no models. Anthropic is the exception: the stand needs a provider to show
  // a conversation with.
  list: PRESETS.map(({ keyless, ...preset }) => ({
    ...preset,
    // A server that never asks for a key drops the default `authorization`.
    headers: keyless ? { authorization: "" } : {},
    models: preset.id === BUILTIN_PROVIDER_ID ? demoModels : [],
  })),
};

/** A copy outwards: the panel gets the state as of the call, not a live reference into the stand. */
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
    inTauri ? call<Providers>("providers") : Promise.resolve(demoProviders()),
  /** The shell itself polls a saved provider that has no models straight away. */
  save: (spec: ProviderSpec): Promise<Asked> => {
    if (!inTauri) {
      const at = demoRegistry.list.findIndex((p) => p.id === spec.id);
      if (at === -1) demoRegistry.list.push(spec);
      else demoRegistry.list[at] = spec;
      demoProvidersHandler?.();
      // The stand has no polling, and no polling errors either: the row stays
      // "not polled", which is the truth.
      return Promise.resolve({ providers: demoProviders(), pollError: null });
    }
    return call<Asked>("save_provider", { spec });
  },
  remove: (id: string): Promise<Providers> => {
    if (!inTauri) {
      if (isPreset(id)) {
        return Promise.reject("a preset provider cannot be removed");
      }
      demoRegistry.list = demoRegistry.list.filter((p) => p.id !== id);
      demoProvidersHandler?.();
      return Promise.resolve(demoProviders());
    }
    return call<Providers>("delete_provider", { id });
  },
};

/** The command is addressed to a chat: there are several, and a nameless one would not know which. */
/**
 * Asks a provider which models it serves (§10).
 *
 * The description is taken from the form, not from the registry: the address and the
 * headers may have just been corrected, and the question must go to the one being
 * configured. The shell takes the key from the keychain itself — it never arrives here
 * in any form.
 *
 * In the stand there is no one to ask: the registry lives in the page's memory, and no
 * provider stands behind it. An empty list would be an answer "no models", which is not
 * true, so this is a refusal.
 */
export function discoverModels(spec: ProviderSpec): Promise<ModelInfo[]> {
  if (!inTauri) return Promise.reject("in the stand there is no provider behind the registry");
  return call<ModelInfo[]>("discover_models", { spec });
}

export function sendCommand(session: SessionId, command: Command): Promise<void> {
  if (!inTauri) {
    console.info("[stand] command", command);
    answerInDemo(session, command);
    return Promise.resolve();
  }
  return call<void>("send_command", { session, command });
}

export function sessionState(session: SessionId): Promise<SessionState | null> {
  if (!inTauri) return Promise.resolve(null);
  return call<SessionState | null>("session_state", { session });
}

/** A chat row in the §9 list: title, time, cost. */
export function sessionMeta(session: SessionId): Promise<SessionMeta | null> {
  if (!inTauri) return Promise.resolve(demoSessions.meta(session));
  return call<SessionMeta | null>("session_meta", { session });
}

/**
 * §10: the key goes into the system keychain. It is never returned.
 *
 * It is stored under the provider's name, and the provider is always named: a guessed
 * one would store the key under the wrong name.
 *
 * The shell itself polls the provider from the list afterwards: the key is what was
 * missing to ask. The answer comes back here together with the list.
 */
export function setApiKey(key: string, provider: string): Promise<Asked> {
  if (!inTauri) return Promise.reject("in the stand there is no keychain");
  return call<Asked>("set_api_key", { key, provider });
}

export function forgetApiKey(provider: string): Promise<void> {
  if (!inTauri) return Promise.reject("in the stand there is no keychain");
  return call<void>("forget_api_key", { provider });
}

/**
 * Where the key came from — but not the key itself.
 *
 * `notNeeded` is a provider that does not ask for a key: a local gateway. "No key"
 * would be a warning about nothing.
 */
export type KeySource = "keychain" | "environment" | "none" | "not_needed";

export function apiKeySource(provider: string): Promise<KeySource> {
  if (!inTauri) {
    const spec = demoRegistry.list.find((p) => p.id === provider);
    if (!spec) return Promise.reject(`provider "${provider}" is not in the list`);
    // The stand has neither a keychain nor environment variables, and claiming
    // "environment" would be dishonest: the honest answers are two — "none" and "not needed".
    return Promise.resolve(
      needsKey(spec.protocol, demoHeaderPairs(spec)) ? "none" : "not_needed",
    );
  }
  return call<KeySource>("api_key_source", { provider });
}

export function models(): Promise<ModelInfo[]> {
  if (!inTauri) return Promise.resolve(demoModels);
  return call<ModelInfo[]>("models");
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
  return call<RoutingStatus>("routing_status");
}

/** The file as it is on disk, or `null` when there is none. */
export function routingText(): Promise<string | null> {
  if (!inTauri) return Promise.resolve(null);
  return call<string | null>("routing_text");
}

/**
 * Checks the text with the core's own rules and writes it. A refusal rejects
 * with the words the file would be refused with, and nothing is written.
 */
export function saveRouting(text: string): Promise<RoutingStatus> {
  if (!inTauri) return Promise.reject(new Error("routing.json is written by the app only"));
  return call<RoutingStatus>("save_routing", { text });
}

export function removeRouting(): Promise<void> {
  if (!inTauri) return Promise.resolve();
  return call<void>("remove_routing");
}

/**
 * Which of a provider's models are in memory right now (§7.1), for a server
 * that says. The stand has no server behind its providers, and an empty map
 * claims nothing.
 */
export function modelResidency(provider: string): Promise<Record<string, boolean>> {
  if (!inTauri) return Promise.resolve({});
  return call<Record<string, boolean>>("model_residency", { provider });
}

/**
 * Subscription to the event stream. The handler gets a whole frame: it must be applied
 * in one redraw, otherwise the saving on IPC is lost in rendering.
 *
 * There is one channel for all chats, and each frame arrives with its session name: a
 * subscription per chat would mean an unsubscribe on every close — one more chance to
 * lose the events of whoever is working in the background.
 */
export function onEvents(handler: (frame: EventFrame) => void): Promise<UnlistenFn> {
  if (!inTauri) {
    demoHandler = handler;
    return Promise.resolve(() => {
      // Only its own: React subscribes twice in development, and the unsubscribe of
      // the first subscription arrives after the second one.
      if (demoHandler !== handler) return;
      demoHandler = null;
      stopAllDemos();
    });
  }
  return listenChannel<EventFrame>(EVENT_CHANNEL, handler);
}

/**
 * The stand's answers to commands the core answers with an event.
 *
 * The file panel (§6.4) would hang on "Reading file…", and `@` would hang on "searching…"
 * if the stand stayed silent — an interface lying about the state. So the stand answers,
 * but only with what it knows: the demo's one file. The frame goes out on the next tick,
 * as it does from the core: an answer within the same call stack would arrive before the
 * interface had remembered the request.
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
    // An approved server of the stand "starts"; a reconnected one stays what it was —
    // a crashed one will crash again, as a real one with the same command would.
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

/** The MCP servers of each stand chat — after whatever was done to them. */
const demoMcpNow = new Map<SessionId, McpServerSummary[]>();

/** Starts playing the recording and returns the name of the chat it went by. */
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

/** Convenience wrappers over the §15 commands. */
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

// --- updates -------------------------------------------------------------

/** Channel for the update check's answer: see `update.rs`. Both screens listen to it. */
const UPDATE_CHANNEL = "yaca://update";

/**
 * Asks GitHub whether a newer version exists. A refusal is a code (`offline`,
 * `rate_limited`, `status:502`, `bad_response`), not a phrase: the window picks the
 * words for it, in the person's language.
 */
export function checkForUpdates(): Promise<UpdateCheck> {
  return inTauri ? call<UpdateCheck>("check_for_updates") : Promise.resolve(DEMO_UPDATE);
}

/** The stand has no shell, and the "found" release is a sample to look at. */
const DEMO_UPDATE: UpdateCheck = {
  current: "1.0.0-beta",
  latest: {
    version: "1.0.1-beta",
    name: "Yaca 1.0.1-beta",
    changelog: [
      "## What's new",
      "",
      "- Update check in About: by hand and in the background",
      "- The browser column keeps its page when a chat is switched",
      "- `Read` returns the cells of a notebook with their ids",
      "",
      "## Fixed",
      "",
      "- A permission request no longer loses focus after a window reload",
      "- The installer keeps the language of Windows",
    ].join("\n"),
    url: "https://github.com/Zorahm/yaca/releases/tag/v1.0.1-beta",
    downloadUrl: "https://github.com/Zorahm/yaca/releases/download/v1.0.1-beta/Yaca_1.0.1-beta_x64-setup.exe",
    publishedAt: "2026-10-25T10:00:00Z",
    prerelease: true,
  },
};

/** What the last check found; `null` until the first one has finished. */
export function updateStatus(): Promise<UpdateCheck | null> {
  return inTauri ? call<UpdateCheck | null>("update_status") : Promise.resolve(DEMO_UPDATE);
}

/** Subscription to the check's answer: both the background check and the one started by the button. */
export function onUpdate(handler: (check: UpdateCheck) => void): Promise<UnlistenFn> {
  if (!inTauri) return Promise.resolve(() => undefined);
  return listenChannel<UpdateCheck>(UPDATE_CHANNEL, handler);
}

// --- settings §10 ----------------------------------------------------------

/** Channel for the app's settings. One name for both windows: see `prefs.rs`. */
const PREFS_CHANNEL = "yaca://preferences";

/** Channel for "the provider list has changed" — see `providers.rs`. */
const PROVIDERS_CHANNEL = "yaca://providers";

/**
 * Subscription to a provider edit made in the other window.
 *
 * There is no payload: the list is read again in full. Sending it as an event would open
 * a second path into the window, and the first mismatch between the two paths would show
 * up as a difference in the model switcher.
 */
export function onProviders(handler: () => void): Promise<UnlistenFn> {
  if (!inTauri) {
    demoProvidersHandler = handler;
    return Promise.resolve(() => {
      if (demoProvidersHandler === handler) demoProvidersHandler = null;
    });
  }
  return listenChannel<null>(PROVIDERS_CHANNEL, () => handler());
}

let demoProvidersHandler: (() => void) | null = null;

/**
 * The app's settings defaults — the same as in `prefs.rs`.
 *
 * The stand and the first frame need them: the window is drawn before the shell answers,
 * and there would be nothing to draw it with. The values are repeated here not by choice —
 * ts-rs carries types but not `impl Default` — so they must match Rust, and the test
 * `prefs.test.ts` checks that.
 */
export const DEFAULT_PREFS: Preferences = {
  theme: "paper",
  lineNumbers: true,
  compact: false,
  language: "system",
  notifications: true,
  respectRobots: true,
  shell: "auto",
  checkUpdates: true,
};

/**
 * Shells that can be chosen on this machine (§10), except "auto".
 *
 * Empty everywhere except Windows: there is one shell there, and the settings window
 * does not draw rows for a choice that does not exist (§0). The stand has no shell —
 * and nothing to choose.
 */
export function installedShells(): Promise<ShellChoice[]> {
  return inTauri ? call<ShellChoice[]>("installed_shells") : Promise.resolve([]);
}

/** The stand has no shell, and the settings live in the page's memory. */
let demoPrefs: Preferences = { ...DEFAULT_PREFS };
let demoPrefsHandler: ((prefs: Preferences) => void) | null = null;

export function preferences(): Promise<Preferences> {
  return inTauri ? call<Preferences>("preferences") : Promise.resolve({ ...demoPrefs });
}

/** Writes the settings and broadcasts them to the windows — both of them (§0). */
export function setPreferences(prefs: Preferences): Promise<void> {
  if (!inTauri) {
    demoPrefs = { ...prefs };
    demoPrefsHandler?.({ ...prefs });
    return Promise.resolve();
  }
  return call<void>("set_preferences", { prefs });
}

/** Subscription to an edit of the settings made elsewhere: the second window changes the look of this one. */
export function onPreferences(
  handler: (prefs: Preferences) => void,
): Promise<UnlistenFn> {
  if (!inTauri) {
    demoPrefsHandler = handler;
    return Promise.resolve(() => {
      if (demoPrefsHandler === handler) demoPrefsHandler = null;
    });
  }
  return listenChannel<Preferences>(PREFS_CHANNEL, handler);
}

/** Empty project settings — the form an unsaved file starts from. */
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

/** In the stand the project settings live in the page's memory, like the project list. */
let demoSettings: Settings = emptySettings();

/** Project settings (§10). A broken file arrives as a refusal, not as an empty result. */
export function projectSettings(project: string): Promise<Settings> {
  if (!inTauri) return Promise.resolve(structuredClone(demoSettings));
  return call<Settings>("project_settings", { project });
}

/** Writes `.yaca/settings.json` whole. */
export function saveProjectSettings(project: string, settings: Settings): Promise<void> {
  if (!inTauri) {
    demoSettings = structuredClone(settings);
    return Promise.resolve();
  }
  return call<void>("save_project_settings", { project, settings });
}

/**
 * Directories the §5.8 snapshot never enters.
 *
 * They are asked of the core rather than written here: the list is its own, and a copy
 * in the frontend would diverge from it at the first addition.
 */
export function shadowExcludedDirs(): Promise<string[]> {
  if (!inTauri) return Promise.resolve([".yaca", ".git", "node_modules", "target"]);
  return call<string[]>("shadow_excluded_dirs");
}

/** Which instructions file is in force for the project (§10), or `null`. */
export function projectInstructions(project: string): Promise<string | null> {
  if (!inTauri) return Promise.resolve("CLAUDE.md");
  return call<string | null>("project_instructions", { project });
}

/** The web search key lies in the keychain under this name — see `secrets.rs`. */
export const SEARCH_KEY_ITEM = "tavily";

/**
 * Where the web search key came from (§10).
 *
 * Its own command, not `apiKeySource`: Tavily is not a provider, and the shared one
 * would answer "not in the list" about a key that lies in the keychain.
 */
export function searchKeySource(): Promise<KeySource> {
  if (!inTauri) return Promise.resolve("none");
  return call<KeySource>("search_key_source");
}

// --- MCP ---------------------------------------------------------------------

type McpServers = { [key in string]?: McpServerSettings };

/** In the stand the shared server list lives in the page's memory. */
let demoOwnMcp: McpServers = {};

/**
 * The person's own MCP servers — `mcp.json` in the settings folder, shared by all projects.
 * A broken file arrives as a refusal: the window must not offer to overwrite servers it
 * failed to read (§0).
 */
export function globalMcpServers(): Promise<McpServers> {
  if (!inTauri) return Promise.resolve(structuredClone(demoOwnMcp));
  return call<McpServers>("global_mcp_servers");
}

/** Writes the shared list whole. It does not reach an open chat: that one works
    with the servers it started with. */
export function saveGlobalMcpServers(servers: McpServers): Promise<void> {
  if (!inTauri) {
    demoOwnMcp = structuredClone(servers);
    return Promise.resolve();
  }
  return call<void>("save_global_mcp_servers", { servers });
}

/** What the project's `.mcp.json` declares. For display only: the file is not ours. */
export function projectMcpFile(project: string): Promise<McpServers> {
  if (!inTauri) return Promise.resolve({});
  return call<McpServers>("project_mcp_file", { project });
}

/**
 * The servers of an open chat as they are now. Usually they arrive with the
 * `mcpUpdated` event; this is for a window that was not listening at that moment.
 */
export function sessionMcp(session: SessionId): Promise<McpServerSummary[]> {
  if (!inTauri) return Promise.resolve(demoMcpNow.get(session) ?? structuredClone(demoMcp));
  return call<McpServerSummary[]>("session_mcp", { session });
}

/** A refusal without a reason: `null`, not an empty string — that is how the core writes it. */
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
    localShell() ? invoke<boolean>("browser_supported") : Promise.resolve(false),
  states: (): Promise<BrowserState[]> =>
    localShell() ? invoke<BrowserState[]>("browser_states") : Promise.resolve([]),
  /** Shows `session`'s page at `rect` and hides every other; both `null` hide all. */
  place: (session: SessionId | null, rect: Rect | null): Promise<void> =>
    localShell() ? invoke<void>("browser_place", { session, rect }) : Promise.resolve(),
  open: (session: SessionId, url: string): Promise<void> =>
    localShell() ? invoke<void>("browser_open", { session, url }) : NO_BROWSER(),
  go: (session: SessionId, step: BrowserStep): Promise<void> =>
    localShell() ? invoke<void>("browser_go", { session, step }) : NO_BROWSER(),
  external: (session: SessionId): Promise<void> =>
    localShell() ? invoke<void>("browser_external", { session }) : NO_BROWSER(),
  devtools: (session: SessionId): Promise<void> =>
    localShell() ? invoke<void>("browser_devtools", { session }) : NO_BROWSER(),
  resetViewport: (session: SessionId): Promise<void> =>
    localShell() ? invoke<void>("browser_reset_viewport", { session }) : NO_BROWSER(),
  onState: (handler: (state: BrowserState) => void): Promise<UnlistenFn> =>
    localShell()
      ? listen<BrowserState>(BROWSER_CHANNEL, (event) => handler(event.payload))
      : Promise.resolve(() => {}),
};
