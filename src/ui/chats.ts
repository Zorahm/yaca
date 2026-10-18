/**
 * Chats open in the window.
 *
 * One feed per chat, all of them live at once: a turn started in one keeps
 * running while the window is showing another, and its events keep arriving.
 * That is the whole point of holding several — a chat you have to be looking
 * at to make progress is not a background chat.
 *
 * The events of every chat come down one channel, each frame carrying the
 * session it belongs to, and are routed here. Feeds are still what §1 says
 * they are: a pure function of the event stream. There are simply several
 * streams now, and each has its own feed.
 *
 * Nothing here reaches the store: the recorded list of sessions lives on disk
 * and is read where it is shown (§9). This is only what is open.
 *
 * The window can be reloaded (Ctrl+R, a hot reload) and the chats cannot: they
 * live in the shell and go on with their turns. So on start the window does not
 * begin empty. It asks the shell for the chats that are still running, folds
 * what each of them has been through, and carries on from there; see
 * `attachChats`.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import {
  attachChats,
  closeChat,
  commands,
  onEvents,
  openProject,
  sessionMcp,
  sendCommand,
  sessionMeta,
  sessions,
  type AgentEvent,
  type AttachedChat,
  type Command,
  type EventFrame,
  type OpenedChat,
  type ProcessId,
  type SessionId,
  type Startup,
} from "../core/client";
import { emptyFeed, reduceAll, type Feed } from "../core/feed";

export type Chat = {
  id: SessionId;
  /** The project it belongs to. The store is per project (§9), and the same
      project can hold several open chats. */
  project: string;
  /** Title from the store (§3), which the core writes after the first turn. */
  title: string;
  feed: Feed;
  /** Where the arrows of the top bar stand in this chat's feed (§3). */
  at: number;
  /** Process whose output this chat is streaming (§6.5). */
  watched: ProcessId | null;
  /**
   * Which bridge of the shell this feed was built at. A frame of another epoch
   * is the old bridge's, still on its way after the window attached again, and
   * folding it would draw its events twice.
   */
  epoch: number;
};

/** Where the window remembers which chat it was showing. Per window, and gone with it. */
const ACTIVE_KEY = "yaca.active-chat";

function rememberActive(id: SessionId | null): void {
  try {
    if (id === null) window.sessionStorage.removeItem(ACTIVE_KEY);
    else window.sessionStorage.setItem(ACTIVE_KEY, id);
  } catch {
    // Storage can be refused; the window then comes back on the home screen.
  }
}

function recalledActive(): SessionId | null {
  try {
    return window.sessionStorage.getItem(ACTIVE_KEY);
  } catch {
    return null;
  }
}

/**
 * Attaches to the shell one after another, never two at once. Each attach puts a
 * new bridge in place of the last, so two in flight could land in either order
 * and leave the window with the snapshot of the bridge that was replaced.
 * (React in development runs an effect twice.)
 */
let attaching: Promise<unknown> = Promise.resolve();

/** Settles when the chats of the shell have been taken into the window. */
let attachedChats: Promise<void> = Promise.resolve();

export type Chats = {
  chats: Chat[];
  /** The chat the window is showing, or `null` for the home screen (§9.1). */
  active: Chat | null;
  activate: (id: SessionId | null) => void;
  /** First message of a new chat: it is what creates the session (§9.1). */
  start: (
    project: string,
    text: string,
    startup: Startup,
    attachments: string[],
  ) => Promise<void>;
  /** Opens a recorded session (§9), or brings it up if it is already open. */
  resume: (project: string, id: SessionId) => Promise<void>;
  close: (id: SessionId) => Promise<void>;
  send: (id: SessionId, command: Command) => void;
  patch: (id: SessionId, fields: Partial<Chat>) => void;
  /**
   * Asks the core for a chat's MCP servers as they are now. For the panel,
   * which may open after a change nobody was listening for.
   */
  refreshMcp: (id: SessionId) => Promise<void>;
};

