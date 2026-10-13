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
 */

import { useCallback, useEffect, useRef, useState } from "react";

import {
  closeAllChats,
  closeChat,
  commands,
  onEvents,
  openProject,
  sessionMcp,
  sendCommand,
  sessionMeta,
  sessions,
  type AgentEvent,
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
};

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

  useEffect(() => {
    // Chats of a previous life of this window are still running in the shell —
    // the webview reloads, sessions do not. Their feeds are gone, so there is
    // nothing honest left to show, and an invisible agent still editing files
    // is worse than one that was stopped.
    void closeAllChats().catch((e: unknown) => report.current(e));
  }, []);

  // The bridge sends events in frames of one animation frame (§12): a frame is
  // one snapshot of one chat's feed.
  useEffect(() => {
    const unlisten = onEvents((frame) => {
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
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, [refreshTitle, patch]);

  const send = useCallback((id: SessionId, command: Command) => {
    void sendCommand(id, command).catch((e: unknown) => report.current(e));
  }, []);

  const start = useCallback(
    async (project: string, text: string, startup: Startup, attachments: string[]) => {
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
  };
}

/** Routes a frame to its chat. A frame for a chat that is gone is dropped. */
function apply(chats: Chat[], frame: EventFrame): Chat[] {
  return chats.map((chat) => {
    if (chat.id !== frame.session) return chat;
    // The queue (§7.4) is in the feed, folded from the core's own list: what
    // waits there is what the core says waits, not a copy kept here.
    return { ...chat, feed: reduceAll(chat.feed, frame.events) };
  });
}