export function useChats(onError: (error: unknown) => void): Chats {
  const [chats, setChats] = useState<Chat[]>([]);
  const [activeId, setActiveId] = useState<SessionId | null>(null);

  // Reporting an error must not re-subscribe the window to the event stream.
  const report = useRef(onError);
  report.current = onError;

  // Read back inside callbacks: a state updater runs at render time, which is
  // after the callback that asked has already decided what to do.
  const known = useRef<Chat[]>(chats);
  known.current = chats;

  // Whether the chats of the shell have been taken back into this window yet.
  const restored = useRef(false);

  const patch = useCallback((id: SessionId, fields: Partial<Chat>) => {
    setChats((prev) => prev.map((chat) => (chat.id === id ? { ...chat, ...fields } : chat)));
  }, []);

  const refreshMcp = useCallback(async (id: SessionId) => {
    const servers = await sessionMcp(id);
    // Into the feed as it is when the answer lands, not as it was when the
    // question was asked: events kept arriving in between.
    setChats((prev) =>
      prev.map((chat) => (chat.id === id ? { ...chat, feed: { ...chat.feed, mcp: servers } } : chat)),
    );
  }, []);

  const refreshTitle = useCallback(async (id: SessionId) => {
    const meta = await sessionMeta(id);
    if (meta) patch(id, { title: meta.title });
  }, [patch]);

  // The bridge sends events in frames of one animation frame (§12): a frame is
  // one snapshot of one chat's feed.
  //
  // Chats of a previous life of this window are still running in the shell —
  // the webview reloads, sessions do not. The same effect takes them back, and
  // the order inside it is the point: the subscription first, because the
  // shell's new bridge starts sending the moment it is attached, and a frame
  // sent before anyone listens is gone; then the attach, whose snapshot ends
  // exactly where the bridge's first frame begins; and the frames that arrived
  // while the snapshot was on its way are folded after it, not before.
  useEffect(() => {
    let cancelled = false;
    let held: EventFrame[] | null = [];

    const fold = (frame: EventFrame) => {
      setChats((prev) => apply(prev, frame));
      // The model named the chat: show it now rather than at the end of the
      // turn. The store has the last word, and `refreshTitle` below reconciles
      // with it, so a name typed by hand is not lost for longer than the turn.
      for (let i = frame.events.length - 1; i >= 0; i--) {
        const event = frame.events[i];
        if (event.type === "titleSet") {
          patch(frame.session, { title: event.title });
          break;
        }
      }
      // §3: the title is written down by the core when the turn ends, and the
      // top bar and the list both read it from there.
      if (frame.events.some((event) => event.type === "turnFinished")) {
        void refreshTitle(frame.session);
      }
    };

    const unlisten = onEvents((frame) => {
      if (cancelled) return;
      if (held !== null) held.push(frame);
      else fold(frame);
    });

    const attached = (async () => {
      await unlisten;
      await attaching.catch(() => undefined);
      if (cancelled) return;
      const run = attachChats();
      attaching = run;
      const found = await run;
      if (cancelled) return;

      setChats((prev) => install(prev, found));
      const remembered = recalledActive();
      if (remembered !== null && found.some((chat) => chat.id === remembered)) {
        setActiveId(remembered);
      }
      for (const chat of found) void refreshTitle(chat.id);

      const waiting = held ?? [];
      held = null;
      for (const frame of waiting) fold(frame);
      restored.current = true;
    })().catch((e: unknown) => {
      report.current(e);
      // Nothing came back, and what the window shows from here on is its own.
      restored.current = true;
    });
    attachedChats = attached;

    return () => {
      cancelled = true;
      void unlisten.then((off) => off());
    };
  }, [refreshTitle, patch]);

  // Which chat was showing is kept for the next load of the window, once the
  // chats of the last one have been taken back: before that the value is the
  // empty start of the page and would overwrite what was remembered.
  useEffect(() => {
    if (restored.current) rememberActive(activeId);
  }, [activeId]);

  const send = useCallback((id: SessionId, command: Command) => {
    void sendCommand(id, command).catch((e: unknown) => report.current(e));
  }, []);

  const start = useCallback(
    async (project: string, text: string, startup: Startup, attachments: string[]) => {
      await attachedChats;
      const opened = await openProject(project, undefined, startup);
      setChats((prev) => [...prev, born(opened, project)]);
      setActiveId(opened.id);
      void sendCommand(opened.id, commands.send(text, attachments)).catch((e: unknown) =>
        report.current(e),
      );
      void refreshTitle(opened.id);
    },
    [refreshTitle],
  );

  const resume = useCallback(
    async (project: string, id: SessionId) => {
      // Not before the chats of the shell are back: one of them may be this
      // session, still running, and opening it again would give one journal
      // two live sessions writing into it.
      await attachedChats;
      // Already open: bring it up. Opening it twice would give one journal two
      // live sessions writing into it.
      if (known.current.some((chat) => chat.id === id)) {
        setActiveId(id);
        return;
      }

      // The journal is read before the session is up: from that moment the
      // live core writes into it too, and an event arriving both down the
      // channel and out of the journal would draw two blocks instead of one.
      const recorded = await sessions.feed(project, id);
      const opened = await openProject(project, id);
      setChats((prev) => [...prev, born(opened, project, recorded)]);
      setActiveId(opened.id);
      void refreshTitle(opened.id);
    },
    [refreshTitle],
  );

  const close = useCallback(async (id: SessionId) => {
    // Out of the window first, so the row stops answering while the shell is
    // still stopping the turn behind it.
    setChats((prev) => prev.filter((chat) => chat.id !== id));
    setActiveId((now) => (now === id ? null : now));
    await closeChat(id);
  }, []);

  return {
    chats,
    active: chats.find((chat) => chat.id === activeId) ?? null,
    activate: setActiveId,
    start,
    resume,
    close,
    send,
    patch,
    refreshMcp,
  };
}

/**
 * A chat as the window first sees it.
 *
 * `recorded` is the journal of a session being reopened (§9), and the feed is
 * what it always is — that stream folded. The core hands it over already
 * trimmed of the previous run's processes, state and checkpoints, so folding
 * it needs no special case here.
 */
function born(opened: OpenedChat, project: string, recorded: AgentEvent[] = []): Chat {
  return {
    id: opened.id,
    project,
    title: "",
    // Состояние приезжает вместе с чатом, а не первым событием: композер §7.1
    // рисуется сразу, и окно не проходит через кадр, в котором чат открыт, а
    // режима и модели у него ещё нет. Оно же перекрывает журнал: режим и
    // модель у открытой заново сессии свои.
    // Серверы MCP — так же и по той же причине: они начинают подключаться
    // вместе с сессией, раньше, чем окно слушает её события.
    feed: { ...reduceAll(emptyFeed, recorded), state: opened.state, mcp: opened.mcp, lsp: opened.lsp },
    at: -1,
    watched: null,
    epoch: 0,
  };
}

/**
 * A chat the shell kept running, as the window takes it back.
 *
 * Its feed is the fold of what the shell sent, and nothing else: the mode and
 * model, the servers, the background commands and the queue are among those
 * events, so there is no second place they are copied from. The title is the
 * store's to give and is asked for after.
 */
export function chatOf(attached: AttachedChat): Chat {
  return {
    id: attached.id,
    project: attached.project,
    title: "",
    feed: reduceAll(emptyFeed, attached.events),
    at: -1,
    watched: null,
    epoch: attached.epoch,
  };
}

/**
 * The window's chats with the shell's put in. A chat that is already here is
 * replaced only by a snapshot of a later bridge, and keeps the title it has.
 */
export function install(chats: Chat[], attached: AttachedChat[]): Chat[] {
  const next = [...chats];
  for (const found of attached) {
    const at = next.findIndex((chat) => chat.id === found.id);
    if (at === -1) {
      next.push(chatOf(found));
    } else if (found.epoch > next[at].epoch) {
      next[at] = { ...chatOf(found), title: next[at].title };
    }
  }
  return next;
}

/**
 * Routes a frame to its chat. A frame for a chat that is gone is dropped, and
 * so is one of a bridge the chat no longer has (see `Chat.epoch`).
 */
export function apply(chats: Chat[], frame: EventFrame): Chat[] {
  return chats.map((chat) => {
    if (chat.id !== frame.session) return chat;
    if (chat.epoch !== frame.epoch) return chat;
    // The queue (§7.4) is in the feed, folded from the core's own list: what
    // waits there is what the core says waits, not a copy kept here.
    return { ...chat, feed: reduceAll(chat.feed, frame.events) };
  });
}
